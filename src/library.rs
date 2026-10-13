//! Private, versioned VM bookmarks. Guest configurations and disks are never written.
use crate::{Error, GuestOs, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub guest_os: GuestOs,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Library {
    version: u16,
    entries: Vec<Entry>,
}
fn valid(entry: &Entry) -> bool {
    entry.path.is_absolute()
        && !entry.name.is_empty()
        && entry.name.len() <= 128
        && !entry.name.chars().any(char::is_control)
}
pub fn default_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("FREELANE_LIBRARY_PATH") {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(Error("Library override must be absolute".into()));
        }
        return Ok(path);
    }
    let home = std::env::var_os("HOME")
        .ok_or_else(|| Error("Home directory unavailable; choose FREELANE_LIBRARY_PATH".into()))?;
    let root = if cfg!(target_os = "macos") {
        PathBuf::from(home).join("Library/Application Support/FreeLane")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| PathBuf::from(home).join(".local/share"))
            .join("freelane")
    };
    Ok(root.join("library.toml"))
}
pub fn read(path: &Path) -> Result<Vec<Entry>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Error(e.to_string())),
    };
    if !metadata.is_file() || metadata.len() > 1024 * 1024 {
        return Err(Error("Library must be a regular file under 1 MiB".into()));
    }
    let data: Library = toml::from_str(
        &fs::read_to_string(path).map_err(|e| Error(e.to_string()))?,
    )
    .map_err(|_| Error("Library is invalid; preserve it and repair/import explicitly".into()))?;
    if data.version != 1 || data.entries.len() > 1000 || data.entries.iter().any(|e| !valid(e)) {
        return Err(Error(
            "Unsupported or invalid library; original file preserved".into(),
        ));
    }
    Ok(data.entries)
}
pub fn record(path: &Path, entry: Entry) -> Result<()> {
    if !path.is_absolute() || !valid(&entry) {
        return Err(Error(
            "Library and VM bookmark paths must be absolute with a valid name".into(),
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error("Library directory missing".into()))?;
    let existed = parent.exists();
    fs::create_dir_all(parent).map_err(|e| Error(e.to_string()))?;
    let metadata = fs::symlink_metadata(parent).map_err(|e| Error(e.to_string()))?;
    let uid = unsafe { libc::getuid() };
    if !metadata.is_dir() || metadata.uid() != uid {
        return Err(Error(
            "Library directory must be owned by the current user, without symlinks".into(),
        ));
    }
    if !existed {
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|e| Error(e.to_string()))?;
    }
    if fs::metadata(parent)
        .map_err(|e| Error(e.to_string()))?
        .mode()
        & 0o077
        != 0
    {
        return Err(Error(
            "Library directory must have private permissions (0700)".into(),
        ));
    }
    let lock_path = parent.join("library.lock");
    if fs::symlink_metadata(&lock_path).is_ok_and(|m| !m.is_file()) {
        return Err(Error("Invalid library lock file".into()));
    }
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(lock_path)
        .map_err(|e| Error(e.to_string()))?;
    lock.try_lock_exclusive()
        .map_err(|_| Error("Library busy; bookmark not saved".into()))?;
    let mut entries = read(path)?;
    if let Some(old) = entries.iter_mut().find(|e| e.path == entry.path) {
        *old = entry;
    } else {
        entries.push(entry);
    }
    if entries.len() > 1000 {
        return Err(Error("Library limit reached".into()));
    }
    let data = toml::to_string(&Library {
        version: 1,
        entries,
    })
    .map_err(|e| Error(e.to_string()))?;
    if data.len() > 1024 * 1024 {
        return Err(Error("Library exceeds 1 MiB".into()));
    }
    let token = getrandom::u64().map_err(|e| Error(e.to_string()))?;
    let temporary = parent.join(format!(".library-{token:016x}.tmp"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|e| Error(e.to_string()))?;
    let result = (|| {
        file.write_all(data.as_bytes())
            .map_err(|e| Error(e.to_string()))?;
        file.sync_all().map_err(|e| Error(e.to_string()))?;
        fs::rename(&temporary, path).map_err(|e| Error(e.to_string()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
