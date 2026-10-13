use super::*;
pub struct Wizard {
    pub open: bool,
    windows: bool,
    name: String,
    directory: String,
    installer: String,
    firmware: String,
    variables: String,
    qemu_img: String,
    cpus: u16,
    memory: u64,
    disk: u64,
    accelerator: qemu::Accelerator,
    confirmed: bool,
}
impl Default for Wizard {
    fn default() -> Self {
        Self {
            open: false,
            windows: true,
            name: "Windows 11 ARM".into(),
            directory: String::new(),
            installer: String::new(),
            firmware: String::new(),
            variables: String::new(),
            qemu_img: "qemu-img".into(),
            cpus: 4,
            memory: 4096,
            disk: 64,
            accelerator: if freelane::supported_host() {
                qemu::Accelerator::Hvf
            } else {
                qemu::Accelerator::Tcg
            },
            confirmed: false,
        }
    }
}
impl Desktop {
    pub fn wizard_ui(&mut self, ctx: &egui::Context) {
        if !self.wizard.open {
            return;
        }
        let lang = self.language;
        let mut open = true;
        let mut create = false;
        let busy = self.pending.is_some();
        egui::Window::new(lang.text("Create virtual machine"))
            .id(egui::Id::new("new-vm-wizard"))
            .open(&mut open)
            .default_width(600.0)
            .default_height(640.0)
            .max_height((ctx.content_rect().height() - 120.0).max(300.0))
            .default_pos(ctx.content_rect().center() - egui::vec2(300.0, 300.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(600.0)
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!busy, |ui| {
                            let previous_windows = self.wizard.windows;
                            ui.horizontal(|ui| {
                                ui.selectable_value(
                                    &mut self.wizard.windows,
                                    true,
                                    "Windows ARM64",
                                );
                                ui.selectable_value(
                                    &mut self.wizard.windows,
                                    false,
                                    "Ubuntu ARM64",
                                );
                            });
                            if previous_windows != self.wizard.windows {
                                let old_name = if previous_windows {
                                    "Windows 11 ARM"
                                } else {
                                    "Ubuntu ARM64"
                                };
                                if self.wizard.name == old_name {
                                    self.wizard.name = if self.wizard.windows {
                                        "Windows 11 ARM"
                                    } else {
                                        "Ubuntu ARM64"
                                    }
                                    .into();
                                }
                            }
                            for (label, value) in [
                                ("VM name", &mut self.wizard.name),
                                ("New VM directory", &mut self.wizard.directory),
                                ("ARM64 installer ISO", &mut self.wizard.installer),
                                ("Firmware code", &mut self.wizard.firmware),
                                ("Variable template", &mut self.wizard.variables),
                                ("qemu-img", &mut self.wizard.qemu_img),
                            ] {
                                ui.label(lang.text(label));
                                ui.add(
                                    egui::TextEdit::singleline(value).desired_width(f32::INFINITY),
                                );
                            }
                            ui.horizontal_wrapped(|ui| {
                                ui.label(lang.text("Virtual CPUs"));
                                ui.add(egui::DragValue::new(&mut self.wizard.cpus).range(1..=64));
                                ui.label("RAM (MiB)");
                                ui.add(
                                    egui::DragValue::new(&mut self.wizard.memory)
                                        .range(2048..=1_048_576),
                                );
                                ui.label(lang.text("Disk size (GiB)"));
                                ui.add(egui::DragValue::new(&mut self.wizard.disk).range(8..=4096));
                            });
                            ui.horizontal(|ui| {
                                ui.add_enabled_ui(freelane::supported_host(), |ui| {
                                    ui.selectable_value(
                                        &mut self.wizard.accelerator,
                                        qemu::Accelerator::Hvf,
                                        "HVF · Mac",
                                    );
                                });
                                ui.selectable_value(
                                    &mut self.wizard.accelerator,
                                    qemu::Accelerator::Tcg,
                                    "TCG",
                                );
                            });
                            ui.checkbox(
                                &mut self.wizard.confirmed,
                                lang.text("I confirm the installer is ARM64"),
                            );
                            create = ui
                                .add_enabled(
                                    self.wizard.confirmed
                                        && !self.wizard.directory.trim().is_empty(),
                                    egui::Button::new(lang.text("Create workspace")),
                                )
                                .clicked();
                        });
                        if busy {
                            ui.spinner();
                        }
                        if !self.notice.is_empty() {
                            ui.label(&self.notice);
                        }
                    });
            });
        self.wizard.open = open;
        if create {
            let w = &self.wizard;
            let request = freelane::provision::Request {
                name: w.name.trim().into(),
                guest_os: if w.windows {
                    GuestOs::Windows
                } else {
                    GuestOs::Ubuntu
                },
                directory: w.directory.trim().into(),
                installer: w.installer.trim().into(),
                firmware: w.firmware.trim().into(),
                variable_template: w.variables.trim().into(),
                qemu_img: w.qemu_img.trim().into(),
                accelerator: w.accelerator,
                cpus: w.cpus,
                memory_mib: w.memory,
                disk_gib: w.disk,
                arm64_media_confirmed: w.confirmed,
            };
            self.notice = lang.text("Create workspace").into();
            self.work(ctx, move || {
                request
                    .create()
                    .map(Event::Created)
                    .map_err(|e| e.to_string())
            });
        }
    }
}
