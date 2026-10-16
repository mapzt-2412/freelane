use super::*;
impl Desktop {
    fn load_outbox_catalog(&mut self, ctx: &egui::Context) {
        let Some(vm) = &self.vm else {
            return;
        };
        let path = vm.path.clone();
        self.transfer_catalog = None;
        self.work(ctx, move || {
            let result = freelane::file_receive::catalog(&path).map_err(|e| e.to_string());
            Ok(Event::OutboxCatalog(path, result))
        });
    }
    pub(super) fn apply_outbox_catalog(
        &mut self,
        path: PathBuf,
        result: Result<freelane_agent_protocol::export_catalog::Page, String>,
    ) {
        if !self.vm.as_ref().is_some_and(|vm| vm.path == path) {
            return;
        }
        match result {
            Ok(page) => {
                self.transfer_catalog = Some((path, page));
                self.notice.clear();
            }
            Err(error) => {
                self.transfer_catalog = None;
                self.notice = error;
            }
        }
    }
    fn outbox_picker(&mut self, ctx: &egui::Context) {
        let lang = self.language;
        let mut selected = None;
        let mut open = true;
        let mut refresh = false;
        if let Some((path, page)) = &self.transfer_catalog {
            if !self.vm.as_ref().is_some_and(|vm| &vm.path == path) {
                self.transfer_catalog = None;
                return;
            }
            egui::Window::new(lang.text("Guest outbox"))
                .id(egui::Id::new("guest_outbox_picker"))
                .open(&mut open)
                .default_width(460.0)
                .default_pos(ctx.content_rect().center() - egui::vec2(230.0, 160.0))
                .resizable(true)
                .show(ctx, |ui| {
                    refresh = ui
                        .add_enabled(
                            self.pending.is_none(),
                            egui::Button::new(lang.text("Refresh list")),
                        )
                        .clicked();
                    if page.limited {
                        ui.label(lang.text("List limited; type a file name for other files"));
                    }
                    if page.files.is_empty() {
                        ui.label(lang.text("No files in guest outbox"));
                    }
                    egui::ScrollArea::vertical()
                        .max_height(320.0)
                        .show(ui, |ui| {
                            for file in &page.files {
                                ui.horizontal(|ui| {
                                    let width = (ui.available_width() - 100.0).max(80.0);
                                    if ui
                                        .add_sized(
                                            [width, 30.0],
                                            egui::Button::new(&file.name).truncate(),
                                        )
                                        .on_hover_text(&file.name)
                                        .clicked()
                                    {
                                        selected = Some(file.name.clone());
                                    }
                                    let size = if file.size < 1024 {
                                        format!("{} B", file.size)
                                    } else if file.size < 1024 * 1024 {
                                        format!("{:.1} KiB", file.size as f64 / 1024.0)
                                    } else {
                                        format!("{:.1} MiB", file.size as f64 / (1024.0 * 1024.0))
                                    };
                                    ui.weak(size);
                                });
                            }
                        });
                });
        }
        if !open {
            self.transfer_catalog = None;
        }
        if let Some(name) = selected {
            self.transfer_export_name = name;
            self.transfer_catalog = None;
        }
        if refresh {
            self.load_outbox_catalog(ctx);
        }
    }

