//! Observed backend/agent state, never inferred installation completion.
use crate::{agent, qemu, Config, Error, Result};
use serde::Serialize;
use std::{io::Read, os::unix::fs::OpenOptionsExt, path::Path};
#[derive(Debug, Serialize)]
pub struct Report {
    pub backend: String,
    pub agent: String,
    pub installation_complete: Option<bool>,
}
pub fn inspect(path: &Path) -> Result<Report> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| Error(e.to_string()))?;
    let metadata = file.metadata().map_err(|e| Error(e.to_string()))?;
    if !metadata.is_file() || metadata.len() > 1024 * 1024 {
        return Err(Error(
            "Readiness requires a regular config of at most 1 MiB".into(),
        ));
    }
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Error(e.to_string()))?;
    if bytes.len() > 1024 * 1024 {
        return Err(Error("Config grew beyond 1 MiB".into()));
    }
    let config = Config::parse(std::str::from_utf8(&bytes).map_err(|e| Error(e.to_string()))?)?;
    if path
        .parent()
        .unwrap_or(Path::new("."))
        .join(crate::provision::MARKER)
        .symlink_metadata()
        .is_ok()
    {
        return Ok(Report {
            backend: "incomplete".into(),
            agent: "not_checked".into(),
            installation_complete: None,
        });
    }
    let backend = match qemu::control(path, "status") {
        Ok(value) => match (value["status"].as_str(), value["running"].as_bool()) {
            (Some("running"), Some(true)) => "running",
            (Some("paused"), Some(false)) => "paused",
            (Some("prelaunch"), Some(false)) => "prelaunch",
            (Some("shutdown"), Some(false)) => "shutdown",
            _ => "unknown",
        },
        Err(_) => "unknown",
    };
    let enabled = config.runtime.as_ref().is_some_and(|r| r.guest_agent);
    let agent_state = if !enabled {
        "disabled"
    } else if backend != "running" {
        "not_checked"
    } else {
        match agent::request(path, freelane_agent_protocol::Action::Ping) {
            Ok(response)
                if matches!(
                    response.outcome,
                    freelane_agent_protocol::Outcome::Pong { .. }
                ) =>
            {
                "connected"
            }
            _ => "unavailable",
        }
    };
    Ok(Report {
        backend: backend.into(),
        agent: agent_state.into(),
        installation_complete: None,
    })
}
