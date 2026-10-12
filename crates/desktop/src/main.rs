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

struct VmInfo {
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
    Status(String),
    Frame(egui::ColorImage, bool),
    Notice(String),
}
type WorkResult = Result<Event, String>;

#[derive(Default)]
struct Desktop {
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
                            self.vm = Some(vm);
                            self.texture = None;
                            self.auto_display = false;
                            self.display_at = None;
                            self.notice.clear();
                            self.status = "Not checked".into();
                            self.control(ctx, "status");
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
        egui::Panel::top("header").show(ui, |ui| {
            ui.heading("FreeLane");
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
                ui.label(format!("Last observed status: {}", self.status));
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
            creation.egui_ctx.global_style_mut(|style| {
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
