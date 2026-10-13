//! Separate native viewer process per saved Windows application; snapshots, not Coherence video.
use eframe::egui;
use freelane::{
    agent,
    app_launcher::{self, AppProfile},
};
use freelane_agent_protocol::{Action, Outcome, WindowInfo, WindowTarget};
use std::{
    collections::VecDeque,
    path::Path,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};
enum Event {
    Opened(u32, Vec<WindowInfo>),
    Frame(WindowTarget, egui::ColorImage),
    Windows(Vec<WindowInfo>),
    Input,
}
type Reply = Result<Event, String>;
struct Viewer {
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
            self.notice = "Hàng đợi nhập đã đầy; thao tác mới chưa gửi.".into();
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
                return Err("Guest không trả khung hình".into());
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
                return Err("JPEG và kích thước RPC không khớp".into());
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
            self.notice = "Đang chờ Windows và agent, sau đó mở app một lần…".into();
            self.task(&ctx, move || {
                let exe = std::env::current_exe().map_err(|e| e.to_string())?;
                let cli = exe.parent().ok_or("Không tìm thấy CLI")?.join("freelane");
                let (pid, windows) = if let Some(pid) = attach_pid {
                    match agent::window_request(&p.vm, Action::Windows { pid })
                        .map_err(|e| e.to_string())?
                        .outcome
                    {
                        Outcome::Windows { windows } if !windows.is_empty() => (pid, windows),
                        _ => return Err("PID không có cửa sổ tương tác; không gửi launch".into()),
                    }
                } else {
                    app_launcher::open(&p, &cli).map_err(|e| e.to_string())?
                };
                Ok(Event::Opened(pid, windows))
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
                    Ok(Event::Opened(pid, windows)) => {
                        self.pid = Some(pid);
                        self.windows = windows;
                        self.notice = "App đã mở; đang lấy ảnh cửa sổ thật.".into();
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
                        self.notice =
                            "Ảnh cửa sổ đã cập nhật. Đây chưa phải video trực tiếp.".into();
                        self.next = Instant::now() + Duration::from_secs(1);
                    }
                    Ok(Event::Windows(windows)) => {
                        self.queue.clear();
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
                            format!("{e}. Đã hủy {} thao tác đang chờ, chưa gửi; kiểm tra app trước khi nhập lại.", self.queue.len())
                        };
                        self.live = false;
                        self.queue.clear();
                    }
                }
            }
        }
        egui::Panel::top("toolbar")
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::WHITE)
                    .inner_margin(16),
            )
            .show(ui, |ui| {
                ui.heading(&self.profile.name);
                ui.small("Cửa sổ Windows riêng · bản thử nghiệm ảnh chụp");
                ui.horizontal_wrapped(|ui| {
                    let old = self.selected;
                    egui::ComboBox::from_id_salt("guest-window")
                        .selected_text(
                            self.windows
                                .get(self.selected)
                                .map(|w| w.title.as_str())
                                .unwrap_or("Chưa có cửa sổ"),
                        )
                        .show_ui(ui, |ui| {
                            for (index, window) in self.windows.iter().enumerate() {
                                ui.selectable_value(&mut self.selected, index, &window.title);
                            }
                        });
                    if old != self.selected {
                        self.texture = None;
                        self.queue.clear();
                        self.next = Instant::now();
                    }
                    ui.checkbox(&mut self.live, "Cập nhật mỗi giây");
                    if ui
                        .add_enabled(
                            self.worker.is_none() && self.target().is_some(),
                            egui::Button::new("Chụp lại"),
                        )
                        .clicked()
                    {
                        self.capture(&ctx);
                    }
                    if ui
                        .add_enabled(
                            self.worker.is_none() && self.pid.is_some(),
                            egui::Button::new("Danh sách cửa sổ"),
                        )
                        .clicked()
                    {
                        let vm = self.profile.vm.clone();
                        let pid = self.pid.unwrap();
                        self.task(&ctx, move || {
                            match agent::window_request(&vm, Action::Windows { pid })
                                .map_err(|e| e.to_string())?
                                .outcome
                            {
                                Outcome::Windows { windows } => Ok(Event::Windows(windows)),
                                _ => Err("Phản hồi cửa sổ không hợp lệ".into()),
                            }
                        });
                    }
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.text)
                            .hint_text("Văn bản gửi tới ô đang focus trong app Windows"),
                    );
                    if ui
                        .add_enabled(
                            self.target().is_some() && !self.text.is_empty(),
                            egui::Button::new("Gửi chữ"),
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
                            self.notice = "Giới hạn 1024 byte mỗi lần gửi.".into();
                        }
                    }
                });
                if self.worker.is_some() {
                    ui.spinner();
                }
                ui.label(&self.notice);
            });
        egui::CentralPanel::default().show(ui,|ui|{
   if let Some(texture)=&self.texture {
    let response=ui.add(egui::Image::new(texture).max_size(ui.available_size()).sense(egui::Sense::click()));
    if response.clicked(){response.request_focus();if let(Some(pos),Some(target))=(response.interact_pointer_pos(),self.target()){let rect=response.rect;let x=((pos.x-rect.left())/rect.width()*32767.0).clamp(0.0,32767.0) as u16;let y=((pos.y-rect.top())/rect.height()*32767.0).clamp(0.0,32767.0) as u16;self.enqueue(Action::WindowClick{target,x,y});}}
    if response.has_focus(){for event in ctx.input(|i|i.events.clone()) {match event {
     egui::Event::Text(text) if text.len()<=1024=>{if let Some(target)=self.target(){self.enqueue(Action::WindowText{target,text});}},
     egui::Event::Key{key,pressed:true,modifiers,..} if !modifiers.ctrl && !modifiers.command && !modifiers.alt=>{
      let code=match key{egui::Key::Backspace=>8,egui::Key::Tab=>9,egui::Key::Enter=>13,egui::Key::Escape=>27,egui::Key::PageUp=>33,egui::Key::PageDown=>34,egui::Key::End=>35,egui::Key::Home=>36,egui::Key::ArrowLeft=>37,egui::Key::ArrowUp=>38,egui::Key::ArrowRight=>39,egui::Key::ArrowDown=>40,egui::Key::Delete=>46,_=>0};
      if code!=0 {if let Some(target)=self.target(){self.enqueue(Action::WindowKey{target,key:code});}}
     },_=>{}
    }}}
   }else{ui.label("Chưa có ảnh cửa sổ. Windows cần agent mới chạy trong phiên người dùng tương tác.");}
   ui.small("Đóng cửa sổ Mac chỉ đóng viewer; app Windows và VM vẫn chạy. Menu mở app trong guest, UAC và app dùng GPU có thể cần màn hình VM.");
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
        ctx.request_repaint_after(Duration::from_millis(250));
    }
}
pub fn run(path: &Path, attach_pid: Option<u32>) -> eframe::Result {
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
            }))
        }),
    )
}
