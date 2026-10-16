//! Explicit inspection and recovery of startup-unavailable guest transfer folders.
use super::*;
impl Desktop {
    fn load_folder_report(&mut self, ctx: &egui::Context, reconnect: bool) {
        let Some(vm) = &self.vm else {
            return;
        };
        let path = vm.path.clone();
        if reconnect {
            self.transfer_catalog = None;
        }
        self.work(ctx, move || {
            let result =
                freelane::file_receive::folders(&path, reconnect).map_err(|e| e.to_string());
            Ok(Event::TransferFolders(path, result))
        });
    }
    pub(super) fn apply_folder_report(
        &mut self,
        path: PathBuf,
        result: Result<freelane_agent_protocol::transfer_folders::Report, String>,
    ) {
        if !self.vm.as_ref().is_some_and(|vm| vm.path == path) {
            return;
        }
        match result {
            Ok(report) => {
                self.transfer_folder_status = Some((path, report));
                self.notice.clear();
            }
            Err(error) => {
                self.transfer_folder_status = None;
                self.notice = error;
            }
        }
    }
    pub(super) fn file_folders_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        use freelane_agent_protocol::transfer_folders::State;
        let lang = self.language;
        let mut reconnect = false;
        if self
            .transfer_folder_status
            .as_ref()
            .is_some_and(|(path, _)| !self.vm.as_ref().is_some_and(|vm| &vm.path == path))
        {
            self.transfer_folder_status = None;
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    self.pending.is_none(),
                    egui::Button::new(lang.text("Check guest folders")),
                )
                .clicked()
            {
                self.load_folder_report(ctx, false);
            }
            if let Some((_, report)) = &self.transfer_folder_status {
                let label = |state| {
                    lang.text(match state {
                        State::Ready => "Folder ready",
                        State::Disabled => "Folder not selected",
                        State::Unavailable => "Folder unavailable",
                    })
                };
                ui.label(format!(
                    "{}: {} · {}: {}",
                    lang.text("Inbox"),
                    label(report.inbox),
                    lang.text("Outbox"),
                    label(report.outbox)
                ));
                if report.inbox == State::Unavailable || report.outbox == State::Unavailable {
                    reconnect = ui
                        .add_enabled(
                            self.pending.is_none() && !report.busy,
                            egui::Button::new(lang.text("Reconnect guest folders")),
                        )
                        .clicked();
                }
                if report.busy {
                    ui.weak(lang.text("File transfer in progress"));
                }
            }
        });
        if reconnect {
            self.load_folder_report(ctx, true);
        }
    }
}