    pub fn transfer_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.outbox_picker(ctx);
        let lang = self.language;
        if let Some(cancel) = &self.transfer_cancel {
            if let Some((done, total)) = self.transfer_status {
                let fraction = if total == 0 {
                    0.0
                } else {
                    done as f32 / total as f32
                };
                ui.add(egui::ProgressBar::new(fraction).show_percentage());
            }
            if ui.button(lang.text("Cancel transfer")).clicked() {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            return;
        }
        ui.checkbox(
            &mut self.transfer_open_document,
            lang.text("Open text document after transfer"),
        );
        let send = ui
            .add_enabled(
                self.pending.is_none(),
                egui::Button::new(lang.text("Send file to guest")),
            )
            .clicked();
        ui.separator();
        ui.label(lang.text("File name in guest outbox"));
        ui.text_edit_singleline(&mut self.transfer_export_name);
        if ui
            .add_enabled(
                self.pending.is_none(),
                egui::Button::new(lang.text("Choose guest file")),
            )
            .clicked()
        {
            self.load_outbox_catalog(ctx);
        }

        let receive = ui
            .add_enabled(
                self.pending.is_none()
                    && freelane_agent_protocol::export::valid_name(&self.transfer_export_name),
                egui::Button::new(lang.text("Receive file from guest")),
            )
            .clicked();
        self.file_folders_ui(ui, ctx);
        if !send && !receive {
            return;
        }
        let Some(vm) = &self.vm else {
            return;
        };
        let config = vm.path.clone();
        self.transfer_catalog = None;
        let open = self.transfer_open_document;
        let name = self.transfer_export_name.clone();
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.transfer_cancel = Some(cancel.clone());
        let (tx, rx) = mpsc::channel();
        self.transfer_progress = Some(rx);
        self.transfer_status = None;
        let repaint = ctx.clone();
        self.work(ctx, move || {
            let result = (|| -> Result<String, String> {
                let Some(source) = file_picker::choose(receive)? else {
                    return Ok(String::new());
                };
                let mut progress = |done, total| {
                    let _ = tx.send((done, total));
                    repaint.request_repaint();
                };
                if receive {
                    freelane::file_receive::receive(
                        &config,
                        &name,
                        Path::new(&source),
                        &|| cancel.load(std::sync::atomic::Ordering::Relaxed),
                        &mut progress,
                    )
                    .map_err(|e| e.to_string())?;
                } else {
                    freelane::file_transfer::send_options(
                        &config,
                        Path::new(&source),
                        open,
                        &|| cancel.load(std::sync::atomic::Ordering::Relaxed),
                        &mut progress,
                    )
                    .map_err(|e| e.to_string())?;
                }
                Ok("File transferred and verified".into())
            })();
            Ok(Event::TransferDone(result.unwrap_or_else(|e| e)))
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn late_file_inventory_cannot_replace_another_vms_selection_or_notice() {
        let config = Config {
            name: "current".into(),
            guest_os: GuestOs::Ubuntu,
            cpus: 1,
            memory_mib: 2048,
            disk: "disk.qcow2".into(),
            installer: None,
            runtime: None,
        };
        let mut desktop = Desktop {
            vm: Some(VmInfo {
                config,
                original: String::new(),
                path: "current.toml".into(),
                name: "current".into(),
                cpus: 1,
                memory: 2048,
                guest_agent: true,
                guest_os: GuestOs::Ubuntu,
                acceleration: "tcg",
            }),
            transfer_export_name: "current.txt".into(),
            notice: "keep".into(),
            ..Default::default()
        };
        let page = freelane_agent_protocol::export_catalog::Page {
            files: vec![],
            limited: false,
        };
        desktop.apply_outbox_catalog("old.toml".into(), Ok(page.clone()));
        assert!(desktop.transfer_catalog.is_none());
        assert_eq!(desktop.transfer_export_name, "current.txt");
        assert_eq!(desktop.notice, "keep");
        desktop.apply_outbox_catalog("current.toml".into(), Ok(page));
        assert!(desktop.transfer_catalog.is_some());
        let report = freelane_agent_protocol::transfer_folders::Report {
            inbox: freelane_agent_protocol::transfer_folders::State::Disabled,
            outbox: freelane_agent_protocol::transfer_folders::State::Unavailable,
            busy: false,
        };
        desktop.apply_folder_report("old.toml".into(), Ok(report.clone()));
        assert!(desktop.transfer_folder_status.is_none());
        desktop.apply_folder_report("current.toml".into(), Ok(report));
        assert!(desktop.transfer_folder_status.is_some());
    }
}
