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
    storage_result: Option<Result<freelane::storage::Destination, String>>,
    storage_due: Option<Instant>,
    components_missing: bool,
    refresh_requested: bool,
    install_requested: bool,
    windows: bool,
    ubuntu_download: bool,
    ubuntu_version: setup::Ubuntu,
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
    pub fn begin(&mut self) {
        self.open = true;
        self.refresh_requested = false;
        self.storage_changed();
    }
    fn storage_changed(&mut self) {
        self.storage_result = None;
        self.storage_due = (!self.directory.trim().is_empty())
            .then(|| Instant::now() + Duration::from_millis(600));
    }
    pub fn storage_received(
        &mut self,
        path: &str,
        result: Result<freelane::storage::Destination, String>,
    ) {
        if path == self.directory.trim() {
            self.storage_result = Some(result);
        }
    }
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
                self.directory = value;
                self.storage_changed();
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
            storage_result: None,
            storage_due: None,
            components_missing: tools.is_none(),
            refresh_requested: false,
            install_requested: false,
            windows: true,
            ubuntu_download: true,
            ubuntu_version: setup::Ubuntu::default(),
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
        let directory_changed = false;
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
                                if self.wizard.disk==64 && !self.wizard.windows {self.wizard.disk=32;}
                                else if self.wizard.disk==32 && self.wizard.windows {self.wizard.disk=64;}
                                if self.wizard.name == old_name {
                                    self.wizard.name = if self.wizard.windows {
                                        "Windows 11 ARM"
                                    } else {
                                        "Ubuntu ARM64"
                                    }
                                    .into();
                                }
                            }
                            if self.wizard.windows { self.wizard.cloud=false; }
                            if !self.wizard.windows && self.wizard.ubuntu_download {
                                egui::ComboBox::from_id_salt("ubuntu-version").selected_text(self.wizard.ubuntu_version.label()).show_ui(ui, |ui| {
                                    for version in [setup::Ubuntu::Latest, setup::Ubuntu::Noble] {
                                        ui.selectable_value(&mut self.wizard.ubuntu_version,version,version.label());
                                    }
                                });
                            } else {
                                ui.label(lang.text("ARM64 installer ISO"));
                                ui.horizontal(|ui| {
                                    ui.text_edit_singleline(&mut self.wizard.installer);
                                    if ui.button(lang.text("Browse")).clicked() { picker=Some(Field::Installer); }
                                });
                            }
                            ui.label(lang.text("New VM directory"));
                            ui.horizontal(|ui| {
                                ui.label(if self.wizard.directory.is_empty() { lang.text("Automatic · Documents / FreeLane VMs") } else { &self.wizard.directory });
                                if ui.button(lang.text("Browse")).clicked() { picker=Some(Field::Directory); }
                            });
                            if let Some(result)=&self.wizard.storage_result {
                                match result {
                                    Ok(report)=>{
                                        ui.label(format!("{}: {:.1} GiB",lang.text("Free at destination"),report.available_bytes as f64/(1024.0*1024.0*1024.0))).on_hover_text(report.filesystem_path.display().to_string());
                                        if self.wizard.windows && report.available_bytes<32*1024*1024*1024 {
                                            ui.small(lang.text("Windows installation may need more free space."));
                                        }
                                    }
                                    Err(error)=>{ui.label(lang.message(error));}
                                }
                            }
                            ui.collapsing(lang.text("Advanced settings"), |ui| {
                            ui.label(lang.text("VM name"));
                            ui.text_edit_singleline(&mut self.wizard.name);
                            if !self.wizard.windows {
                                ui.checkbox(&mut self.wizard.ubuntu_download,lang.text("Download Ubuntu Server automatically"));
                                if !self.wizard.ubuntu_download {
                                    ui.checkbox(&mut self.wizard.cloud,lang.text("Ubuntu cloud image"));
                                } else { self.wizard.cloud=false; }
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
                            ui.small(lang.text("Disk grows as used on the selected storage."));
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
                                ui.label(lang.text("Virtual disk capacity (GiB)"));
                                ui.add(egui::DragValue::new(&mut self.wizard.disk).range(8..=u64::MAX/(1024*1024*1024)));
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
                            if self.wizard.cloud {
                                ui.checkbox(&mut self.wizard.confirmed,lang.text("I confirm the cloud image is ARM64"));
                            }
                            });
                            create=ui.add_enabled(
                                (!self.wizard.windows && self.wizard.ubuntu_download) || (!self.wizard.installer.trim().is_empty() && (!self.wizard.cloud || self.wizard.confirmed)),
                                egui::Button::new(lang.text("Set up and start"))
                            ).clicked();
                        });
                        if busy {
                            if let Some(progress)=&self.setup_status { ui.label(lang.message(progress)); }
                            if let Some(cancel)=&self.setup_cancel {
                                if ui.button(lang.text("Cancel setup")).clicked() { cancel.store(true,std::sync::atomic::Ordering::Relaxed); }
                            }
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
        if !open && busy {
            if let Some(cancel) = &self.setup_cancel {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        self.wizard.open = open;
        if directory_changed {
            self.wizard.storage_changed();
        }
        if let Some(field) = picker {
            let name = self.wizard.name.clone();
            self.work(ctx, move || {
                file_picker::choose(matches!(field, Field::Directory | Field::SharedFolder))
                    .and_then(|path| {
                        let path = if matches!(field, Field::Directory) {
                            path.map(|parent| {
                                freelane::storage::prepare_vm_parent(Path::new(&parent))
                                    .map_err(|e| e.to_string())
                                    .and_then(|parent| fresh_directory(&parent, &name))
                            })
                            .transpose()?
                        } else {
                            path
                        };
                        Ok(Event::WizardPicked(field, path))
                    })
            });
        }
        if !create && self.pending.is_none() && std::mem::take(&mut self.wizard.refresh_requested) {
            self.work(ctx, || {
                freelane::host_tools::discover()
                    .map(Event::WizardTools)
                    .ok_or_else(|| "VM components not found".into())
            });
        }
        if !create && self.pending.is_none() && std::mem::take(&mut self.wizard.install_requested) {
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
            let mut request = freelane::provision::Request {
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
                arm64_media_confirmed: !w.cloud || w.confirmed,
                network: w.network,
                guest_agent: w.guest_agent,
                shared_directory: (!w.shared_folder.trim().is_empty())
                    .then(|| PathBuf::from(w.shared_folder.trim())),
                share_mode: w.share_mode,
            };
            let download = (!w.windows && w.ubuntu_download).then_some(w.ubuntu_version);
            let (setup_tx, setup_rx) = mpsc::channel();
            let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
            self.setup_progress = Some(setup_rx);
            self.setup_cancel = Some(cancel.clone());
            self.setup_status = Some("Preparing installation".into());
            self.notice.clear();
            let (tx, rx) = mpsc::channel();
            self.creation_progress = Some(rx);
            self.creation_stage = Some(freelane::provision::Stage::CheckingInputs);
            self.work(ctx, move || {
                if freelane::host_tools::discover().is_none() {
                    setup::install_components(&cancel, &setup_tx)?;
                }
                if let Some(tools) = freelane::host_tools::discover() {
                    if request.firmware.as_os_str().is_empty() {
                        request.firmware = tools.firmware;
                    }
                    if request.variable_template.as_os_str().is_empty() {
                        request.variable_template = tools.variables;
                    }
                    if request.qemu_img == Path::new("qemu-img") {
                        request.qemu_img = tools.image;
                    }
                }
                if request.directory.as_os_str().is_empty() {
                    let home = std::env::var_os("HOME")
                        .ok_or_else(|| "Home directory unavailable".to_string())?;
                    let documents = PathBuf::from(home).join("Documents");
                    std::fs::create_dir_all(&documents).map_err(|e| e.to_string())?;
                    let parent = freelane::storage::prepare_vm_parent(&documents)
                        .map_err(|e| e.to_string())?;
                    request.directory = fresh_directory(&parent, &request.name)?.into();
                }
                if let Some(version) = download {
                    if cfg!(target_os = "macos") && !freelane::host_tools::binary("gpg").is_file() {
                        setup::install_components(&cancel, &setup_tx)?;
                    }
                    request.installer = setup::ubuntu_image(
                        version,
                        request
                            .directory
                            .parent()
                            .ok_or("Choose a VM destination folder")?,
                        &cancel,
                        &setup_tx,
                    )?;
                    request.source = freelane::provision::Source::InstallerIso;
                }
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    return Err("Setup cancelled".into());
                }
                let _ = setup_tx.send("Starting installer".into());
                request
                    .create_with_progress(|stage| {
                        let _ = tx.send(stage);
                    })
                    .map(Event::Created)
                    .map_err(|e| e.to_string())
            });
        }
        if !create && self.pending.is_none() {
            if let Some(due) = self.wizard.storage_due {
                if Instant::now() >= due {
                    self.wizard.storage_due = None;
                    let path = self.wizard.directory.trim().to_string();
                    self.work(ctx, move || {
                        let result = freelane::storage::inspect_destination(Path::new(&path))
                            .map_err(|e| e.to_string());
                        Ok(Event::WizardStorage(path, result))
                    });
                } else {
                    ctx.request_repaint_after(Duration::from_millis(200));
                }
            }
        }
    }
}

