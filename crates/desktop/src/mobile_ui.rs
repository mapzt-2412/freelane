use super::*;
use freelane::mobile::{self, Platform};
impl Desktop {
    fn mobile_task(
        &mut self,
        ctx: &egui::Context,
        task: impl FnOnce() -> WorkResult + Send + 'static,
    ) {
        self.work(ctx, move || {
            Ok(match task() {
                Ok(event) => event,
                Err(error) => Event::MobileNotice(error),
            })
        });
    }
    fn load_mobile(&mut self, ctx: &egui::Context) {
        let path = PathBuf::from(self.mobile_path.trim());
        self.mobile_task(ctx, move || {
            let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
            let profile = mobile::load(&path).map_err(|e| e.to_string())?;
            Ok(Event::MobileLoaded(path, profile))
        });
    }
    fn mobile_action(&mut self, ctx: &egui::Context, action: &'static str) {
        let Some((path, profile)) = self.mobile_profile.clone() else {
            return;
        };
        let application = self.mobile_application.trim().to_owned();
        let package = PathBuf::from(self.mobile_package.trim());
        self.mobile_task(ctx, move || {
            let base = path.parent().unwrap();
            let result = match action {
                "status" => profile.status(base).map(|v| v.to_string()),
                "devices" => profile.devices(base),
                "install" => profile.install(base, &package),
                "launch" => profile.launch(base, &application),
                "capture" => {
                    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
                    let frame = temp.path().join("frame.png");
                    profile.capture(base, &frame).map_err(|e| e.to_string())?;
                    let mut reader = image::ImageReader::open(frame).map_err(|e| e.to_string())?;
                    let mut limits = image::Limits::default();
                    limits.max_alloc = Some(64 * 1024 * 1024);
                    limits.max_image_width = Some(8192);
                    limits.max_image_height = Some(8192);
                    reader.limits(limits);
                    let rgb = reader.decode().map_err(|e| e.to_string())?.into_rgb8();
                    return Ok(Event::MobileFrame(egui::ColorImage::from_rgb(
                        [rgb.width() as usize, rgb.height() as usize],
                        rgb.as_raw(),
                    )));
                }
                _ => profile.control(base, action),
            }
            .map_err(|e| e.to_string())?;
            Ok(Event::MobileNotice(result))
        });
    }
    fn start_mobile(&mut self) {
        let Some((path, _)) = &self.mobile_profile else {
            return;
        };
        let result = std::env::current_exe()
            .map_err(|e| e.to_string())
            .and_then(|exe| {
                Command::new(exe.parent().ok_or("Cannot locate CLI")?.join("freelane"))
                    .arg("mobile-start")
                    .arg(path)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(|e| e.to_string())
            });
        match result {
            Ok(mut child) => {
                let errors = Arc::new(Mutex::new(Vec::new()));
                let saved = Arc::clone(&errors);
                if let Some(mut stderr) = child.stderr.take() {
                    std::thread::spawn(move || {
                        let mut chunk = [0u8; 4096];
                        while let Ok(n) = stderr.read(&mut chunk) {
                            if n == 0 {
                                break;
                            }
                            if let Ok(mut bytes) = saved.lock() {
                                let keep = n.min((16 * 1024usize).saturating_sub(bytes.len()));
                                bytes.extend_from_slice(&chunk[..keep]);
                            }
                        }
                    });
                }
                self.mobile_errors = Some(errors);
                self.mobile_child = Some(child);
                self.mobile_notice = self
                    .language
                    .text("Start requested; refresh status to check boot completion.")
                    .into();
            }
            Err(e) => self.mobile_notice = e,
        }
    }
    pub fn poll_mobile(&mut self, ctx: &egui::Context) {
        if let Some(child) = &mut self.mobile_child {
            match child.try_wait() {
                Ok(Some(exit)) => {
                    let bytes = self
                        .mobile_errors
                        .take()
                        .and_then(|e| e.lock().ok().map(|b| b.clone()))
                        .unwrap_or_default();
                    self.mobile_notice = format!(
                        "Mobile supervisor: {exit}. {}",
                        String::from_utf8_lossy(&bytes)
                    );
                    self.mobile_child = None;
                }
                Ok(None) => ctx.request_repaint_after(Duration::from_millis(500)),
                Err(e) => self.mobile_notice = e.to_string(),
            }
        }
    }
    pub fn mobile_ui(&mut self, ctx: &egui::Context) {
        if !self.mobile_open {
            return;
        }
        let lang = self.language;
        let mut open = true;
        egui::Window::new(lang.text("Android & iOS"))
            .id(egui::Id::new("mobile-center"))
            .open(&mut open)
            .default_width(720.0)
            .default_height(650.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(560.0)
                    .show(ui, |ui| {
                        ui.label(lang.text("Mobile profile"));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.mobile_path)
                                .desired_width(f32::INFINITY)
                                .hint_text("/path/to/mobile.toml"),
                        );
                        if ui
                            .add_enabled(
                                self.pending.is_none()
                                    && self.mobile_child.is_none()
                                    && !self.mobile_path.trim().is_empty(),
                                egui::Button::new(lang.text("Open")),
                            )
                            .clicked()
                        {
                            self.load_mobile(ctx);
                        }
                        if let Some((_, profile)) = &self.mobile_profile {
                            let ios = profile.platform == Platform::Ios;
                            ui.heading(&profile.name);
                            ui.small(if ios {
                                "Apple Xcode Simulator"
                            } else {
                                "Android Emulator · ADB"
                            });
                            if let Some(a) = &profile.android {
                                ui.label(format!(
                                    "AVD: {} · {} CPU · {} MiB · {:?} / {:?}",
                                    a.avd, a.cpus, a.memory_mib, a.acceleration, a.gpu
                                ));
                            }
                            let busy = self.pending.is_some();
                            ui.horizontal_wrapped(|ui| {
                                if ui
                                    .add_enabled(
                                        !busy && self.mobile_child.is_none(),
                                        egui::Button::new(lang.text("Start")),
                                    )
                                    .clicked()
                                {
                                    self.start_mobile();
                                }
                                for (label, action) in [
                                    ("Refresh status", "status"),
                                    ("Available devices", "devices"),
                                    ("Shutdown", "stop"),
                                    ("Refresh display", "capture"),
                                ] {
                                    if ui
                                        .add_enabled(!busy, egui::Button::new(lang.text(label)))
                                        .clicked()
                                    {
                                        self.mobile_action(ctx, action);
                                    }
                                }
                                if ios
                                    && ui
                                        .add_enabled(
                                            !busy,
                                            egui::Button::new(lang.text("Open Simulator window")),
                                        )
                                        .clicked()
                                {
                                    self.mobile_action(ctx, "window");
                                }
                            });
                            ui.separator();
                            ui.label(lang.text("APK / Simulator .app path"));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.mobile_package)
                                    .desired_width(f32::INFINITY),
                            );
                            if ui
                                .add_enabled(
                                    !busy && !self.mobile_package.trim().is_empty(),
                                    egui::Button::new(lang.text("Install application")),
                                )
                                .clicked()
                            {
                                self.mobile_action(ctx, "install");
                            }
                            ui.label(lang.text("Android package/activity or iOS bundle ID"));
                            ui.add(
                                egui::TextEdit::singleline(&mut self.mobile_application)
                                    .desired_width(f32::INFINITY)
                                    .hint_text(if ios {
                                        "com.example.app"
                                    } else {
                                        "com.android.settings/.Settings"
                                    }),
                            );
                            if ui
                                .add_enabled(
                                    !busy && !self.mobile_application.trim().is_empty(),
                                    egui::Button::new(lang.text("Launch app")),
                                )
                                .clicked()
                            {
                                self.mobile_action(ctx, "launch");
                            }
                            if let Some(texture) = &self.mobile_texture {
                                ui.add(
                                    egui::Image::new(texture)
                                        .max_size(egui::vec2(ui.available_width(), 340.0)),
                                );
                            }
                        }
                        if self.pending.is_some() {
                            ui.spinner();
                        }
                        if !self.mobile_notice.is_empty() {
                            ui.label(&self.mobile_notice);
                        }
                    });
            });
        self.mobile_open = open;
    }
}
