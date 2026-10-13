use super::*;
impl Desktop {
    pub fn import_settings_ui(&mut self, ctx: &egui::Context) {
        if self.settings.is_none() {
            return;
        }
        self.work(ctx, move || {
            let result: WorkResult = (|| {
                let Some(path) = file_picker::choose(false)? else {
                    return Ok(Event::Notice(String::new()));
                };
                freelane::settings_bundle::load(Path::new(&path))
                    .map(Event::SettingsPreview)
                    .map_err(|e| e.to_string())
            })();
            Ok(result.unwrap_or_else(Event::Notice))
        });
    }
    pub fn export_settings_ui(&mut self, ctx: &egui::Context) {
        let Some(vm) = &self.vm else {
            return;
        };
        let config = vm.path.clone();
        self.work(ctx, move || {
            let result: WorkResult = (|| {
                let Some(path) = file_picker::save()? else {
                    return Ok(Event::Notice(String::new()));
                };
                let output = freelane::settings_bundle::export_saved(&config, Path::new(&path))
                    .map_err(|e| e.to_string())?;
                Ok(Event::Notice(format!(
                    "Settings exported: {}",
                    output.display()
                )))
            })();
            Ok(result.unwrap_or_else(Event::Notice))
        });
    }
}
