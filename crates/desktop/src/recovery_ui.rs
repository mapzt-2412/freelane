//! Offline full-copy recovery; always restores into a new directory.
use super::*;
impl Desktop {
    pub fn recovery_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let lang = self.language;
        let busy = self.pending.is_some();
        if ui.button(lang.text("Back")).clicked() {
            self.view = interface::View::Display;
        }
        ui.heading(lang.text("Snapshots and backups"));
        ui.label(
            lang.text(
                "Stop the VM before saving. Restore creates a new VM and preserves originals.",
            ),
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.checkpoint_label)
                .hint_text(lang.text("Backup name")),
        );
        if ui
            .add_enabled(
                !busy && self.vm.is_some(),
                egui::Button::new(lang.text("Save full checkpoint")),
            )
            .clicked()
        {
            self.create_checkpoint(ctx);
        }
        ui.separator();
        ui.label(lang.text("Checkpoint folder"));
        if ui
            .text_edit_singleline(&mut self.checkpoint_source)
            .changed()
        {
            self.checkpoint_info = None;
        }
        if ui
            .add_enabled(!busy, egui::Button::new(lang.text("Choose checkpoint")))
            .clicked()
        {
            self.work(ctx, || {
                file_picker::choose(true).map(Event::CheckpointPicked)
            });
        }
        if ui
            .add_enabled(
                !busy && !self.checkpoint_source.is_empty(),
                egui::Button::new(lang.text("Verify backup")),
            )
            .clicked()
        {
            self.inspect_checkpoint(ctx);
        }
        if let Some(info) = &self.checkpoint_info {
            ui.group(|ui| {
                ui.strong(&info.label);
                ui.label(
                    info.created_utc
                        .as_deref()
                        .unwrap_or(lang.text("Unknown date")),
                );
                ui.label(format!(
                    "{} · {} {} · {:.1} MiB",
                    lang.text("Integrity checked"),
                    info.file_count,
                    lang.text("files"),
                    info.stored_bytes as f64 / (1024.0 * 1024.0)
                ));
            });
        }
        if ui
            .add_enabled(
                !busy && !self.checkpoint_source.is_empty(),
                egui::Button::new(lang.text("Restore as new VM")),
            )
            .clicked()
        {
            self.restore_checkpoint(ctx);
        }
    }
    pub fn inspect_checkpoint(&mut self, ctx: &egui::Context) {
        self.checkpoint_info = None;
        let source = self.checkpoint_source.clone();
        self.work(ctx, move || {
            let info = freelane::checkpoint::inspect(Path::new(&source)).map_err(|e| e.to_string());
            Ok(Event::CheckpointInfo(source, info))
        });
    }
    fn create_checkpoint(&mut self, ctx: &egui::Context) {
        let Some(vm) = &self.vm else {
            return;
        };
        let profile = vm.path.clone();
        let label = if self.checkpoint_label.trim().is_empty() {
            "Checkpoint".to_string()
        } else {
            self.checkpoint_label.trim().into()
        };
        let tool = vm
            .config
            .runtime
            .as_ref()
            .and_then(|r| r.binary.parent())
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| p.join("qemu-img"))
            .filter(|p| p.is_file())
            .or_else(|| freelane::host_tools::discover().map(|t| t.image))
            .unwrap_or_else(|| PathBuf::from("qemu-img"));
        self.work(ctx, move || {
            let result = (|| -> Result<Event, String> {
                let Some(parent) = file_picker::choose(true)? else {
                    return Ok(Event::Notice(String::new()));
                };
                let target = PathBuf::from(parent).join(format!(
                    "FreeLane-checkpoint-{}",
                    freelane::clipboard_source().map_err(|e| e.to_string())?
                ));
                let snapshot = freelane::checkpoint::create(&profile, &target, &label, &tool)
                    .map_err(|e| e.to_string())?;
                Ok(Event::CheckpointCreated(snapshot))
            })();
            Ok(result.unwrap_or_else(Event::Notice))
        });
    }
    fn restore_checkpoint(&mut self, ctx: &egui::Context) {
        let source = PathBuf::from(self.checkpoint_source.clone());
        self.work(ctx, move || {
            let result = (|| -> Result<Event, String> {
                let Some(parent) = file_picker::choose(true)? else {
                    return Ok(Event::Notice(String::new()));
                };
                let target = PathBuf::from(parent).join(format!(
                    "FreeLane-restored-{}",
                    freelane::clipboard_source().map_err(|e| e.to_string())?
                ));
                let profile =
                    freelane::checkpoint::restore(&source, &target).map_err(|e| e.to_string())?;
                // Do not auto-boot a restored guest. Preserve and expose its actual output path.
                let config = freelane::Config::parse(
                    &std::fs::read_to_string(&profile).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                let library = freelane::library::default_path().map_err(|e| e.to_string())?;
                freelane::library::record(
                    &library,
                    freelane::library::Entry {
                        path: profile.clone(),
                        name: config.name,
                        guest_os: config.guest_os,
                    },
                )
                .map_err(|e| {
                    format!(
                        "Restored files retained at {}; library update failed: {e}",
                        profile.display()
                    )
                })?;
                Ok(Event::CheckpointRestored(
                    profile,
                    freelane::library::read(&library).map_err(|e| e.to_string())?,
                ))
            })();
            Ok(result.unwrap_or_else(Event::Notice))
        });
    }
}
