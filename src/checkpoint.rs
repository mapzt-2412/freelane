//! Offline full-copy checkpoints. Original guest files are never replaced or modified.
use crate::{
    qemu,
    storage::{free_space, publish},
    Config, Error, Result,
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
pub const MARKER: &str = "freelane-checkpoint.json";
fn error(e: impl std::fmt::Display) -> Error {
    Error(e.to_string())
}
fn command(tool: &Path, args: &[&str], paths: &[&Path]) -> Result<std::process::Output> {
    let mut c = Command::new(tool);
    c.args(args);
    for p in paths {
        c.arg(p);
    }
    let output = c.output().map_err(error)?;
    if !output.status.success() {
        return Err(Error(format!(
            "Checkpoint tool failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(1000)
                .collect::<String>()
        )));
    }
    Ok(output)
}
fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir(path).map_err(error)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(error)
}
pub(crate) fn hash(path: &Path) -> Result<String> {
    let out = if cfg!(target_os = "macos") {
        command(Path::new("/usr/bin/shasum"), &["-a", "256"], &[path])?
    } else {
        command(Path::new("sha256sum"), &[], &[path])?
    };
    let value = String::from_utf8(out.stdout)
        .map_err(error)?
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_owned();
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error("Invalid checkpoint SHA256 result".into()));
    }
    Ok(value)
}
fn owned_source(base: &Path, path: &Path) -> Result<PathBuf> {
    let path = fs::canonicalize(base.join(path)).map_err(error)?;
    if !path.starts_with(base) {
        return Err(Error(
            "Checkpoint sources must be contained in the VM workspace".into(),
        ));
    }
    let m = fs::metadata(&path).map_err(error)?;
    if !m.is_file() || m.nlink() != 1 {
        return Err(Error(
            "Checkpoint requires regular, unshared source files".into(),
        ));
    }
    Ok(path)
}
fn copy_file(
    source: &Path,
    target: &Path,
    root: &Path,
    hashes: &mut BTreeMap<String, String>,
) -> Result<()> {
    if fs::symlink_metadata(target).is_ok() {
        return Err(Error(
            "Checkpoint/restore destination filenames collide; incomplete copy discarded".into(),
        ));
    }
    let before = hash(source)?;
    fs::copy(source, target).map_err(error)?;
    fs::set_permissions(target, fs::Permissions::from_mode(0o600)).map_err(error)?;
    fs::File::open(target)
        .map_err(error)?
        .sync_all()
        .map_err(error)?;
    if hash(target)? != before || hash(source)? != before {
        return Err(Error(
            "Source changed during checkpoint; incomplete copy discarded".into(),
        ));
    }
    hashes.insert(
        target
            .strip_prefix(root)
            .map_err(error)?
            .to_string_lossy()
            .into_owned(),
        before,
    );
    Ok(())
}
fn copy_tpm(
    source: &Path,
    target: &Path,
    root: &Path,
    hashes: &mut BTreeMap<String, String>,
    depth: usize,
    count: &mut usize,
    total_bytes: &mut u64,
) -> Result<()> {
    if depth > 8 || !fs::symlink_metadata(source).map_err(error)?.is_dir() {
        return Err(Error("Unsupported TPM directory or symlink".into()));
    }
    private_dir(target)?;
    for entry in fs::read_dir(source).map_err(error)? {
        let entry = entry.map_err(error)?;
        let kind = entry.file_type().map_err(error)?;
        *count += 1;
        if *count > 1024 {
            return Err(Error("TPM file limit exceeded".into()));
        }
        let to = target.join(entry.file_name());
        if kind.is_dir() {
            copy_tpm(
                &entry.path(),
                &to,
                root,
                hashes,
                depth + 1,
                count,
                total_bytes,
            )?;
        } else if kind.is_file() {
            let m = entry.metadata().map_err(error)?;
            *total_bytes = total_bytes.saturating_add(m.len());
            if m.nlink() != 1 || m.len() > 16 * 1024 * 1024 || *total_bytes > 64 * 1024 * 1024 {
                return Err(Error("Unsupported TPM file".into()));
            }
            copy_file(&entry.path(), &to, root, hashes)?;
        } else {
            return Err(Error(
                "TPM symlinks and special files are unsupported".into(),
            ));
        }
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u16,
    label: String,
    created_unix_seconds: u64,
    kind: String,
    guest_runtime_verified: bool,
    files_sha256: BTreeMap<String, String>,
}
struct Pending {
    path: PathBuf,
    active: bool,
}
impl Drop for Pending {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub fn create(config_path: &Path, output: &Path, label: &str, qemu_img: &Path) -> Result<PathBuf> {
    if label.is_empty() || label.len() > 128 || label.chars().any(char::is_control) {
        return Err(Error(
            "Checkpoint label must be 1–128 bytes without control characters".into(),
        ));
    }
    if fs::symlink_metadata(output).is_ok() {
        return Err(Error(
            "Checkpoint output already exists; original data preserved".into(),
        ));
    }
    let config_path = fs::canonicalize(config_path).map_err(error)?;
    let base = config_path.parent().unwrap();
    if base.join(MARKER).exists() {
        return Err(Error(
            "Cannot checkpoint an immutable checkpoint workspace".into(),
        ));
    }
    let runtime_dir = qemu::runtime_dir(&config_path)?;
    match fs::create_dir(&runtime_dir) {
        Ok(()) => {
            fs::set_permissions(&runtime_dir, fs::Permissions::from_mode(0o700)).map_err(error)?
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(error(e)),
    }
    let m = fs::symlink_metadata(&runtime_dir).map_err(error)?;
    if !m.is_dir() || m.uid() != unsafe { libc::getuid() } || m.mode() & 0o077 != 0 {
        return Err(Error(
            "VM runtime directory must be private and owned by this user".into(),
        ));
    }
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(runtime_dir.join("lock"))
        .map_err(error)?;
    lock.try_lock_exclusive()
        .map_err(|_| Error("Stop the VM before creating a checkpoint".into()))?;
    if fs::metadata(&config_path).map_err(error)?.len() > 1024 * 1024 {
        return Err(Error("VM configuration exceeds 1 MiB".into()));
    }
    let original = fs::read_to_string(&config_path).map_err(error)?;
    let mut config = Config::parse(&original)?;
    let r = config
        .runtime
        .as_ref()
        .ok_or_else(|| Error("Runtime settings are required".into()))?;
    if !matches!(r.disk_format, qemu::DiskFormat::Qcow2) {
        return Err(Error(
            "Checkpoint currently supports qcow2 guest disks only".into(),
        ));
    }
    let mut structural = config.clone();
    structural.installer = None;
    structural.runtime.as_mut().unwrap().accelerator = qemu::Accelerator::Tcg;
    qemu::arguments(&structural, base, &runtime_dir.join("qmp.sock"))?;
    let disk = owned_source(base, &config.disk)?;
    let firmware = owned_source(base, &r.firmware)?;
    let vars = owned_source(base, &r.nvram)?;
    let tpm = r
        .tpm
        .as_ref()
        .map(|t| fs::canonicalize(base.join(&t.state)).map_err(error))
        .transpose()?;
    if tpm.as_ref().is_some_and(|p| !p.starts_with(base)) {
        return Err(Error("TPM state must be inside the VM workspace".into()));
    }
    let parent = fs::canonicalize(
        output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )
    .map_err(error)?;
    if tpm.as_ref().is_some_and(|p| parent.starts_with(p)) {
        return Err(Error(
            "Checkpoint output cannot be inside source TPM state".into(),
        ));
    }
    let parent_metadata = fs::metadata(&parent).map_err(error)?;
    if parent_metadata.uid() != unsafe { libc::getuid() } || parent_metadata.mode() & 0o022 != 0 {
        return Err(Error(
            "Use a destination parent owned by this user without group/world write access".into(),
        ));
    }
    let final_path = parent.join(
        output
            .file_name()
            .ok_or_else(|| Error("Checkpoint directory name required".into()))?,
    );
    standalone_disk(&disk)?;
    let info = command(qemu_img, &["info", "--output=json"], &[&disk])?;
    let info: serde_json::Value = serde_json::from_slice(&info.stdout).map_err(error)?;
    let virtual_bytes = info["virtual-size"]
        .as_u64()
        .ok_or_else(|| Error("Unknown guest virtual disk size".into()))?;
    let required = virtual_bytes
        .saturating_add(fs::metadata(&firmware).map_err(error)?.len())
        .saturating_add(fs::metadata(&vars).map_err(error)?.len())
        .saturating_add(64 * 1024 * 1024);
    let free = free_space(&parent)?;
    if free < required {
        return Err(Error(format!("Checkpoint storage insufficient: require conservative {} MiB, available {} MiB. Use a destination with more space.",required/1024/1024,free/1024/1024)));
    }
    let pending = parent.join(format!(
        ".freelane-checkpoint-{:016x}.pending",
        getrandom::u64().map_err(error)?
    ));
    private_dir(&pending)?;
    let mut guard = Pending {
        path: pending.clone(),
        active: true,
    };
    fs::write(pending.join(MARKER), b"incomplete").map_err(error)?;
    let mut hashes = BTreeMap::new();
    let copied_disk = pending.join("disk.qcow2");
    command(
        qemu_img,
        &["convert", "-f", "qcow2", "-O", "qcow2"],
        &[&disk, &copied_disk],
    )?;
    fs::set_permissions(&copied_disk, fs::Permissions::from_mode(0o600)).map_err(error)?;
    command(
        qemu_img,
        &["compare", "-f", "qcow2", "-F", "qcow2"],
        &[&disk, &copied_disk],
    )?;
    fs::File::open(&copied_disk)
        .map_err(error)?
        .sync_all()
        .map_err(error)?;
    hashes.insert("disk.qcow2".into(), hash(&copied_disk)?);
    copy_file(
        &firmware,
        &pending.join("firmware.fd"),
        &pending,
        &mut hashes,
    )?;
    copy_file(&vars, &pending.join("nvram.fd"), &pending, &mut hashes)?;
    if let Some(tpm) = &tpm {
        copy_tpm(
            tpm,
            &pending.join("tpm"),
            &pending,
            &mut hashes,
            0,
            &mut 0,
            &mut 0,
        )?;
    }
    config.disk = "disk.qcow2".into();
    config.installer = None;
    let r = config.runtime.as_mut().unwrap();
    r.firmware = "firmware.fd".into();
    r.nvram = "nvram.fd".into();
    if let Some(t) = &mut r.tpm {
        t.state = "tpm".into();
    }
    fs::write(
        pending.join("vm.toml"),
        toml::to_string_pretty(&config).map_err(error)?,
    )
    .map_err(error)?;
    hashes.insert("vm.toml".into(), hash(&pending.join("vm.toml"))?);
    let manifest = Manifest {
        version: 1,
        label: label.into(),
        created_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(error)?
            .as_secs(),
        kind: "stopped_workspace_copy".into(),
        guest_runtime_verified: false,
        files_sha256: hashes,
    };
    fs::write(
        pending.join(MARKER),
        serde_json::to_vec_pretty(&manifest).map_err(error)?,
    )
    .map_err(error)?;
    for file in [pending.join(MARKER), pending.join("vm.toml")] {
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).map_err(error)?;
        fs::File::open(file)
            .map_err(error)?
            .sync_all()
            .map_err(error)?;
    }
    if fs::read_to_string(&config_path).map_err(error)? != original
        || fs::symlink_metadata(&final_path).is_ok()
    {
        return Err(Error(
            "Configuration/output changed during checkpoint; incomplete copy discarded".into(),
        ));
    }
    for name in manifest.files_sha256.keys() {
        fs::set_permissions(pending.join(name), fs::Permissions::from_mode(0o400))
            .map_err(error)?;
    }
    fs::set_permissions(pending.join(MARKER), fs::Permissions::from_mode(0o400)).map_err(error)?;
    publish(&pending, &final_path)?;
    guard.active = false;
    Ok(final_path)
}

fn standalone_disk(path: &Path) -> Result<()> {
    use std::io::Read;
    let mut header = [0u8; 104];
    fs::File::open(path)
        .map_err(error)?
        .read_exact(&mut header)
        .map_err(error)?;
    let version = u32::from_be_bytes(header[4..8].try_into().unwrap());
    if &header[..4] != b"QFI\xfb" || !matches!(version, 2 | 3) {
        return Err(Error(
            "Checkpoint requires a valid standalone qcow2 header".into(),
        ));
    }
    if u64::from_be_bytes(header[8..16].try_into().unwrap()) != 0
        || u32::from_be_bytes(header[16..20].try_into().unwrap()) != 0
    {
        return Err(Error(
            "Backing-chain checkpoints are not supported; use a standalone disk".into(),
        ));
    }
    if version == 3 && u64::from_be_bytes(header[72..80].try_into().unwrap()) & 0b111 != 0 {
        return Err(Error(
            "Dirty/corrupt or external-data-file qcow2 checkpoints are unsupported".into(),
        ));
    }
    Ok(())
}

/// Restore a verified checkpoint into a fresh directory. Never overwrite an existing VM.
pub fn restore(snapshot: &Path, output: &Path) -> Result<PathBuf> {
    if fs::symlink_metadata(output).is_ok() {
        return Err(Error(
            "Restore requires a new directory; existing VM data is preserved".into(),
        ));
    }
    let source = fs::canonicalize(snapshot).map_err(error)?;
    let marker = source.join(MARKER);
    let m = fs::symlink_metadata(&marker).map_err(error)?;
    if !m.is_file() || m.len() > 1024 * 1024 {
        return Err(Error("Invalid checkpoint manifest file".into()));
    }
    let manifest: Manifest = serde_json::from_slice(&fs::read(&marker).map_err(error)?)
        .map_err(|_| Error("Invalid checkpoint manifest; source preserved".into()))?;
    if manifest.version != 1
        || manifest.kind != "stopped_workspace_copy"
        || manifest.files_sha256.len() > 1028
    {
        return Err(Error("Unsupported checkpoint manifest".into()));
    }
    for required in ["disk.qcow2", "firmware.fd", "nvram.fd", "vm.toml"] {
        if !manifest.files_sha256.contains_key(required) {
            return Err(Error("Checkpoint components missing".into()));
        }
    }
    let mut source_files = BTreeMap::new();
    for (name, digest) in &manifest.files_sha256 {
        let path = Path::new(name);
        let allowed = matches!(
            name.as_str(),
            "disk.qcow2" | "firmware.fd" | "nvram.fd" | "vm.toml"
        ) || name.starts_with("tpm/");
        if !allowed
            || path
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
            || digest.len() != 64
            || !digest.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(Error("Unsafe or invalid checkpoint file entry".into()));
        }
        let p = owned_source(&source, path)?;
        if name == "vm.toml" && fs::metadata(&p).map_err(error)?.len() > 1024 * 1024 {
            return Err(Error("Checkpoint configuration exceeds 1 MiB".into()));
        }
        if hash(&p)? != *digest {
            return Err(Error(
                "Checkpoint checksum mismatch; restore refused".into(),
            ));
        }
        source_files.insert(name.clone(), p);
    }
    standalone_disk(&source_files["disk.qcow2"])?;
    let config = Config::parse(&fs::read_to_string(&source_files["vm.toml"]).map_err(error)?)
        .map_err(|_| Error("Invalid checkpoint configuration".into()))?;
    let r = config
        .runtime
        .as_ref()
        .ok_or_else(|| Error("Checkpoint runtime settings missing".into()))?;
    if config.disk != Path::new("disk.qcow2")
        || r.firmware != Path::new("firmware.fd")
        || r.nvram != Path::new("nvram.fd")
        || r.tpm.as_ref().is_some_and(|t| t.state != Path::new("tpm"))
        || config.installer.is_some()
    {
        return Err(Error(
            "Checkpoint configuration paths are not self-contained".into(),
        ));
    }
    let parent = fs::canonicalize(
        output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )
    .map_err(error)?;
    let m = fs::metadata(&parent).map_err(error)?;
    if parent.starts_with(&source) || m.uid() != unsafe { libc::getuid() } || m.mode() & 0o022 != 0
    {
        return Err(Error(
            "Choose an owned destination outside the checkpoint without group/world write access"
                .into(),
        ));
    }
    let needed = source_files
        .values()
        .try_fold(16 * 1024 * 1024u64, |sum, p| {
            fs::metadata(p)
                .map(|m| sum.saturating_add(m.len()))
                .map_err(error)
        })?;
    if free_space(&parent)? < needed {
        return Err(Error("Insufficient restore destination space".into()));
    }
    let final_path = parent.join(
        output
            .file_name()
            .ok_or_else(|| Error("Restore directory name required".into()))?,
    );
    let pending = parent.join(format!(
        ".freelane-restore-{:016x}.pending",
        getrandom::u64().map_err(error)?
    ));
    private_dir(&pending)?;
    let mut guard = Pending {
        path: pending.clone(),
        active: true,
    };
    fs::write(pending.join(MARKER), b"incomplete restore").map_err(error)?;
    let mut hashes = BTreeMap::new();
    for (name, p) in source_files {
        let target = pending.join(&name);
        if let Some(dir) = target.parent() {
            fs::create_dir_all(dir).map_err(error)?;
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(error)?;
        }
        copy_file(&p, &target, &pending, &mut hashes)?;
        if hashes[&name] != manifest.files_sha256[&name] {
            return Err(Error(
                "Checkpoint changed during restore; incomplete copy discarded".into(),
            ));
        }
    }
    if r.tpm.is_some() && !pending.join("tpm").exists() {
        private_dir(&pending.join("tpm"))?;
    }
    if fs::symlink_metadata(&final_path).is_ok() {
        return Err(Error("Restore output changed; source preserved".into()));
    }
    publish(&pending, &final_path)?;
    guard.active = false;
    fs::remove_file(final_path.join(MARKER)).map_err(|e| {
        Error(format!(
            "Restored data retained with startup marker; finalization failed: {e}"
        ))
    })?;
    Ok(final_path.join("vm.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publication_never_replaces_even_an_empty_existing_directory() {
        let root = std::env::temp_dir().join(format!("fl-publish-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let source = root.join("pending");
        let target = root.join("existing");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&target).unwrap();
        fs::write(source.join("data"), b"owned staging data").unwrap();
        assert!(publish(&source, &target).is_err());
        assert!(source.join("data").exists());
        assert!(target.read_dir().unwrap().next().is_none());
        fs::remove_dir(&target).unwrap();
        publish(&source, &target).unwrap();
        assert_eq!(
            fs::read(target.join("data")).unwrap(),
            b"owned staging data"
        );
        assert!(!source.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
