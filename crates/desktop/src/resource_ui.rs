//! Resource presets only edit the existing settings draft; normal save owns persistence.
use super::*;
pub fn presets(
    ui: &mut egui::Ui,
    config: &mut Config,
    capacity: Option<freelane::resource_presets::HostCapacity>,
    lang: Language,
) {
    use freelane::resource_presets::{suggest, Preset};
    let Some(host) = capacity else {
        ui.small(lang.text("Resource suggestions unavailable; set CPU and RAM below."));
        return;
    };
    ui.small(format!(
        "{}: {} CPU · {} MiB",
        lang.text("Host capacity"),
        host.cpus,
        host.memory_mib
    ));
    ui.horizontal_wrapped(|ui| {
        for (label, preset) in [
            ("Battery", Preset::Battery),
            ("Balanced", Preset::Balanced),
            ("Performance", Preset::Performance),
        ] {
            let allocation = suggest(host, config.guest_os, preset).ok();
            let label = allocation
                .map(|a| {
                    format!(
                        "{} · {} CPU / {} MiB",
                        lang.text(label),
                        a.cpus,
                        a.memory_mib
                    )
                })
                .unwrap_or_else(|| lang.text(label).into());
            if ui
                .add_enabled(allocation.is_some(), egui::Button::new(label))
                .clicked()
            {
                let allocation = allocation.unwrap();
                config.cpus = allocation.cpus;
                config.memory_mib = allocation.memory_mib;
            }
        }
    });
    ui.small(lang.text("Allocation suggestions only; battery life and speed depend on workload."));
}
