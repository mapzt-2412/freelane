use super::*;
impl Desktop {
    pub fn pick_shared_folder(&mut self, ctx: &egui::Context) {
        self.work(ctx, || file_picker::choose(true).map(Event::SharedPicked));
    }
    pub fn inspect_shared_folder(&mut self, ctx: &egui::Context) {
        let Some(vm) = &self.vm else {
            return;
        };
        let base = vm.path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let Some(mut config) = self.settings.clone() else {
            return;
        };
        let selected = self.shared_path.clone();
        let Some(runtime) = config.runtime.as_mut() else {
            return;
        };
        runtime.shared_directory =
            (!selected.trim().is_empty()).then(|| PathBuf::from(selected.trim()));
        let mode = runtime.share_mode;
        self.work(ctx, move || {
            Ok(Event::SharedReport(
                selected,
                mode,
                freelane::sharing::inspect(&config, &base),
            ))
        });
    }
}
