use super::*;
#[derive(Clone, Copy)]
pub enum Field {
    Directory,
    Installer,
    Firmware,
    Variables,
    QemuImg,
    SharedFolder,
}
pub struct Wizard {
    pub open: bool,
    components_missing: bool,
    refresh_requested: bool,
    install_requested: bool,
    windows: bool,
    cloud: bool,
    cloud_seed: String,
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
    network: bool,
    guest_agent: bool,
    shared_folder: String,
    share_mode: qemu::ShareMode,
}
impl Wizard {
    pub fn apply_tools(&mut self, firmware: String, variables: String, image: String) {
        self.components_missing = false;
        if self.firmware.is_empty() {
            self.firmware = firmware;
        }
        if self.variables.is_empty() {
            self.variables = variables;
        }
        if self.qemu_img.is_empty() || self.qemu_img == "qemu-img" {
            self.qemu_img = image;
        }
    }
    pub fn picked(&mut self, field: Field, value: String) {
        match field {
            Field::Directory => {
                self.directory = PathBuf::from(value)
                    .join("FreeLane VM")
                    .display()
                    .to_string()
            }
            Field::Installer => {
                self.installer = value;
                self.confirmed = false;
            }
            Field::Firmware => self.firmware = value,
            Field::Variables => self.variables = value,
            Field::QemuImg => self.qemu_img = value,
            Field::SharedFolder => self.shared_folder = value,
        }
    }
}
impl Default for Wizard {
    fn default() -> Self {
        let tools = freelane::host_tools::discover();
        Self {
            open: false,
            components_missing: tools.is_none(),
            refresh_requested: false,
            install_requested: false,
            windows: true,
            cloud: false,
            cloud_seed: String::new(),
            name: "Windows 11 ARM".into(),
            directory: String::new(),
            installer: String::new(),
            firmware: tools
                .as_ref()
                .map(|t| t.firmware.display().to_string())
                .unwrap_or_default(),
            variables: tools
                .as_ref()
                .map(|t| t.variables.display().to_string())
                .unwrap_or_default(),
            qemu_img: tools
                .map(|t| t.image.display().to_string())
                .unwrap_or_else(|| "qemu-img".into()),
            cpus: 4,
            memory: 4096,
            disk: 64,
            accelerator: if freelane::supported_host() {
                qemu::Accelerator::Hvf
            } else {
                qemu::Accelerator::Tcg
            },
            confirmed: false,
            network: true,
            guest_agent: true,
            shared_folder: String::new(),
            share_mode: qemu::ShareMode::ReadonlyUsb,
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
        let mut picker = None;
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
                                if self.wizard.windows {
                                    self.wizard.share_mode = qemu::ShareMode::ReadonlyUsb;
                                }
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
                            if self.wizard.windows {
                                self.wizard.cloud = false;
                            } else {
                                let previous = self.wizard.cloud;
                                ui.horizontal(|ui| {
                                    ui.selectable_value(
                                        &mut self.wizard.cloud,
                                        false,
                                        lang.text("Installer ISO"),
                                    );
                                    ui.selectable_value(
                                        &mut self.wizard.cloud,
                                        true,
                                        lang.text("Ubuntu cloud image"),
                                    );
                                });
                                if previous != self.wizard.cloud {
                                    self.wizard.confirmed = false;
                                }
                            }
                            ui.label(lang.text("VM name"));
                            ui.text_edit_singleline(&mut self.wizard.name);
                            for (label, value, field) in [
                                (
                                    "New VM directory",
                                    &mut self.wizard.directory,
                                    Field::Directory,
                                ),
                                (
                                    if self.wizard.cloud {
                                        "ARM64 cloud image (qcow2)"
                                    } else {
                                        "ARM64 installer ISO"
                                    },
                                    &mut self.wizard.installer,
                                    Field::Installer,
                                ),
                            ] {
                                ui.label(lang.text(label));
                                ui.horizontal(|ui| {
                                    let changed = ui.add_sized(
                                        egui::vec2((ui.available_width() - 120.0).max(100.0), 24.0),
                                        egui::TextEdit::singleline(value),
                                    ).changed();
                                    if changed && matches!(field, Field::Installer) { self.wizard.confirmed=false; }
                                    if ui.button(lang.text("Browse")).clicked() {
                                        picker = Some(field);
                                    }
                                });
                            }
                            if self.wizard.components_missing {
                                ui.label(lang.text("VM components required"));
                                if ui.button(lang.text("Find installed components")).clicked() {
                                    self.wizard.refresh_requested=true;
                                }
                                if cfg!(target_os="macos") {
                                    if brew_path().is_some() {
                                        if ui.button(lang.text("Install QEMU and TPM with Homebrew")).clicked() { self.wizard.install_requested=true; }
                                    } else {
                                        ui.hyperlink_to(lang.text("Install Homebrew first"),"https://brew.sh");
                                    }
                                }
                            }
                            ui.collapsing(lang.text("Advanced components"),|ui| {
                                for (label,value,field) in [
                                    ("Firmware code", &mut self.wizard.firmware,Field::Firmware),
                                    ("Variable template", &mut self.wizard.variables,Field::Variables),
                                    ("qemu-img", &mut self.wizard.qemu_img,Field::QemuImg),
                                ] {
                                    ui.label(lang.text(label));
                                    ui.horizontal(|ui| {ui.text_edit_singleline(value);if ui.button(lang.text("Browse")).clicked(){picker=Some(field);}});
                                }
                            });
                            if self.wizard.cloud {
                                ui.label(lang.text("NoCloud seed ISO (optional)")).on_hover_text(lang.text("Configure a login or SSH key in the seed. Seed contents are not verified."));
                                ui.text_edit_singleline(&mut self.wizard.cloud_seed);
                            }
                            ui.horizontal(|ui| {
                                for (label, cpus, memory, ubuntu_disk) in [
                                    ("Small", 2, 2048, 16),
                                    ("Standard", 4, 4096, 32),
                                    ("Large", 8, 8192, 64),
                                ] {
                                    if ui.button(lang.text(label)).clicked() {
                                        self.wizard.cpus = cpus;
                                        self.wizard.memory = if self.wizard.windows {
                                            memory.max(4096)
                                        } else {
                                            memory
                                        };
                                        self.wizard.disk = if self.wizard.windows {
                                            if label == "Large" {
                                                128
                                            } else {
                                                64
                                            }
                                        } else {
                                            ubuntu_disk
                                        };
                                    }
                                }
                            });
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
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.wizard.network, lang.text("Networking"));
                                ui.checkbox(
                                    &mut self.wizard.guest_agent,
                                    lang.text("Guest agent channel"),
                                );
                            });
                            ui.collapsing(lang.text("Shared folder"), |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_sized(
                                        egui::vec2((ui.available_width() - 120.0).max(100.0), 24.0),
                                        egui::TextEdit::singleline(&mut self.wizard.shared_folder),
                                    );
                                    if ui.button(lang.text("Browse")).clicked() {
                                        picker = Some(Field::SharedFolder);
                                    }
                                });
                                ui.horizontal_wrapped(|ui| {
                                    ui.selectable_value(
                                        &mut self.wizard.share_mode,
                                        qemu::ShareMode::ReadonlyUsb,
                                        lang.text("Read-only USB"),
                                    );
                                    ui.add_enabled_ui(!self.wizard.windows, |ui| {
                                        ui.selectable_value(
                                            &mut self.wizard.share_mode,
                                            qemu::ShareMode::Virtio9p,
                                            lang.text("Read/write 9p"),
                                        );
                                    });
                                });
                                if self.wizard.windows {
                                    self.wizard.share_mode = qemu::ShareMode::ReadonlyUsb;
                                }
                            });
                            ui.checkbox(
                                &mut self.wizard.confirmed,
                                lang.text(if self.wizard.cloud {
                                    "I confirm the cloud image is ARM64"
                                } else {
                                    "I confirm the installer is ARM64"
                                }),
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
                            ui.horizontal(|ui| {
                                ui.spinner();
                                if let Some(stage) = self.creation_stage {
                                    use freelane::provision::Stage;
                                    ui.label(lang.text(match stage {
                                        Stage::CheckingInputs => "Checking inputs",
                                        Stage::InspectingMedia => "Checking installer",
                                        Stage::PreparingWorkspace => "Preparing workspace",
                                        Stage::ImportingImage => "Importing cloud image",
                                        Stage::Configuring => "Saving configuration",
                                        Stage::Verifying => "Verifying files",
                                        Stage::Publishing => "Finishing workspace",
                                        Stage::Complete => "Workspace created",
                                    }));
                                }
                            });
                        }
                        if !self.notice.is_empty() {
                            ui.add(egui::Label::new(lang.message(&self.notice)).wrap());
                        }
                    });
            });
        self.wizard.open = open;
        if let Some(field) = picker {
            self.work(ctx, move || {
                file_picker::choose(matches!(field, Field::Directory | Field::SharedFolder))
                    .map(|path| Event::WizardPicked(field, path))
            });
        }
        if std::mem::take(&mut self.wizard.refresh_requested) {
            self.work(ctx, || {
                freelane::host_tools::discover()
                    .map(Event::WizardTools)
                    .ok_or_else(|| "VM components not found".into())
            });
        }
        if std::mem::take(&mut self.wizard.install_requested) {
            self.notice = "Installing VM components".into();
            self.work(ctx, || {
                install_components()?;
                freelane::host_tools::discover()
                    .map(Event::WizardTools)
                    .ok_or_else(|| "VM components not found".into())
            });
        }
        if create {
            let w = &self.wizard;
            let request = freelane::provision::Request {
                source: if w.cloud {
                    freelane::provision::Source::UbuntuCloudImage
                } else {
                    freelane::provision::Source::InstallerIso
                },
                cloud_init_iso: (w.cloud && !w.cloud_seed.trim().is_empty())
                    .then(|| PathBuf::from(w.cloud_seed.trim())),
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
                network: w.network,
                guest_agent: w.guest_agent,
                shared_directory: (!w.shared_folder.trim().is_empty())
                    .then(|| PathBuf::from(w.shared_folder.trim())),
                share_mode: w.share_mode,
            };
            self.notice.clear();
            let (tx, rx) = mpsc::channel();
            self.creation_progress = Some(rx);
            self.creation_stage = Some(freelane::provision::Stage::CheckingInputs);
            self.work(ctx, move || {
                request
                    .create_with_progress(|stage| {
                        let _ = tx.send(stage);
                    })
                    .map(Event::Created)
                    .map_err(|e| e.to_string())
            });
        }
    }
}

fn brew_path() -> Option<PathBuf> {
    ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
        .into_iter()
        .map(PathBuf::from)
        .find(|p| {
            use std::os::unix::fs::PermissionsExt;
            p.metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}
fn install_components() -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("Automatic component installation requires macOS".into());
    }
    let brew = brew_path().ok_or_else(|| "Install Homebrew first".to_string())?;
    freelane::host_tools::run_installer(
        Command::new(brew)
            .args(["install", "qemu", "swtpm", "7zip"])
            .env("HOMEBREW_NO_AUTO_UPDATE", "1"),
        Duration::from_secs(600),
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod setup_tests {
    use super::*;
    #[test]
    fn detected_tools_fill_empty_fields_without_replacing_manual_choices() {
        let mut w = Wizard {
            firmware: "/manual/code.fd".into(),
            variables: String::new(),
            qemu_img: "qemu-img".into(),
            ..Wizard::default()
        };
        w.apply_tools(
            "/auto/code.fd".into(),
            "/auto/vars.fd".into(),
            "/auto/qemu-img".into(),
        );
        assert_eq!(w.firmware, "/manual/code.fd");
        assert_eq!(w.variables, "/auto/vars.fd");
        assert_eq!(w.qemu_img, "/auto/qemu-img");
    }
}
