//! Portable mutable settings; storage/firmware/TPM identity remains in the target VM.
use crate::{qemu, Config, Error, GuestOs, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub version: u32,
    pub guest_os: GuestOs,
    pub name: String,
    pub cpus: u16,
    pub memory_mib: u64,
    pub accelerator: qemu::Accelerator,
    pub graphics: qemu::Graphics,
    pub network: bool,
    pub network_device: qemu::NetworkDevice,
    pub guest_agent: bool,
    pub share_mode: qemu::ShareMode,
    pub installer: Option<PathBuf>,
    pub shared_directory: Option<PathBuf>,
}
fn read_regular(path: &Path) -> Result<String> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| Error(e.to_string()))?;
    let m = file.metadata().map_err(|e| Error(e.to_string()))?;
    if !m.is_file() || m.len() > 1024 * 1024 {
        return Err(Error(
            "Settings input must be a regular file of at most 1 MiB".into(),
        ));
    }
    let mut text = String::new();
    file.take(1024 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|e| Error(e.to_string()))?;
    if text.len() > 1024 * 1024 {
        return Err(Error("Settings input grew beyond 1 MiB".into()));
    }
    Ok(text)
}
pub fn load(path: &Path) -> Result<Bundle> {
    let bundle: Bundle = serde_json::from_str(&read_regular(path)?).map_err(|e| {
        Error(format!(
            "Invalid settings JSON at line {}, column {}",
            e.line(),
            e.column()
        ))
    })?;
    if bundle.version != 1 {
        return Err(Error("Unsupported settings version".into()));
    }
    Ok(bundle)
}
impl Bundle {
    pub fn apply(&self, target: &Config) -> Result<Config> {
        if self.version != 1 || self.guest_os != target.guest_os {
            return Err(Error(
                "Settings version/guest type does not match this VM".into(),
            ));
        }
        for path in [&self.installer, &self.shared_directory]
            .into_iter()
            .flatten()
        {
            if !path.is_absolute() {
                return Err(Error(
                    "Settings media and shared-folder references must be absolute paths".into(),
                ));
            }
        }
        let mut config = target.clone();
        config.name = self.name.clone();
        config.cpus = self.cpus;
        config.memory_mib = self.memory_mib;
        config.installer = self.installer.clone();
        let runtime = config
            .runtime
            .as_mut()
            .ok_or_else(|| Error("Configure the VM runtime before importing settings".into()))?;
        runtime.accelerator = self.accelerator;
        if self.accelerator == qemu::Accelerator::Hvf {
            runtime.tcg_threading = None;
        }
        runtime.graphics = self.graphics;
        runtime.network = self.network;
        runtime.network_device = self.network_device;
        runtime.guest_agent = self.guest_agent;
        runtime.share_mode = self.share_mode;
        runtime.shared_directory = self.shared_directory.clone();
        config.validate()?;
        Ok(config)
    }
}
pub fn export_saved(config_path: &Path, destination: &Path) -> Result<PathBuf> {
    let config_path = fs::canonicalize(config_path).map_err(|e| Error(e.to_string()))?;
    let config = Config::parse(&read_regular(&config_path)?)?;
    let runtime = config
        .runtime
        .as_ref()
        .ok_or_else(|| Error("Configure the VM runtime before exporting settings".into()))?;
    let base = config_path.parent().unwrap();
    let absolute = |path: &PathBuf| {
        if path.is_absolute() {
            path.clone()
        } else {
            base.join(path)
        }
    };
    let bundle = Bundle {
        version: 1,
        guest_os: config.guest_os,
        name: config.name,
        cpus: config.cpus,
        memory_mib: config.memory_mib,
        accelerator: runtime.accelerator,
        graphics: runtime.graphics,
        network: runtime.network,
        network_device: runtime.network_device,
        guest_agent: runtime.guest_agent,
        share_mode: runtime.share_mode,
        installer: config.installer.as_ref().map(absolute),
        shared_directory: runtime.shared_directory.as_ref().map(absolute),
    };
    let bytes = serde_json::to_vec_pretty(&bundle).map_err(|e| Error(e.to_string()))?;
    if bytes.len() > 1024 * 1024 {
        return Err(Error("Settings export exceeds 1 MiB".into()));
    }
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
            "Settings destination parent must be owned by you without group/world write access"
                .into(),
        ));
    }
    let target = parent.join(
        destination
            .file_name()
            .ok_or_else(|| Error("Choose a new settings filename".into()))?,
    );
    let pending = parent.join(format!(
        ".freelane-settings-{:016x}.pending",
        getrandom::u64().map_err(|e| Error(e.to_string()))?
    ));
    // Cleanup starts only after exclusive creation succeeds; foreign files are never removed.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&pending)
        .map_err(|e| Error(e.to_string()))?;
    let result = (|| {
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| Error(e.to_string()))?;
        crate::storage::publish(&pending, &target)?;
        Ok(target)
    })();
    if result.is_err() {
        let _ = fs::remove_file(pending);
    }
    result
}
pub fn import_saved(config_path: &Path, settings_path: &Path) -> Result<PathBuf> {
    let config_path = fs::canonicalize(config_path).map_err(|e| Error(e.to_string()))?;
    let original = read_regular(&config_path)?;
    let config = Config::parse(&original)?;
    let updated = load(settings_path)?.apply(&config)?;
    qemu::save_config(&config_path, &original, &updated)
}
