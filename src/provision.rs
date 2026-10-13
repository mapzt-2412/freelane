//! Explicit creation of a fresh VM workspace. Does not boot or install a guest.
use crate::{qemu, Config, Error, GuestOs, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
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
            return Err(Error("Confirm your selected installer is ARM64; automatic ISO architecture verification is not implemented".into()));
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
    pub fn create(&self) -> Result<PathBuf> {
        self.validate()?;
        let installer = fs::canonicalize(&self.installer).map_err(|e| Error(e.to_string()))?;
        let path = qemu::initialize_guest_sized(
            &self.directory,
            &self.firmware,
            &self.variable_template,
            &self.qemu_img,
            self.guest_os,
            self.disk_gib,
        )?;
        let original = fs::read_to_string(&path).map_err(|e| Error(e.to_string()))?;
        let mut config = Config::parse(&original)?;
        config.name = self.name.clone();
        config.cpus = self.cpus;
        config.memory_mib = self.memory_mib;
        config.installer = Some(installer);
        config.runtime.as_mut().unwrap().accelerator = self.accelerator;
        qemu::save_config(&path, &original, &config)?;
        fs::canonicalize(path).map_err(|e| Error(e.to_string()))
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
