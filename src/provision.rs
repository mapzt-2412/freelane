//! Explicit creation of a fresh VM workspace. Does not boot or install a guest.
use crate::{qemu, Config, Error, GuestOs, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub const MARKER: &str = "freelane-creation.json";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepared {
    version: u32,
    status: String,
    files: std::collections::BTreeMap<String, String>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
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
        if !(minimum..=4096).contains(&self.disk_gib) {
            return Err(Error(format!("Disk must be {minimum}–4096 GiB")));
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
        // Check the nearest existing ancestor without creating any directories.
        let mut parent = self.directory.parent().unwrap_or(Path::new("."));
        loop {
            if parent.as_os_str().is_empty() {
                parent = Path::new(".");
            }
            match fs::metadata(parent) {
                Ok(m) if m.is_dir() => break,
                Ok(_) => return Err(Error("VM destination ancestor is not a directory".into())),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    parent = parent
                        .parent()
                        .ok_or_else(|| Error("No existing destination ancestor".into()))?;
                }
                Err(e) => return Err(Error(e.to_string())),
            }
        }
        let required = (self.disk_gib * 1024 * 1024 * 1024)
            .checked_add(f.len())
            .and_then(|n| n.checked_add(v.len()))
            .and_then(|n| n.checked_add(64 * 1024 * 1024))
            .ok_or_else(|| Error("VM storage estimate overflow".into()))?;
        let available = crate::storage::free_space(parent)?;
        if available < required {
            return Err(Error(format!("VM storage insufficient: require {} MiB for disk growth and firmware, available {} MiB. Choose a larger destination or a smaller disk. This check does not reserve space.", required / 1024 / 1024, available / 1024 / 1024)));
        }
        Ok(())
    }
    fn build_staged(&self, installer: PathBuf) -> Result<PathBuf> {
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
            let path = qemu::initialize_guest_transaction(
                &pending,
                &self.firmware,
                &self.variable_template,
                &self.qemu_img,
                self.guest_os,
                self.disk_gib,
                true,
            )?;
            let mut config =
                Config::parse(&fs::read_to_string(&path).map_err(|e| Error(e.to_string()))?)?;
            config.name = self.name.clone();
            config.cpus = self.cpus;
            config.memory_mib = self.memory_mib;
            config.installer = Some(installer);
            config.runtime.as_mut().unwrap().accelerator = self.accelerator;
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
            crate::storage::publish(&pending, &final_path)?;
            published = true;
            fs::remove_file(final_path.join(MARKER)).map_err(|e| Error(e.to_string()))?;
            Ok(final_path.join("vm.toml"))
        })();
        result.map_err(|e: Error| {
            Error(format!(
                "Creation failed: {e}. Retained workspace: {}. Originals are preserved.",
                if published { &final_path } else { &pending }.display()
            ))
        })
    }
    pub fn create(&self) -> Result<PathBuf> {
        self.validate()?;
        crate::media::verify_arm64_iso(&self.installer)?;
        let installer = fs::canonicalize(&self.installer).map_err(|e| Error(e.to_string()))?;
        self.build_staged(installer)
    }
}
/// Load an explicit creation request; relative inputs resolve against its file, not process cwd.
pub fn load(path: &Path) -> Result<Request> {
    if fs::metadata(path).map_err(|e| Error(e.to_string()))?.len() > 1024 * 1024 {
        return Err(Error("Creation request exceeds 1 MiB".into()));
    }
    let text = fs::read_to_string(path).map_err(|e| Error(e.to_string()))?;
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
        }
    }
    #[test]
    fn staging_component_publishes_only_completed_configuration() {
        let root = std::env::temp_dir().join(format!("fl-stage-success-{}", std::process::id()));
        let r = request(&root, false);
        let result = r.build_staged(r.installer.clone()).unwrap();
        let config = Config::parse(&fs::read_to_string(result).unwrap()).unwrap();
        assert_eq!(config.name, r.name);
        assert_eq!(config.cpus, 2);
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
pub fn recover_creation(source: &Path, destination: &Path) -> Result<PathBuf> {
    use std::{
        io::Read,
        os::unix::fs::{MetadataExt, OpenOptionsExt},
    };
    let source = fs::canonicalize(source).map_err(|e| Error(e.to_string()))?;
    let metadata = fs::metadata(&source).map_err(|e| Error(e.to_string()))?;
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(Error(
            "Recovery source must be a private workspace owned by you".into(),
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
        || runtime.shared_directory.is_some()
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
    fs::remove_file(target.join(MARKER)).map_err(|e| {
        Error(format!(
            "Recovered data retained at {} with marker: {e}",
            target.display()
        ))
    })?;
    Ok(target.join("vm.toml"))
}
#[cfg(test)]
mod recovery_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
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
        let result = recover_creation(&source, &root.join("recovered")).unwrap();
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