fn fresh_directory(parent: &Path, name: &str) -> Result<String, String> {
    let name: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .take(80)
        .collect();
    let name = if name.trim().is_empty() {
        "Virtual machine"
    } else {
        name.trim()
    };
    for index in 0..10000 {
        let target = parent.join(if index == 0 {
            name.to_string()
        } else {
            format!("{name}-{index}")
        });
        if !target.try_exists().map_err(|e| e.to_string())? {
            return Ok(target.display().to_string());
        }
    }
    Err("Choose a folder with fewer conflicting VM names".into())
}
pub(super) fn brew_path() -> Option<PathBuf> {
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
    let (tx, _rx) = mpsc::channel();
    setup::install_components(&Arc::new(std::sync::atomic::AtomicBool::new(false)), &tx)
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
    #[test]
    fn folder_selection_keeps_vm_on_selected_storage_and_preserves_existing_vm() {
        let root = std::env::temp_dir().join(format!("fl-wizard-folder-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("Windows 11 ARM")).unwrap();
        std::fs::write(root.join("Windows 11 ARM/user-data"), b"preserve").unwrap();
        let mut wizard = Wizard::default();
        wizard.picked(
            Field::Directory,
            fresh_directory(&root, &wizard.name).unwrap(),
        );
        assert_eq!(
            PathBuf::from(&wizard.directory),
            root.join("Windows 11 ARM-1")
        );
        assert!(!Path::new(&wizard.directory).exists());
        assert_eq!(
            std::fs::read(root.join("Windows 11 ARM/user-data")).unwrap(),
            b"preserve"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
