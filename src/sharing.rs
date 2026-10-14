//! Read-only host availability; configured transport is not guest mount evidence.
use crate::{qemu::ShareMode, Config};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostState {
    Disabled,
    Available,
    Missing,
    NotDirectory,
    VolumeUnavailable,
    Unavailable,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub host: HostState,
    pub path: Option<PathBuf>,
    pub mode: Option<ShareMode>,
    pub guest_mount: &'static str,
}
pub fn inspect(config: &Config, base: &Path) -> Report {
    let runtime = config.runtime.as_ref();
    let selected = runtime.and_then(|r| r.shared_directory.as_ref());
    let path = selected.map(|p| {
        if p.is_absolute() {
            p.clone()
        } else {
            base.join(p)
        }
    });
    let mut report = Report {
        host: HostState::Disabled,
        path,
        mode: runtime.map(|r| r.share_mode),
        guest_mount: "unknown",
    };
    let Some(path) = &report.path else {
        return report;
    };
    report.host = match fs::metadata(path) {
        Ok(m) if !m.is_dir() => HostState::NotDirectory,
        Ok(_) => {
            let absolute = fs::canonicalize(path);
            if crate::storage::check_share_volume(path).is_err()
                || absolute
                    .as_ref()
                    .is_ok_and(|p| crate::storage::check_share_volume(p).is_err())
            {
                HostState::VolumeUnavailable
            } else if absolute.is_err() {
                HostState::Unavailable
            } else {
                HostState::Available
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => HostState::Missing,
        Err(_) => HostState::Unavailable,
    };
    report
}
