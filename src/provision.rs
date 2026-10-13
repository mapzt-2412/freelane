//! Explicit creation of a fresh VM workspace. Does not boot or install a guest.
use crate::{qemu, Config, Error, GuestOs, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    CheckingInputs,
    InspectingMedia,
    PreparingWorkspace,
    ImportingImage,
    Configuring,
    Verifying,
    Publishing,
    Complete,
}
pub const MARKER: &str = "freelane-creation.json";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepared {
    version: u32,
    status: String,
    files: std::collections::BTreeMap<String, String>,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    #[default]
    InstallerIso,
    UbuntuCloudImage,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    #[serde(default)]
    pub source: Source,
    #[serde(default)]
    pub cloud_init_iso: Option<PathBuf>,
    pub name: String,
    pub guest_os: GuestOs,
    pub directory: PathBuf,
    pub installer: PathBuf,
    pub firmware: PathBuf,
    pub variable_template: PathBuf,
    pub qemu_img: PathBuf,
    pub accelerator: qemu::Accelerator,
    pub cpus: u16,
    pub memory_mib: u64,
    pub disk_gib: u64,
    pub arm64_media_confirmed: bool,
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub guest_agent: bool,
    #[serde(default)]
    pub shared_directory: Option<PathBuf>,
    #[serde(default)]
    pub share_mode: qemu::ShareMode,
}
impl Request {
    pub fn validate(&self) -> Result<()> {
        if !crate::diagnostics::executable(&self.qemu_img, Path::new(".")) {
            return Err(Error(
                "qemu-img is missing or not executable; install QEMU before creating a VM".into(),
            ));
        }
        Config {
            name: self.name.clone(),
            guest_os: self.guest_os,
            cpus: self.cpus,
            memory_mib: self.memory_mib,
            disk: "new.qcow2".into(),
            installer: None,
            runtime: None,
        }
        .validate()?;
        if self.directory.as_os_str().is_empty() || self.directory.exists() {
            return Err(Error(
                "Choose a new, nonexisting VM directory; existing directories are preserved".into(),
            ));
        }
        if let Some(folder) = &self.shared_directory {
            if !fs::metadata(folder)
                .map_err(|e| Error(format!("Shared folder: {e}")))?
                .is_dir()
            {
                return Err(Error("Shared folder must be an existing directory".into()));
            }
            if self.guest_os != GuestOs::Ubuntu && self.share_mode == qemu::ShareMode::Virtio9p {
                return Err(Error("Read/write 9p sharing requires Ubuntu".into()));
            }
        }
        if self.source == Source::UbuntuCloudImage && self.guest_os != GuestOs::Ubuntu {
            return Err(Error("Cloud images are supported only for Ubuntu".into()));
        }
        if self.source == Source::InstallerIso && self.cloud_init_iso.is_some() {
            return Err(Error(
                "NoCloud seed is available only with an Ubuntu cloud image".into(),
            ));
        }
        if let Some(seed) = &self.cloud_init_iso {
            let m = fs::metadata(seed).map_err(|e| Error(format!("NoCloud seed: {e}")))?;
            if !m.is_file() || m.len() == 0 || m.len() > 64 * 1024 * 1024 {
                return Err(Error(
                    "NoCloud seed must be a nonempty regular ISO of at most 64 MiB".into(),
                ));
            }
        }
        if !self.arm64_media_confirmed {
            return Err(Error("Confirm your selected installer is ARM64".into()));
        }
        if self.guest_os == GuestOs::Windows
            && (self.cpus < 2 || self.memory_mib < 4096 || self.disk_gib < 64)
        {
            return Err(Error(
                "Windows requires at least 2 CPUs, 4096 MiB RAM and a 64 GiB disk".into(),
            ));
        }
        let minimum = if self.guest_os == GuestOs::Windows {
            64
        } else {
            8
        };
        if self.disk_gib < minimum || self.disk_gib.checked_mul(1024 * 1024 * 1024).is_none() {
            return Err(Error(format!("Virtual disk must be at least {minimum} GiB and fit the backend size representation")));
        }
        if self.accelerator == qemu::Accelerator::Hvf && !crate::supported_host() {
            return Err(Error(
                "HVF requires Apple Silicon macOS; select TCG explicitly on other hosts".into(),
            ));
        }
        for (name, p) in [
            ("installer", &self.installer),
            ("firmware", &self.firmware),
            ("variable template", &self.variable_template),
        ] {
            let m = fs::metadata(p).map_err(|e| Error(format!("{name}: {e}")))?;
            if !m.is_file() || m.len() == 0 {
                return Err(Error(format!("{name} must be a nonempty regular file")));
            }
        }
        let firmware = fs::canonicalize(&self.firmware).map_err(|e| Error(e.to_string()))?;
        let vars = fs::canonicalize(&self.variable_template).map_err(|e| Error(e.to_string()))?;
        use std::os::unix::fs::MetadataExt;
        let f = fs::metadata(firmware).map_err(|e| Error(e.to_string()))?;
        let v = fs::metadata(vars).map_err(|e| Error(e.to_string()))?;
        if (f.dev(), f.ino()) == (v.dev(), v.ino()) {
            return Err(Error(
                "Firmware and variable template must be distinct files".into(),
            ));
        }
        // Empty qcow2 disks are sparse: virtual capacity is not an allocation request.
        let metadata = self
            .disk_gib
            .checked_mul(32)
            .ok_or_else(|| Error("VM storage estimate overflow".into()))?;
        let initial = f
            .len()
            .checked_add(v.len())
            .and_then(|n| n.checked_add(64 * 1024 * 1024))
            .and_then(|n| n.checked_add(metadata))
            .ok_or_else(|| Error("VM storage estimate overflow".into()))?;
        let required = if self.source == Source::UbuntuCloudImage {
            // Import needs a private source copy and room for converted source data.
            // Resizing to the selected virtual maximum does not allocate that maximum.
            let source_bytes = fs::metadata(&self.installer)
                .map_err(|e| Error(e.to_string()))?
                .len();
            initial
                .checked_add(crate::cloud_image::virtual_size(
                    &self.installer,
                    self.disk_gib,
                )?)
                .and_then(|n| n.checked_add(source_bytes))
                .ok_or_else(|| Error("VM storage estimate overflow".into()))?
        } else {
            initial
        };
        let destination = crate::storage::inspect_destination(&self.directory)?;
        if destination.available_bytes < required {
            return Err(Error(format!("VM storage insufficient: need {} MiB for initial creation/import, available {} MiB at {}. Select a destination with more free space. The virtual disk grows as data is written.",required/1024/1024,destination.available_bytes/1024/1024,destination.filesystem_path.display())));
        }
        Ok(())
    }
    #[cfg(test)]
    fn build_staged(&self, installer: PathBuf) -> Result<PathBuf> {
        self.build_staged_progress(installer, &mut |_| {})
    }
    #[cfg(test)]
    fn build_staged_progress(
        &self,
        installer: PathBuf,
        update: &mut impl FnMut(Stage),
    ) -> Result<PathBuf> {
        let anchor = crate::storage::ParentAnchor::open(&self.directory)?;
        self.build_with_anchor(installer, update, &anchor)
    }
    fn build_with_anchor(
        &self,
        installer: PathBuf,
        update: &mut impl FnMut(Stage),
        anchor: &crate::storage::ParentAnchor,
    ) -> Result<PathBuf> {
        anchor.verify(&self.directory)?;
        use std::{
            io::Write,
            os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        };
        let parent = fs::canonicalize(
            self.directory
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )
        .map_err(|e| Error(e.to_string()))?;
        let metadata = fs::metadata(&parent).map_err(|e| Error(e.to_string()))?;
        if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o022 != 0 {
            return Err(Error(
                "Choose a destination parent owned by you without group/world write access".into(),
            ));
        }
        let name = self
            .directory
            .file_name()
            .ok_or_else(|| Error("Choose a new directory name".into()))?;
        let final_path = parent.join(name);
        match fs::symlink_metadata(&final_path) {
            Ok(_) => return Err(Error("Existing destination is preserved".into())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(Error(e.to_string())),
        }
        let pending = parent.join(format!(
            ".freelane-create-{:016x}.pending",
            getrandom::u64().map_err(|e| Error(e.to_string()))?
        ));
        let mut published = false;
        let result = (|| {
            update(Stage::PreparingWorkspace);
            let path = qemu::initialize_guest_transaction(
                &pending,
                &self.firmware,
                &self.variable_template,
                &self.qemu_img,
                self.guest_os,
                self.disk_gib,
                qemu::Creation {
                    retain_marker: true,
                    anchor: Some(anchor),
                },
            )?;
            update(Stage::Configuring);
            let mut config =
                Config::parse(&fs::read_to_string(&path).map_err(|e| Error(e.to_string()))?)?;
            config.name = self.name.clone();
            config.cpus = self.cpus;
            config.memory_mib = self.memory_mib;
            config.installer = if self.source == Source::InstallerIso {
                Some(installer.clone())
            } else {
                update(Stage::ImportingImage);
                let imported = pending.join("cloud-import.qcow2");
                crate::cloud_image::import(&self.qemu_img, &installer, &imported, self.disk_gib)?;
                fs::rename(imported, pending.join(&config.disk))
                    .map_err(|e| Error(e.to_string()))?;
                self.cloud_init_iso
                    .as_ref()
                    .map(fs::canonicalize)
                    .transpose()
                    .map_err(|e| Error(e.to_string()))?
            };
            let runtime = config.runtime.as_mut().unwrap();
            runtime.accelerator = self.accelerator;
            runtime.network = self.network;
            runtime.guest_agent = self.guest_agent;
            runtime.shared_directory = self
                .shared_directory
                .as_ref()
                .map(fs::canonicalize)
                .transpose()
                .map_err(|e| Error(e.to_string()))?;
            runtime.share_mode = self.share_mode;
            let mut checked = config.clone();
            checked.runtime.as_mut().unwrap().accelerator = qemu::Accelerator::Tcg;
            qemu::creation_arguments(
                &checked,
                &pending,
                &qemu::runtime_dir(&path)?.join("qmp.sock"),
            )?;
            let text = toml::to_string_pretty(&config).map_err(|e| Error(e.to_string()))?;
            let temporary = pending.join("vm.toml.prepared");
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)
                .map_err(|e| Error(e.to_string()))?;
            file.write_all(text.as_bytes())
                .and_then(|_| file.sync_all())
                .map_err(|e| Error(e.to_string()))?;
            fs::rename(temporary, &path).map_err(|e| Error(e.to_string()))?;
            for name in ["firmware.fd", "nvram.fd", config.disk.to_str().unwrap()] {
                let file = pending.join(name);
                fs::set_permissions(&file, fs::Permissions::from_mode(0o600))
                    .map_err(|e| Error(e.to_string()))?;
                fs::File::open(file)
                    .and_then(|f| f.sync_all())
                    .map_err(|e| Error(e.to_string()))?;
            }
            update(Stage::Verifying);
            let mut files = std::collections::BTreeMap::new();
            for name in [
                "vm.toml",
                "firmware.fd",
                "nvram.fd",
                config.disk.to_str().unwrap(),
            ] {
                files.insert(
                    name.to_string(),
                    crate::checkpoint::hash(&pending.join(name))?,
                );
            }
            let prepared = Prepared {
                version: 1,
                status: "prepared".into(),
                files,
            };
            let temporary = pending.join("freelane-creation.prepared");
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)
                .map_err(|e| Error(e.to_string()))?;
            file.write_all(&serde_json::to_vec(&prepared).map_err(|e| Error(e.to_string()))?)
                .and_then(|_| file.sync_all())
                .map_err(|e| Error(e.to_string()))?;
            fs::rename(temporary, pending.join(MARKER)).map_err(|e| Error(e.to_string()))?;
            update(Stage::Publishing);
            anchor.publish(&pending, &final_path)?;
            published = true;
            anchor.remove_marker(&final_path)?;
            update(Stage::Complete);
            Ok(final_path.join("vm.toml"))
        })();
        result.map_err(|e: Error| {
            Error(format!(
                "Creation failed: {e}. Retained workspace: {}. Originals are preserved.",
                if published { &final_path } else { &pending }.display()
            ))
        })
    }
    pub fn create_with_progress(&self, mut update: impl FnMut(Stage)) -> Result<PathBuf> {
        update(Stage::CheckingInputs);
        self.validate()?;
        let anchor = crate::storage::ParentAnchor::open(&self.directory)?;
        update(Stage::InspectingMedia);
        match self.source {
            Source::InstallerIso => crate::media::verify_arm64_iso(&self.installer)?,
            Source::UbuntuCloudImage => {
                crate::cloud_image::validate(&self.installer, self.disk_gib)?
            }
        }
        let installer = fs::canonicalize(&self.installer).map_err(|e| Error(e.to_string()))?;
        self.build_with_anchor(installer, &mut update, &anchor)
    }
    pub fn create(&self) -> Result<PathBuf> {
        self.create_with_progress(|_| {})
    }
}
/// Load an explicit creation request; relative inputs resolve against its file, not process cwd.
pub fn load(path: &Path) -> Result<Request> {
    use std::{io::Read, os::unix::fs::OpenOptionsExt};
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| Error(e.to_string()))?;
    let metadata = file.metadata().map_err(|e| Error(e.to_string()))?;
    if !metadata.is_file() || metadata.len() > 1024 * 1024 {
        return Err(Error(
            "Creation request must be a regular file of at most 1 MiB".into(),
        ));
    }
    let mut text = String::new();
    file.take(1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|e| Error(e.to_string()))?;
    if text.len() > 1024 * 1024 {
        return Err(Error("Creation request grew beyond 1 MiB".into()));
    }
    let mut r: Request = toml::from_str(&text).map_err(|e| Error(e.to_string()))?;
    let base = path.parent().unwrap_or(Path::new("."));
    for p in [
        &mut r.directory,
        &mut r.installer,
        &mut r.firmware,
        &mut r.variable_template,
    ] {
        if p.is_relative() {
            *p = base.join(&*p);
        }
    }
    if let Some(seed) = &mut r.cloud_init_iso {
        if seed.is_relative() {
            *seed = base.join(&*seed);
        }
    }
    if let Some(folder) = &mut r.shared_directory {
        if folder.is_relative() {
            *folder = base.join(&*folder);
        }
    }
    if r.qemu_img.components().count() > 1 && r.qemu_img.is_relative() {
        r.qemu_img = base.join(&r.qemu_img);
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn request(root: &Path, failing: bool) -> Request {
        fs::create_dir(root).unwrap();
        fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
        for file in ["firmware.fd", "vars.fd", "installer.iso"] {
            fs::write(root.join(file), b"transaction fixture only").unwrap();
        }
        let tool = root.join("qemu-img");
        fs::write(
            &tool,
            if failing {
                "#!/bin/sh\nexit 19\n"
            } else {
                "#!/bin/sh\nprintf 'fake disk fixture' > \"$4\"\n"
            },
        )
        .unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
        Request {
            source: Source::InstallerIso,
            cloud_init_iso: None,
            name: "Transaction Ubuntu".into(),
            guest_os: GuestOs::Ubuntu,
            directory: root.join("new"),
            installer: root.join("installer.iso"),
            firmware: root.join("firmware.fd"),
            variable_template: root.join("vars.fd"),
            qemu_img: tool,
            accelerator: qemu::Accelerator::Tcg,
            cpus: 2,
            memory_mib: 2048,
            disk_gib: 8,
            arm64_media_confirmed: true,
            network: false,
            guest_agent: false,
            shared_directory: None,
            share_mode: qemu::ShareMode::ReadonlyUsb,
        }
    }
    #[test]
    fn rejected_request_reports_validation_without_false_completion() {
        let root =
            std::env::temp_dir().join(format!("fl-creation-progress-{}", std::process::id()));
        let mut r = request(&root, false);
        r.arm64_media_confirmed = false;
        let mut stages = Vec::new();
        assert!(r.create_with_progress(|stage| stages.push(stage)).is_err());
        assert_eq!(stages, vec![Stage::CheckingInputs]);
        assert!(!r.directory.exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn staging_component_publishes_only_completed_configuration() {
        let root = std::env::temp_dir().join(format!("fl-stage-success-{}", std::process::id()));
        let mut r = request(&root, false);
        r.network = true;
        r.guest_agent = true;
        let share = root.join("Tài liệu");
        fs::create_dir(&share).unwrap();
        fs::write(share.join("user.txt"), b"shared originals").unwrap();
        r.shared_directory = Some(share.clone());
        r.share_mode = qemu::ShareMode::Virtio9p;
        let mut stages = Vec::new();
        let result = r
            .build_staged_progress(r.installer.clone(), &mut |stage| stages.push(stage))
            .unwrap();
        assert_eq!(
            stages,
            vec![
                Stage::PreparingWorkspace,
                Stage::Configuring,
                Stage::Verifying,
                Stage::Publishing,
                Stage::Complete
            ]
        );
        let config = Config::parse(&fs::read_to_string(result).unwrap()).unwrap();
        assert_eq!(config.name, r.name);
        assert_eq!(config.cpus, 2);
        let runtime = config.runtime.as_ref().unwrap();
        assert!(runtime.network);
        assert!(runtime.guest_agent);
        assert_eq!(
            runtime.shared_directory,
            Some(fs::canonicalize(&share).unwrap())
        );
        assert_eq!(
            fs::read(share.join("user.txt")).unwrap(),
            b"shared originals"
        );
        assert_eq!(config.installer, Some(r.installer));
        assert!(!r.directory.join("freelane-creation.json").exists());
        assert!(!root.read_dir().unwrap().any(|p| p
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".freelane-create-")));
        assert_eq!(
            fs::read(r.variable_template).unwrap(),
            b"transaction fixture only"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn staging_component_retains_failed_state_without_publishing() {
        let root = std::env::temp_dir().join(format!("fl-stage-fail-{}", std::process::id()));
        let r = request(&root, true);
        assert!(r.build_staged(r.installer.clone()).is_err());
        assert!(!r.directory.exists());
        let pending = root
            .read_dir()
            .unwrap()
            .filter_map(|p| p.ok())
            .find(|p| {
                p.file_name()
                    .to_string_lossy()
                    .starts_with(".freelane-create-")
            })
            .unwrap();
        assert!(pending.path().join("freelane-creation.json").exists());
        assert!(recover_creation(&pending.path(), &root.join("recovery")).is_err());
        assert!(!root.join("recovery").exists());
        assert!(pending.path().exists());
        assert_eq!(
            fs::read(r.variable_template).unwrap(),
            b"transaction fixture only"
        );
        fs::remove_dir_all(root).unwrap();
    }
}

/// Finish an interrupted, prepared creation into a fresh destination.
#[derive(Debug, Serialize)]
pub struct Recovery {
    pub config: PathBuf,
    pub published: bool,
    pub startup_allowed: bool,
    pub warning: Option<String>,
}
pub fn recover_creation(source: &Path, destination: &Path) -> Result<Recovery> {
    use std::{
        io::Read,
        os::unix::fs::{MetadataExt, OpenOptionsExt},
    };
    let source = fs::canonicalize(source).map_err(|e| Error(e.to_string()))?;
    let metadata = fs::metadata(&source).map_err(|e| Error(e.to_string()))?;
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
        || metadata.mode() & 0o700 != 0o700
    {
        return Err(Error(
            "Recovery source must be a private read/write workspace owned by you".into(),
        ));
    }
    let marker = source.join(MARKER);
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&marker)
        .map_err(|_| {
            Error("No recoverable creation marker; preserve source and create a fresh VM".into())
        })?;
    let metadata = file.metadata().map_err(|e| Error(e.to_string()))?;
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.len() > 65_536 {
        return Err(Error("Invalid creation marker".into()));
    }
    let mut bytes = Vec::new();
    file.take(65_537)
        .read_to_end(&mut bytes)
        .map_err(|e| Error(e.to_string()))?;
    if bytes.len() > 65_536 {
        return Err(Error("Creation marker exceeds limit".into()));
    }
    let prepared: Prepared = serde_json::from_slice(&bytes).map_err(|_| {
        Error("Creation was not prepared; preserve it and retry with a new workspace".into())
    })?;
    if prepared.version != 1 || prepared.status != "prepared" || prepared.files.len() != 4 {
        return Err(Error("Creation is not recoverable at this stage".into()));
    }
    let disk = if prepared.files.contains_key("windows.qcow2") {
        "windows.qcow2"
    } else {
        "ubuntu.qcow2"
    };
    for name in ["vm.toml", "firmware.fd", "nvram.fd", disk] {
        let expected = prepared
            .files
            .get(name)
            .ok_or_else(|| Error("Incomplete creation manifest".into()))?;
        let path = source.join(name);
        let m = fs::symlink_metadata(&path).map_err(|e| Error(e.to_string()))?;
        if !m.is_file()
            || m.nlink() != 1
            || m.uid() != unsafe { libc::geteuid() }
            || (name == "vm.toml" && m.len() > 1024 * 1024)
        {
            return Err(Error("Recovery component is unsafe or oversized".into()));
        }
        if crate::checkpoint::hash(&path)? != *expected {
            return Err(Error("Recovery checksum mismatch; source preserved".into()));
        }
    }
    let config = Config::parse(
        &fs::read_to_string(source.join("vm.toml")).map_err(|e| Error(e.to_string()))?,
    )?;
    let runtime = config
        .runtime
        .as_ref()
        .ok_or_else(|| Error("Missing runtime".into()))?;
    if config.disk != Path::new(disk)
        || runtime.firmware != Path::new("firmware.fd")
        || runtime.nvram != Path::new("nvram.fd")
        || runtime
            .tpm
            .as_ref()
            .is_some_and(|t| t.state != Path::new("tpm"))
    {
        return Err(Error("Recovery configuration is not self-contained".into()));
    }
    let tpm = source.join("tpm");
    let m = fs::symlink_metadata(&tpm).map_err(|e| Error(e.to_string()))?;
    if !m.is_dir()
        || m.uid() != unsafe { libc::geteuid() }
        || fs::read_dir(&tpm)
            .map_err(|e| Error(e.to_string()))?
            .next()
            .is_some()
    {
        return Err(Error(
            "Creation TPM directory changed; preserve this workspace".into(),
        ));
    }
    let mut checked = config.clone();
    checked.runtime.as_mut().unwrap().accelerator = qemu::Accelerator::Tcg;
    qemu::creation_arguments(
        &checked,
        &source,
        &qemu::runtime_dir(&source.join("vm.toml"))?.join("qmp.sock"),
    )?;
    let parent = fs::canonicalize(
        destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )
    .map_err(|e| Error(e.to_string()))?;
    let m = fs::metadata(&parent).map_err(|e| Error(e.to_string()))?;
    if m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o022 != 0 {
        return Err(Error(
            "Recovery destination parent must be owned by you without group/world write access"
                .into(),
        ));
    }
    let target = parent.join(
        destination
            .file_name()
            .ok_or_else(|| Error("Choose a new destination name".into()))?,
    );
    if parent.starts_with(&source) {
        return Err(Error("Recovery destination cannot be inside source".into()));
    }
    // Atomic no-replace rename keeps originals intact when the destination collides.
    crate::storage::publish(&source, &target)?;
    Ok(finalize_recovery(&target))
}
fn finalize_recovery(target: &Path) -> Recovery {
    let warning=fs::remove_file(target.join(MARKER)).err().map(|e|format!("Publication committed; data retained at {} with a startup marker. Resolve directory permissions before finalizing: {e}",target.display()));
    Recovery {
        config: target.join("vm.toml"),
        published: true,
        startup_allowed: warning.is_none(),
        warning,
    }
}
#[cfg(test)]
mod recovery_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn committed_publication_reports_marker_failure_without_claiming_rollback() {
        let root =
            std::env::temp_dir().join(format!("fl-creation-finalization-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("vm.toml"), b"retained data").unwrap();
        // Simulates a marker replacement race after atomic publication.
        fs::create_dir(root.join(MARKER)).unwrap();
        let result = finalize_recovery(&root);
        assert!(result.published);
        assert!(!result.startup_allowed);
        assert!(result.warning.is_some());
        assert!(root.join(MARKER).exists());
        assert_eq!(fs::read(result.config).unwrap(), b"retained data");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn prepared_creation_can_recover_without_replacing_a_collision() {
        let root =
            std::env::temp_dir().join(format!("fl-creation-recovery-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        for name in ["code.fd", "vars.fd", "installer.iso"] {
            fs::write(root.join(name), b"recovery fixture only").unwrap();
        }
        let tool = root.join("qemu-img");
        fs::write(&tool,"#!/bin/sh\nprintf 'fake disk fixture' > \"$4\"\nmkdir \"$(dirname \"$4\")/../new\"\nprintf 'user data' > \"$(dirname \"$4\")/../new/user.txt\"\n").unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
        let r = Request {
            source: Source::InstallerIso,
            cloud_init_iso: None,
            name: "Recovered fixture".into(),
            guest_os: GuestOs::Ubuntu,
            directory: root.join("new"),
            installer: root.join("installer.iso"),
            firmware: root.join("code.fd"),
            variable_template: root.join("vars.fd"),
            qemu_img: tool,
            accelerator: qemu::Accelerator::Tcg,
            cpus: 2,
            memory_mib: 2048,
            disk_gib: 8,
            arm64_media_confirmed: true,
            network: false,
            guest_agent: false,
            shared_directory: None,
            share_mode: qemu::ShareMode::ReadonlyUsb,
        };
        assert!(r.build_staged(r.installer.clone()).is_err());
        let source = root
            .read_dir()
            .unwrap()
            .filter_map(|p| p.ok())
            .find(|p| {
                p.file_name()
                    .to_string_lossy()
                    .starts_with(".freelane-create-")
            })
            .unwrap()
            .path();
        assert!(recover_creation(&source, &root.join("new")).is_err());
        assert!(source.exists());
        let disk = source.join("ubuntu.qcow2");
        let original = fs::read(&disk).unwrap();
        fs::write(&disk, b"corrupted fixture").unwrap();
        assert!(recover_creation(&source, &root.join("recovered"))
            .unwrap_err()
            .to_string()
            .contains("checksum"));
        assert!(source.exists());
        assert!(!root.join("recovered").exists());
        fs::write(disk, original).unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o500)).unwrap();
        let attempt = recover_creation(&source, &root.join("readonly-recovery"));
        let preserved = source.exists();
        if preserved {
            fs::set_permissions(&source, fs::Permissions::from_mode(0o700)).unwrap();
        } else {
            fs::set_permissions(
                root.join("readonly-recovery"),
                fs::Permissions::from_mode(0o700),
            )
            .unwrap();
        }
        assert!(attempt.is_err());
        assert!(preserved, "Failed recovery moved its original source");
        assert!(!root.join("readonly-recovery").exists());
        let recovery = recover_creation(&source, &root.join("recovered")).unwrap();
        assert!(recovery.published && recovery.startup_allowed && recovery.warning.is_none());
        let result = recovery.config;
        assert!(result.exists());
        assert!(!source.exists());
        assert!(!result.parent().unwrap().join(MARKER).exists());
        assert_eq!(fs::read(root.join("new/user.txt")).unwrap(), b"user data");
        assert_eq!(
            fs::read(root.join("vars.fd")).unwrap(),
            b"recovery fixture only"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
