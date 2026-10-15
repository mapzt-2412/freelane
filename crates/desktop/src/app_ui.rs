//! Separate native viewer process per saved Windows application; snapshots, not Coherence video.
use eframe::egui;
use freelane::{
    agent,
    app_launcher::{self, AppProfile},
};
use freelane_agent_protocol::window_input::{Button, Gesture, Position};
use freelane_agent_protocol::{Action, Outcome, WindowInfo, WindowTarget};
use std::{
    collections::VecDeque,
    path::Path,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};
enum Event {
    Opened(u32, Vec<WindowInfo>, bool),
    Frame(WindowTarget, egui::ColorImage),
    Windows(Vec<WindowInfo>),
    Input,
}
type Reply = Result<Event, String>;
struct Viewer {
    language: super::Language,
    profile: AppProfile,
    attach_pid: Option<u32>,
    pid: Option<u32>,
    windows: Vec<WindowInfo>,
    selected: usize,
    texture: Option<egui::TextureHandle>,
    worker: Option<Receiver<Reply>>,
    queue: VecDeque<Action>,
    next: Instant,
    notice: String,
    started: bool,
    live: bool,
    text: String,
    extended: bool,
    drag: Option<(WindowTarget, Position, Button)>,
    guest_width: u16,
    guest_height: u16,
}
impl Viewer {
    fn task(&mut self, ctx: &egui::Context, f: impl FnOnce() -> Reply + Send + 'static) {
        let (sender, receiver) = mpsc::channel();
        self.worker = Some(receiver);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = sender.send(f());
            ctx.request_repaint();
        });
    }
    fn target(&self) -> Option<WindowTarget> {
        self.windows.get(self.selected).map(|w| w.target.clone())
    }
    fn enqueue(&mut self, action: Action) {
        if self.queue.len() < 32 {
            self.queue.push_back(action);
        } else {
            self.notice = "Input queue full; new action was not sent".into();
        }
    }
    fn capture(&mut self, ctx: &egui::Context) {
        let Some(target) = self.target() else {
            return;
        };
        let vm = self.profile.vm.clone();
        self.task(ctx, move || {
            let action = Action::CaptureWindow {
                target: target.clone(),
            };
            let reply = match agent::window_request(
                &vm,
                Action::CaptureWindow {
                    target: target.clone(),
                },
            ) {
                Ok(reply) => reply,
                Err(error)
                    if error.to_string()
                        == "Guest agent: Window capture timed out; guest may be unresponsive" =>
                {
                    // A guest-reported capture timeout acknowledges this read-only RPC.
                    // Retry one snapshot after a pause; never retry input or transport errors.
                    std::thread::sleep(Duration::from_secs(1));
                    agent::window_request(&vm, action).map_err(|e| e.to_string())?
                }
                Err(error) => return Err(error.to_string()),
            };
            let Outcome::WindowFrame {
                width,
                height,
                jpeg_hex,
                ..
            } = reply.outcome
            else {
                return Err("Guest did not return a window frame".into());
            };
            let bytes: Result<Vec<u8>, _> = jpeg_hex
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap_or(""), 16))
                .collect();
            let bytes = bytes.map_err(|e| e.to_string())?;
            let mut reader = image::ImageReader::with_format(
                std::io::Cursor::new(bytes),
                image::ImageFormat::Jpeg,
            );
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(4 * 1024 * 1024);
            limits.max_image_width = Some(640);
            limits.max_image_height = Some(480);
            reader.limits(limits);
            let rgb = reader.decode().map_err(|e| e.to_string())?.into_rgb8();
            if rgb.width() != width || rgb.height() != height {
                return Err("JPEG dimensions do not match the response".into());
            }
            Ok(Event::Frame(
                target,
                egui::ColorImage::from_rgb([width as usize, height as usize], rgb.as_raw()),
            ))
        });
    }
}
impl eframe::App for Viewer {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.started {
            self.started = true;
            let p = self.profile.clone();
            let attach_pid = self.attach_pid;
            self.notice = "Waiting for Windows and agent; launch will be sent once".into();
            self.task(&ctx, move || {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                let cli = exe.parent().ok_or("CLI not found")?.join("freelane");
                let (pid, windows) = if let Some(pid) = attach_pid {
                    match agent::window_inventory(&p.vm, pid)
                        .map_err(|e| e.to_string())?
                        .outcome
                    {
                        Outcome::Windows { windows } if !windows.is_empty() => (pid, windows),
                        _ => {
                            return Err(
                                "Process has no interactive window; launch was not sent".into()
                            )
                        }
                    }
                } else {
                    app_launcher::open(&p, &cli).map_err(|e| e.to_string())?
                };
                let extended = freelane::agent_capabilities::inspect(&p.vm)
                    .ok()
                    .is_some_and(|i| {
                        i.supports(
                            "window_input",
                            freelane_agent_protocol::window_input::VERSION,
                        ) == Some(true)
                    });
                Ok(Event::Opened(pid, windows, extended))
            });
        }
        if let Some(receiver) = &self.worker {
            let ready = match receiver.try_recv() {
                Ok(reply) => Some(reply),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(
                    "Viewer worker stopped; inspect before retrying any mutation".into(),
                )),
            };
            if let Some(reply) = ready {
                self.worker = None;
                match reply {
                    Ok(Event::Opened(pid, windows, extended)) => {
                        self.extended = extended;
                        self.pid = Some(pid);
                        self.windows = windows;
                        self.notice = "Application opened; capturing window".into();
                        self.next = Instant::now();
                    }
                    Ok(Event::Frame(target, frame)) => {
                        if self.target().as_ref() != Some(&target) {
                            self.next = Instant::now();
                            return;
                        }

                        self.texture = Some(ctx.load_texture(
                            "app-frame",
                            frame,
                            egui::TextureOptions::LINEAR,
                        ));
                        self.notice.clear();
                        self.next = Instant::now() + Duration::from_secs(1);
                    }
                    Ok(Event::Windows(windows)) => {
                        self.queue.clear();
                        self.drag = None;
                        self.windows = windows;
                        self.selected = 0;
                        self.texture = None;
                        self.next = Instant::now();
                    }
                    Ok(Event::Input) => {
                        self.next = Instant::now();
                    }
                    Err(e) => {
                        self.notice = if self.queue.is_empty() {
                            e
                        } else {
                            format!("{e}. Discarded {} unsent actions; inspect the application before retrying", self.queue.len())
                        };
                        self.live = false;
                        self.queue.clear();
                    }
                }
            }
        }
        let lang = self.language;
        egui::Panel::top("toolbar")
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::WHITE)
                    .inner_margin(16),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(&self.profile.name);
                    egui::ComboBox::from_id_salt("viewer-language")
                        .selected_text(if self.language == super::Language::English {
                            "English"
                        } else {
                            "Tiếng Việt"
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.language,
                                super::Language::English,
                                "English",
                            );
                            ui.selectable_value(
                                &mut self.language,
                                super::Language::Vietnamese,
                                "Tiếng Việt",
                            );
                        });
                });
                ui.small(lang.text("Windows app viewer"));
                ui.horizontal_wrapped(|ui| {
                    let old = self.selected;
                    egui::ComboBox::from_id_salt("guest-window")
                        .selected_text(
                            self.windows
                                .get(self.selected)
                                .map(|w| w.title.as_str())
                                .unwrap_or(lang.text("No windows")),
                        )
                        .show_ui(ui, |ui| {
                            for (index, window) in self.windows.iter().enumerate() {
                                ui.selectable_value(&mut self.selected, index, &window.title);
                            }
                        });
                    if old != self.selected {
                        self.drag = None;
                        self.texture = None;
                        self.queue.clear();
                        self.next = Instant::now();
                    }
                    ui.checkbox(&mut self.live, lang.text("Refresh every second"));
                    if ui
                        .add_enabled(
                            self.worker.is_none() && self.target().is_some(),
                            egui::Button::new(lang.text("Refresh frame")),
                        )
                        .clicked()
                    {
                        self.capture(&ctx);
                    }
                    if ui
                        .add_enabled(
                            self.worker.is_none() && self.pid.is_some(),
                            egui::Button::new(lang.text("Refresh windows")),
                        )
                        .clicked()
                    {
                        let vm = self.profile.vm.clone();
                        let pid = self.pid.unwrap();
                        self.task(&ctx, move || {
                            match agent::window_inventory(&vm, pid)
                                .map_err(|e| e.to_string())?
                                .outcome
                            {
                                Outcome::Windows { windows } => Ok(Event::Windows(windows)),
                                _ => Err("Invalid window response".into()),
                            }
                        });
                    }
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.text)
                            .hint_text(lang.text("Text for focused guest control")),
                    );
                    if ui
                        .add_enabled(
                            self.target().is_some() && !self.text.is_empty(),
                            egui::Button::new(lang.text("Send text")),
                        )
                        .clicked()
                    {
                        if self.text.len() <= 1024 {
                            let text = std::mem::take(&mut self.text);
                            self.enqueue(Action::WindowText {
                                target: self.target().unwrap(),
                                text,
                            });
                        } else {
                            self.notice = "Text is limited to 1024 bytes per request".into();
                        }
                    }
                });
                if self.extended {
                    ui.horizontal_wrapped(|ui| {
                        for (label, key) in [("Copy in guest", 67), ("Paste in guest", 86)] {
                            if ui
                                .add_enabled(
                                    self.target().is_some(),
                                    egui::Button::new(lang.text(label)),
                                )
                                .clicked()
                            {
                                self.enqueue(Action::WindowShortcut {
                                    target: self.target().unwrap(),
                                    key,
                                    modifiers: 1,
                                });
                            }
                        }
                        ui.label(lang.text("Guest window size"));
                        ui.add(egui::DragValue::new(&mut self.guest_width).range(64..=8192));
                        ui.label("×");
                        ui.add(egui::DragValue::new(&mut self.guest_height).range(64..=8192));
                        if ui
                            .add_enabled(
                                self.target().is_some(),
                                egui::Button::new(lang.text("Resize window")),
                            )
                            .clicked()
                        {
                            self.enqueue(Action::WindowResize {
                                target: self.target().unwrap(),
                                width: self.guest_width,
                                height: self.guest_height,
                            });
                        }
                    });
                }
                if self.worker.is_some() {
                    ui.spinner();
                }
                ui.label(lang.message(&self.notice));
            });
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(texture) = &self.texture {
                let response = ui.add(
                    egui::Image::new(texture)
                        .max_size(ui.available_size())
                        .sense(egui::Sense::click_and_drag()),
                );
                let position = |pos: egui::Pos2| Position {
                    x: ((pos.x - response.rect.left()) / response.rect.width() * 32767.0)
                        .clamp(0.0, 32767.0) as u16,
                    y: ((pos.y - response.rect.top()) / response.rect.height() * 32767.0)
                        .clamp(0.0, 32767.0) as u16,
                };
                for (native, button) in [
                    (egui::PointerButton::Primary, Button::Primary),
                    (egui::PointerButton::Secondary, Button::Secondary),
                ] {
                    if response.clicked_by(native) {
                        response.request_focus();
                        if let (Some(pos), Some(target)) =
                            (response.interact_pointer_pos(), self.target())
                        {
                            let at = position(pos);
                            if self.extended {
                                // egui emits the first single click before the second click arrives.
                                // The second event emits only double-click/up, avoiding a third click.
                                self.enqueue(Action::WindowPointer {
                                    target,
                                    gesture: Gesture::Click {
                                        at,
                                        button,
                                        count: if response.double_clicked_by(native) {
                                            2
                                        } else {
                                            1
                                        },
                                    },
                                });
                            } else if button == Button::Primary {
                                self.enqueue(Action::WindowClick {
                                    target,
                                    x: at.x,
                                    y: at.y,
                                });
                            }
                        }
                    }
                    if self.extended && response.drag_started_by(native) {
                        response.request_focus();
                        if let (Some(origin), Some(target)) =
                            (ctx.input(|i| i.pointer.press_origin()), self.target())
                        {
                            self.drag = Some((target, position(origin), button));
                        }
                    }
                    if response.drag_stopped_by(native) {
                        if let Some((target, from, drag_button)) = self.drag.take() {
                            if drag_button == button && self.target().as_ref() == Some(&target) {
                                if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
                                    self.enqueue(Action::WindowPointer {
                                        target,
                                        gesture: Gesture::Drag {
                                            from,
                                            to: position(pos),
                                            button,
                                        },
                                    });
                                }
                            }
                        }
                    }
                }
                for event in ctx.input(|i| i.events.clone()) {
                    if self.extended && response.hovered() {
                        if let egui::Event::MouseWheel { unit, delta, .. } = event {
                            if let (Some(pos), Some(target)) =
                                (ctx.input(|i| i.pointer.hover_pos()), self.target())
                            {
                                for (horizontal, value) in [(false, delta.y), (true, delta.x)] {
                                    let delta =
                                        super::app_input::wheel_delta(unit, horizontal, value);
                                    if delta != 0 {
                                        self.enqueue(Action::WindowPointer {
                                            target: target.clone(),
                                            gesture: Gesture::Wheel {
                                                at: position(pos),
                                                horizontal,
                                                delta,
                                            },
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                if response.has_focus() {
                    if let Some(target) = self.target() {
                        for action in super::app_input::key_actions(
                            &ctx.input(|i| i.events.clone()),
                            &target,
                            self.extended,
                        ) {
                            self.enqueue(action);
                        }
                    }
                }
            } else {
                ui.label(
                    lang.text("No window frame; run the updated agent in the Windows user session"),
                );
            }
            ui.small(lang.text("Closing this viewer keeps the guest application and VM running"));
        });
        if self.worker.is_none() {
            if let Some(action) = self.queue.pop_front() {
                let vm = self.profile.vm.clone();
                self.task(&ctx, move || {
                    agent::window_request(&vm, action).map_err(|e| e.to_string())?;
                    Ok(Event::Input)
                });
            } else if self.live && self.target().is_some() && Instant::now() >= self.next {
                self.capture(&ctx);
            }
        }
        let repaint = if self.worker.is_some() || !self.queue.is_empty() {
            Duration::from_millis(250) // Observe a disconnected worker even if it never sends a wakeup.
        } else if self.live && self.target().is_some() {
            self.next
                .saturating_duration_since(Instant::now())
                .max(Duration::from_millis(16))
        } else {
            Duration::from_secs(5) // Native input and worker completion still wake the event loop immediately.
        };
        ctx.request_repaint_after(repaint);
    }
}
pub fn run(path: &Path, attach_pid: Option<u32>, language: super::Language) -> eframe::Result {
    let profile = match AppProfile::load(path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return Err(eframe::Error::AppCreation(Box::new(e)));
        }
    };
    let title = profile.name.clone();
    eframe::run_native(
        &title,
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([900.0, 650.0])
                .with_min_inner_size([400.0, 300.0]),
            ..Default::default()
        },
        Box::new(move |creation| {
            super::interface::install_style(&creation.egui_ctx);
            Ok(Box::new(Viewer {
                language,
                profile,
                attach_pid,
                pid: None,
                windows: Vec::new(),
                selected: 0,
                texture: None,
                worker: None,
                queue: VecDeque::new(),
                next: Instant::now(),
                notice: String::new(),
                started: false,
                live: true,
                text: String::new(),
                extended: false,
                drag: None,
                guest_width: 1280,
                guest_height: 720,
            }))
        }),
    )
}
