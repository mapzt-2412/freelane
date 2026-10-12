use eframe::egui;
use freelane::{agent, qemu, Config, GuestOs};
use freelane_agent_protocol::{Action, Outcome};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone)]
struct VmInfo {
    config: Config,
    original: String,
    path: PathBuf,
    name: String,
    cpus: u16,
    memory: u64,
    guest_agent: bool,
    guest_os: GuestOs,
    acceleration: &'static str,
}
enum Event {
    Loaded(VmInfo),
    Saved(VmInfo, String),
    Status(String),
    Frame(egui::ColorImage, bool),
    Notice(String),
}
type WorkResult = Result<Event, String>;

#[derive(Default)]
struct Desktop {
    settings_open: bool,
    settings: Option<Config>,
    installer_path: String,
    shared_path: String,
    draft_path: String,
    app_path: String,
    app_arguments: String,
    vm: Option<VmInfo>,
    status: String,
    notice: String,
    pending: Option<Receiver<WorkResult>>,
    texture: Option<egui::TextureHandle>,
    child: Option<Child>,
    child_errors: Option<Arc<Mutex<Vec<u8>>>>,
    refresh_at: Option<Instant>,
    auto_display: bool,
    display_at: Option<Instant>,
}
impl Desktop {
    fn work(&mut self, ctx: &egui::Context, task: impl FnOnce() -> WorkResult + Send + 'static) {
        if self.pending.is_some() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        self.pending = Some(receiver);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(task());
            ctx.request_repaint();
        });
    }
    fn open(&mut self, ctx: &egui::Context) {
        let path = PathBuf::from(self.draft_path.trim());
        self.work(ctx, move || {
            let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
            if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 1024 * 1024 {
                return Err("VM file exceeds 1 MiB".into());
            }
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let config = Config::parse(&text).map_err(|e| e.to_string())?;
            config
                .check_images(path.parent().unwrap_or(Path::new(".")))
                .map_err(|e| e.to_string())?;
            let acceleration = match config.runtime.as_ref() {
                Some(runtime) => match runtime.accelerator {
                    qemu::Accelerator::Hvf => "Hardware (HVF; requires Apple Silicon)",
                    qemu::Accelerator::Tcg => match runtime.tcg_threading {
                        None => "Software emulation (TCG; default threading)",
                        Some(qemu::TcgThreading::Single) => {
                            "Software emulation (TCG; single thread)"
                        }
                        Some(qemu::TcgThreading::Multi) => {
                            "Software emulation (TCG; multiple threads)"
                        }
                    },
                },
                None => "Runtime not configured",
            };
            Ok(Event::Loaded(VmInfo {
                path,
                config: config.clone(),
                original: text,
                name: config.name,
                guest_os: config.guest_os,
                cpus: config.cpus,
                memory: config.memory_mib,
                acceleration,
                guest_agent: config
                    .runtime
                    .as_ref()
                    .is_some_and(|runtime| runtime.guest_agent),
            }))
        });
    }
    fn save_settings(&mut self, ctx: &egui::Context) {
        let (Some(vm), Some(mut config)) = (self.vm.clone(), self.settings.clone()) else {
            return;
        };
        config.installer = (!self.installer_path.trim().is_empty())
            .then(|| PathBuf::from(self.installer_path.trim()));
        if let Some(runtime) = config.runtime.as_mut() {
            runtime.shared_directory = (!self.shared_path.trim().is_empty())
                .then(|| PathBuf::from(self.shared_path.trim()));
        }
        self.work(ctx, move || {
            let backup =
                qemu::save_config(&vm.path, &vm.original, &config).map_err(|e| e.to_string())?;
            let mut updated = vm;
            updated.original = std::fs::read_to_string(&updated.path).map_err(|e| e.to_string())?;
            updated.name = config.name.clone();
            updated.cpus = config.cpus;
            updated.memory = config.memory_mib;
            updated.guest_agent = config.runtime.as_ref().is_some_and(|r| r.guest_agent);
            updated.acceleration = match config.runtime.as_ref().map(|r| r.accelerator) {
                Some(qemu::Accelerator::Hvf) => "Hardware (HVF; requires Apple Silicon)",
                Some(qemu::Accelerator::Tcg) => {
                    "Software emulation (TCG; see settings for threading)"
                }
                None => "Runtime not configured",
            };
            updated.config = config;
            Ok(Event::Saved(
                updated,
                format!(
                    "Settings saved for next start. Backup: {}",
                    backup.display()
                ),
            ))
        });
    }
    fn settings_ui(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        let mut save = false;
        egui::Window::new("Virtual machine settings").open(&mut open)
            .default_width(580.0)
            .default_height(640.0).resizable(true).show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(570.0).show(ui, |ui| {
                let Some(config) = self.settings.as_mut() else { return };
                ui.label("Changes apply on the next start. Shut down the VM before saving.");
                ui.separator();
                ui.heading("Compute resources");
                ui.horizontal(|ui| { ui.label("Name"); ui.text_edit_singleline(&mut config.name); });
                ui.add(egui::Slider::new(&mut config.cpus, 1..=64).text("Virtual CPUs"));
                ui.horizontal(|ui| { ui.label("RAM (MiB)"); ui.add(egui::DragValue::new(&mut config.memory_mib).range(2048..=1_048_576).speed(256)); });
                ui.small("CPU and RAM are allocated to the guest. GPU passthrough is unavailable with the current Apple Silicon backend.");
                ui.separator();
                ui.heading("Storage & host files");
                ui.label(format!("Guest disk: {}", config.disk.display()));
                ui.label("Installer / CD image (blank to detach)");
                ui.add(egui::TextEdit::singleline(&mut self.installer_path).desired_width(f32::INFINITY).hint_text("/path/to/installer.iso"));
                ui.label("Host folder (blank to disable)");
                ui.add(egui::TextEdit::singleline(&mut self.shared_path).desired_width(f32::INFINITY).hint_text("/path/to/files-to-share"));
                if let Some(runtime) = config.runtime.as_mut() {
                    ui.horizontal_wrapped(|ui| {
                        ui.selectable_value(&mut runtime.share_mode, qemu::ShareMode::ReadonlyUsb, "Read-only USB · all guests");
                        ui.add_enabled_ui(config.guest_os == GuestOs::Ubuntu, |ui| {
                            ui.selectable_value(&mut runtime.share_mode, qemu::ShareMode::Virtio9p, "Live read/write · Ubuntu");
                        });
                    });
                    match runtime.share_mode {
                        qemu::ShareMode::ReadonlyUsb => { ui.small("FAT volume under 504 MiB. Keep source files unchanged while the VM runs; copy into the guest to edit."); }
                        qemu::ShareMode::Virtio9p => {
                            ui.small("Live host filesystem: guest writes change host files. Share a dedicated folder. Requires the Ubuntu 9p driver and a guest mount.");
                            ui.label("Ubuntu: sudo mount -t 9p -o trans=virtio,version=9p2000.L freelane /mnt/freelane");
                        }
                    }
                }
                if let Some(runtime) = config.runtime.as_mut() {
                    ui.separator(); ui.heading("Acceleration & devices");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut runtime.accelerator, qemu::Accelerator::Hvf, "Mac hardware · HVF");
                        ui.selectable_value(&mut runtime.accelerator, qemu::Accelerator::Tcg, "Software · TCG");
                    });
                    if runtime.accelerator == qemu::Accelerator::Hvf { runtime.tcg_threading = None; }
                    else { ui.horizontal_wrapped(|ui| {
                        ui.label("TCG threads");
                        ui.selectable_value(&mut runtime.tcg_threading, None, "Default");
                        ui.selectable_value(&mut runtime.tcg_threading, Some(qemu::TcgThreading::Single), "Single");
                        ui.selectable_value(&mut runtime.tcg_threading, Some(qemu::TcgThreading::Multi), "Multi");
                    }); }
                    ui.checkbox(&mut runtime.network, "Guest network · NAT, no forwarded host ports");
                    if runtime.network {
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Network adapter");
                            ui.selectable_value(&mut runtime.network_device, qemu::NetworkDevice::Virtio, "Virtio PCI");
                            ui.selectable_value(&mut runtime.network_device, qemu::NetworkDevice::Usb, "USB network");
                        });
                    }
                    ui.checkbox(&mut runtime.headless, "Headless · hide native QEMU window");
                    ui.checkbox(&mut runtime.guest_agent, "Guest agent channel · agent installation required");
                    ui.checkbox(&mut runtime.serial_console, "Private serial console socket");
                    ui.separator(); ui.heading("Physical USB devices");
                    ui.small("Use scripts/list-usb.py to inspect host bus/address before starting. Addresses may change after reconnection. QEMU needs libusb and OS access; a captured device becomes unavailable to host apps.");
                    let mut remove = None;
                    for (index, device) in runtime.usb_devices.iter_mut().enumerate() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label("Bus"); ui.add(egui::DragValue::new(&mut device.bus).range(1..=255));
                            ui.label("Address"); ui.add(egui::DragValue::new(&mut device.address).range(1..=127));
                            ui.label("Vendor"); ui.add(egui::DragValue::new(&mut device.vendor_id).hexadecimal(4, false, true));
                            ui.label("Product"); ui.add(egui::DragValue::new(&mut device.product_id).hexadecimal(4, false, true));
                            if ui.button("Remove").clicked() { remove = Some(index); }
                        });
                    }
                    if let Some(index) = remove { runtime.usb_devices.remove(index); }
                    if ui.add_enabled(runtime.usb_devices.len() < 16, egui::Button::new("Add USB device")).clicked() {
                        runtime.usb_devices.push(qemu::UsbHostDevice { bus: 1, address: 1, vendor_id: 0, product_id: 0 });
                    }
                    ui.small("TPM, firmware and disk controller are preserved. Network drivers must exist inside the guest.");
                }
                ui.separator();
                if ui.add_enabled(self.pending.is_none() && self.child.is_none(), egui::Button::new("Save settings & create backup").fill(egui::Color32::from_rgb(52, 105, 177))).clicked() { save = true; }
                ui.small("Saving rewrites TOML formatting; the original file is backed up. Active VMs and conflicting file edits are rejected.");
                if !self.notice.is_empty() { ui.label(&self.notice); }
            });
        });
        self.settings_open = open;
        if save {
            self.save_settings(ctx);
        }
    }
    fn control(&mut self, ctx: &egui::Context, action: &'static str) {
        let Some(vm) = &self.vm else { return };
        let path = vm.path.clone();
        self.work(ctx, move || {
            let reply = qemu::control(&path, action).map_err(|e| {
                if action == "status" {
                    format!("Status unavailable. If the VM is powered off, use Start; otherwise inspect its runtime log. Details: {e}")
                } else {
                    e.to_string()
                }
            })?;
            Ok(match action {
                "status" => Event::Status(reply["status"].as_str().unwrap_or("unknown").to_owned()),
                "shutdown" => {
                    Event::Notice("Guest shutdown requested. Refresh to check completion.".into())
                }
                "stop" => {
                    Event::Notice("Force stop requested. Refresh to check completion.".into())
                }
                "pause" => Event::Status("paused".into()),
                "resume" => Event::Status("running".into()),
                _ => Event::Notice("Request sent".into()),
            })
        });
    }
    fn capture(&mut self, ctx: &egui::Context, announce: bool) {
        let Some(vm) = &self.vm else { return };
        let path = vm.path.clone();
        self.work(ctx, move || {
            let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
            let frame = temp.path().join("frame.ppm");
            qemu::screenshot(&path, &frame).map_err(|e| e.to_string())?;
            if std::fs::metadata(&frame).map_err(|e| e.to_string())?.len() > 64 * 1024 * 1024 {
                return Err("Display frame exceeds 64 MiB".into());
            }
            let mut reader = image::ImageReader::open(frame).map_err(|e| e.to_string())?;
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(64 * 1024 * 1024);
            limits.max_image_width = Some(8192);
            limits.max_image_height = Some(8192);
            reader.limits(limits);
            let rgb = reader.decode().map_err(|e| e.to_string())?.into_rgb8();
            Ok(Event::Frame(
                egui::ColorImage::from_rgb(
                    [rgb.width() as usize, rgb.height() as usize],
                    rgb.as_raw(),
                ),
                announce,
            ))
        });
    }
    fn guest_request(&mut self, ctx: &egui::Context, launch: bool) {
        let Some(vm) = &self.vm else { return };
        let path = vm.path.clone();
        let action = if launch {
            Action::Launch {
                executable: self.app_path.trim().to_owned(),
                arguments: if self.app_arguments.is_empty() {
                    Vec::new()
                } else {
                    self.app_arguments.lines().map(str::to_owned).collect()
                },
            }
        } else {
            Action::Ping
        };
        self.work(ctx, move || {
            let notice = match agent::request(&path, action) {
                Ok(response) => match response.outcome {
                    Outcome::Pong { os, arch } => format!("Guest responded: {os} / {arch}"),
                    Outcome::Launched { pid } => format!("Guest reported process started (PID {pid}). Check its guest window."),
                    Outcome::Failed { message } => format!("Guest request failed: {message}"),
                },
                Err(error) => {
                    let followup = if launch { " If launching timed out, check the guest before trying again; execution outcome may be unknown." } else { "" };
                    format!("Guest request failed: {error}.{followup}")
                },
            };
            Ok(Event::Notice(notice))
        });
    }
    fn click(&mut self, ctx: &egui::Context, x: u16, y: u16) {
        let Some(vm) = &self.vm else { return };
        let path = vm.path.clone();
        self.work(ctx, move || {
            let notice = match qemu::click(&path, x, y) {
                Ok(()) => "Mouse click sent. Check the updated display.".into(),
                Err(error) => error.to_string(),
            };
            Ok(Event::Notice(notice))
        });
    }
    fn start(&mut self) {
        let Some(vm) = &self.vm else { return };
        let result = std::env::current_exe().map_err(|e| e.to_string()).and_then(|exe| {
            let cli = exe.parent().ok_or("Cannot locate FreeLane CLI")?.join("freelane");
            Command::new(cli).args(["start"]).arg(&vm.path)
                .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped())
                .spawn().map_err(|e| format!("Cannot start VM: {e}. Build the FreeLane CLI alongside the desktop binary."))
        });
        match result {
            Ok(mut child) => {
                let errors = Arc::new(Mutex::new(Vec::new()));
                if let Some(mut stderr) = child.stderr.take() {
                    let captured = Arc::clone(&errors);
                    // Drain continuously: a full stderr pipe must never stall the VM.
                    std::thread::spawn(move || {
                        let mut chunk = [0u8; 4096];
                        while let Ok(count) = stderr.read(&mut chunk) {
                            if count == 0 {
                                break;
                            }
                            if let Ok(mut bytes) = captured.lock() {
                                let keep = count.min((16 * 1024usize).saturating_sub(bytes.len()));
                                bytes.extend_from_slice(&chunk[..keep]);
                            }
                        }
                    });
                }
                self.child_errors = Some(errors);
                self.child = Some(child);
                self.status = "Starting; awaiting VM status".into();
                self.refresh_at = Some(Instant::now() + Duration::from_secs(2));
            }
            Err(error) => self.notice = error,
        }
    }
    fn receive(&mut self, ctx: &egui::Context) {
        if let Some(receiver) = &self.pending {
            match receiver.try_recv() {
                Ok(result) => {
                    self.pending = None;
                    match result {
                        Ok(Event::Loaded(vm)) => {
                            self.settings = Some(vm.config.clone());
                            self.installer_path = vm
                                .config
                                .installer
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default();
                            self.shared_path = vm
                                .config
                                .runtime
                                .as_ref()
                                .and_then(|r| r.shared_directory.as_ref())
                                .map(|p| p.display().to_string())
                                .unwrap_or_default();
                            self.vm = Some(vm);
                            self.texture = None;
                            self.auto_display = false;
                            self.display_at = None;
                            self.notice.clear();
                            self.status = "Not checked".into();
                            self.control(ctx, "status");
                        }
                        Ok(Event::Saved(vm, notice)) => {
                            self.vm = Some(vm);
                            self.notice = notice;
                            self.settings_open = false;
                            self.status = "Not checked".into();
                            self.texture = None;
                            self.auto_display = false;
                        }
                        Ok(Event::Status(status)) => {
                            self.status = status;
                            self.notice.clear();
                        }
                        Ok(Event::Frame(image, announce)) => {
                            self.texture = Some(ctx.load_texture(
                                "guest-frame",
                                image,
                                egui::TextureOptions::LINEAR,
                            ));
                            if announce {
                                self.notice = "Display snapshot updated".into();
                            }
                        }
                        Ok(Event::Notice(notice)) => self.notice = notice,
                        Err(error) => {
                            self.auto_display = false;
                            self.status = "Unknown".into();
                            self.notice = error;
                        }
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.auto_display = false;
                    self.pending = None;
                    self.notice = "VM worker disconnected".into();
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(100))
                }
            }
        }
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(Some(exit)) => {
                    let bytes = self
                        .child_errors
                        .take()
                        .and_then(|errors| errors.lock().ok().map(|bytes| bytes.clone()))
                        .unwrap_or_default();
                    self.notice = format!(
                        "VM process ended ({exit}). {}",
                        String::from_utf8_lossy(&bytes)
                    );
                    self.child = None;
                    self.auto_display = false;
                    self.status = "Unknown".into();
                    self.refresh_at = None;
                }
                Ok(None) => ctx.request_repaint_after(Duration::from_millis(500)),
                Err(error) => self.notice = error.to_string(),
            }
        }
        if self.pending.is_none() && self.refresh_at.is_some_and(|at| Instant::now() >= at) {
            self.refresh_at = None;
            self.control(ctx, "status");
        }
        if self.auto_display && self.vm.is_some() {
            let now = Instant::now();
            if self.pending.is_none() && self.display_at.is_none_or(|at| now >= at) {
                self.display_at = Some(now + Duration::from_secs(5));
                self.capture(ctx, false);
            }
            ctx.request_repaint_after(Duration::from_secs(1));
        }
    }
}
impl eframe::App for Desktop {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.receive(&ctx);
        self.settings_ui(&ctx);
        egui::Panel::top("header").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(115, 196, 255),
                    egui::RichText::new("FreeLane").size(26.0).strong(),
                );
                ui.label(
                    egui::RichText::new("ARM64 LAB").color(egui::Color32::from_rgb(178, 147, 255)),
                );
            });
            ui.label("Windows and Ubuntu on your desktop · experimental VM manager");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label("VM file");
                ui.add(
                    egui::TextEdit::singleline(&mut self.draft_path)
                        .desired_width((ui.available_width() - 115.0).max(100.0)),
                );
                if ui
                    .add_enabled(
                        self.pending.is_none()
                            && self.child.is_none()
                            && !self.draft_path.trim().is_empty(),
                        egui::Button::new("Open VM"),
                    )
                    .clicked()
                {
                    self.open(&ctx);
                }
            });
            ui.add_space(8.0);
        });
        egui::Panel::left("controls")
            .default_size(330.0)
            .size_range(280.0..=420.0)
            .resizable(true)
            .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
            ui.separator();
            if let Some(vm) = &self.vm {
                ui.heading(&vm.name);
                ui.label(match vm.guest_os { GuestOs::Ubuntu => "Ubuntu · ARM64", GuestOs::Windows => "Windows · ARM64" });
                ui.collapsing("Configuration path", |ui| { ui.label(vm.path.display().to_string()); });
                ui.label(format!("{} CPUs · {:.1} GiB RAM", vm.cpus, vm.memory as f64 / 1024.0));
                let color = if self.status == "running" { egui::Color32::from_rgb(105, 220, 166) } else if self.status == "paused" { egui::Color32::from_rgb(255, 199, 104) } else { egui::Color32::from_rgb(166, 184, 208) };
                ui.colored_label(color, format!("Last observed: {}", self.status));
                if ui.add_enabled(self.pending.is_none(), egui::Button::new("⚙ VM settings")).clicked() { self.notice.clear(); self.settings_open = true; }
                ui.small(format!("Configured acceleration: {}", vm.acceleration));
                let busy = self.pending.is_some();
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(!busy && self.child.is_none() && self.status != "running" && self.status != "paused", egui::Button::new("Start")).clicked() { self.start(); }
                    for (label, action) in [("Refresh status", "status"), ("Pause", "pause"), ("Resume", "resume"), ("Shutdown", "shutdown"), ("Force stop", "stop")] {
                        if ui.add_enabled(!busy, egui::Button::new(label)).clicked() { self.control(&ctx, action); }
                    }
                    if ui.add_enabled(!busy, egui::Button::new("Refresh display")).clicked() { self.capture(&ctx, true); }
                });
            }
            if self.vm.is_some() && ui.checkbox(&mut self.auto_display, "Refresh display every 5 seconds").changed() {
                self.display_at = None;
                ctx.request_repaint();
            }
            if let Some(vm) = &self.vm {
                let configured = vm.guest_agent;
                let enabled = configured && self.pending.is_none();
                ui.separator();
                ui.heading("Guest applications");
                ui.vertical(|ui| {
                    let ubuntu = self.vm.as_ref().is_some_and(|vm| vm.guest_os == GuestOs::Ubuntu);
                    ui.label(if ubuntu { "Ubuntu app (absolute path)" } else { "Windows app (.exe path)" });
                    ui.add(egui::TextEdit::singleline(&mut self.app_path)
                        .desired_width(f32::INFINITY).hint_text(if ubuntu { "/usr/bin/gedit" } else { r"C:\Windows\System32\notepad.exe" }));
                    if ui.add_enabled(enabled, egui::Button::new("Check guest connection")).clicked() {
                        self.guest_request(&ctx, false);
                    }
                    if ui.add_enabled(enabled && !self.app_path.trim().is_empty(), egui::Button::new("Launch app")).clicked() {
                        self.guest_request(&ctx, true);
                    }
                });
                ui.collapsing("Arguments (one per line)", |ui| {
                    ui.add(egui::TextEdit::multiline(&mut self.app_arguments).desired_width(f32::INFINITY).desired_rows(2));
                });
                if !configured {
                    ui.small("Enable the guest agent in VM settings and run it in the guest's interactive session to launch apps.");
                }
            }
            if self.pending.is_some() { ui.spinner(); }
            if !self.notice.is_empty() { ui.label(&self.notice); }
            if self.vm.is_none() {
                ui.heading("Choose a virtual machine");
                ui.label("Open an existing vm.toml above to view its resources and controls.");
                ui.add_space(8.0);
                ui.small("Create Windows or Ubuntu ARM64 profiles with the FreeLane CLI.");
            }
            ui.separator();
            ui.small("Shutdown requests guest powerdown. Force stop can lose unsaved data.");
            ui.small("Closing FreeLane leaves the VM running.");
            });
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Guest display");
            ui.small("Snapshots · optional refresh every 5 seconds");
            ui.add_space(10.0);
            let display_height = (ui.available_height() - 50.0).max(100.0);
            egui::ScrollArea::both().max_height(display_height).show(ui, |ui| {
                if let Some(texture) = &self.texture {
                    let frame = ui.add(egui::Image::new(texture).max_size(egui::vec2(ui.available_width(), display_height - 8.0)).sense(egui::Sense::click()));
                    if frame.clicked() && self.pending.is_none() {
                        if let Some(point) = frame.interact_pointer_pos() {
                            let x = ((point.x - frame.rect.left()) / frame.rect.width() * 32767.0).clamp(0.0, 32767.0).round() as u16;
                            let y = ((point.y - frame.rect.top()) / frame.rect.height() * 32767.0).clamp(0.0, 32767.0).round() as u16;
                            self.click(&ctx, x, y);
                        }
                    }
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label("No display snapshot yet\nStart your VM, then choose Refresh display.");
                    });
                }
            });
            ui.separator();
            ui.small("Click a display snapshot to send mouse input. Use the VM's native display for keyboard input and interactive apps.");

        });
    }
}
fn main() -> eframe::Result {
    let path = std::env::args().nth(1).unwrap_or_default();
    eframe::run_native(
        "FreeLane",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1200.0, 820.0])
                .with_min_inner_size([800.0, 600.0]),
            ..Default::default()
        },
        Box::new(move |creation| {
            creation.egui_ctx.set_theme(egui::Theme::Dark);
            creation.egui_ctx.global_style_mut(|style| {
                style.visuals.panel_fill = egui::Color32::from_rgb(18, 25, 39);
                style.visuals.window_fill = egui::Color32::from_rgb(25, 34, 51);
                style.visuals.extreme_bg_color = egui::Color32::from_rgb(12, 18, 30);
                style.visuals.selection.bg_fill = egui::Color32::from_rgb(57, 105, 178);
                style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(37, 53, 77);
                style.visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(37, 53, 77);
                style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 89, 130);
                style.visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(55, 89, 130);
                style.visuals.override_text_color = Some(egui::Color32::from_rgb(222, 232, 247));
                style.spacing.item_spacing = egui::vec2(10.0, 10.0);
                style.spacing.button_padding = egui::vec2(12.0, 7.0);
            });
            Ok(Box::new(Desktop {
                draft_path: path,
                ..Default::default()
            }))
        }),
    )
}
