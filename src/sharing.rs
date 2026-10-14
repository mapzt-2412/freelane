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
    pub guest_mounts: Vec<freelane_agent_protocol::SharedMount>,
    pub source_identity_verified: bool,
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
        guest_mounts: Vec::new(),
        source_identity_verified: false,
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

/// Guest mount-tag observation does not identify the running backend's host source.
pub fn inspect_live(profile: &Path, config: &Config, base: &Path) -> Report {
    let mut report = inspect(config, base);
    if report.host == HostState::Disabled
        || !matches!(
            report.mode,
            Some(ShareMode::Virtio9p | ShareMode::Virtio9pReadonly)
        )
    {
        return report;
    }
    let Ok(info) = crate::agent_capabilities::inspect(profile) else {
        return report;
    };
    if info.supports(
        "share_status",
        freelane_agent_protocol::SHARE_STATUS_VERSION,
    ) != Some(true)
    {
        return report;
    }
    if let Ok(response) = crate::agent::share_status(profile) {
        if let freelane_agent_protocol::Outcome::ShareMounts { mounts } = response.outcome {
            report.guest_mount = if mounts.is_empty() {
                "not_mounted"
            } else {
                "mounted"
            };
            report.guest_mounts = mounts;
        }
    }
    report
}
