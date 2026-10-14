use super::*;
impl Desktop {
    pub fn transfer_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
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
        if !ui
            .add_enabled(
                self.pending.is_none(),
                egui::Button::new(lang.text("Send file to guest")),
            )
            .clicked()
        {
            return;
        }
        let Some(vm) = &self.vm else {
            return;
        };
        let config = vm.path.clone();
        let open = self.transfer_open_document;
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.transfer_cancel = Some(cancel.clone());
        let (tx, rx) = mpsc::channel();
        self.transfer_progress = Some(rx);
        self.transfer_status = None;
        let repaint = ctx.clone();
        self.work(ctx, move || {
            let result = (|| -> Result<String, String> {
                let Some(source) = file_picker::choose(false)? else {
                    return Ok(String::new());
                };
                freelane::file_transfer::send_options(
                    &config,
                    Path::new(&source),
                    open,
                    &|| cancel.load(std::sync::atomic::Ordering::Relaxed),
                    &mut |done, total| {
                        let _ = tx.send((done, total));
                        repaint.request_repaint();
                    },
                )
                .map_err(|e| e.to_string())?;
                Ok("File transferred and verified".into())
            })();
            Ok(Event::TransferDone(result.unwrap_or_else(|e| e)))
        });
    }
}
