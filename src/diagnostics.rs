//! Read-only prerequisite checks. A passing preflight does not prove guest readiness.
use crate::{qemu, Config};
use serde::Serialize;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Passed,
    Failed,
    NotChecked,
}
#[derive(Debug, Serialize)]
pub struct Check {
    pub id: &'static str,
    pub state: State,
    pub detail: String,
    pub remedy: Option<&'static str>,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub host_os: &'static str,
    pub host_arch: &'static str,
    pub preflight_passed: bool,
    pub guest_runtime_verified: bool,
    pub checks: Vec<Check>,
}
fn check(id: &'static str, ok: bool, detail: String, remedy: &'static str) -> Check {
    Check {
        id,
        state: if ok { State::Passed } else { State::Failed },
        detail,
        remedy: if ok { None } else { Some(remedy) },
    }
}
pub(crate) fn executable(path: &Path, base: &Path) -> bool {
    let valid = |p: PathBuf| {
        fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    if path.components().count() > 1 || path.is_absolute() {
        valid(base.join(path))
    } else {
        std::env::var_os("PATH")
            .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| valid(dir.join(path))))
    }
}
pub fn inspect(path: &Path) -> Report {
    let mut report = Report {
        host_os: std::env::consts::OS,
        host_arch: std::env::consts::ARCH,
        preflight_passed: false,
        guest_runtime_verified: false,
        checks: Vec::new(),
    };
    let loaded = fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|s| Config::parse(&s).map_err(|e| e.to_string()));
    let config = match loaded {
        Ok(c) => c,
        Err(e) => {
            report.checks.push(check(
                "configuration",
                false,
                e,
                "Choose a readable valid VM TOML configuration.",
            ));
            return report;
        }
    };
    report.checks.push(check(
        "configuration",
        true,
        "Configuration parsed and validated".into(),
        "",
    ));
    let base = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let Some(runtime) = &config.runtime else {
        report.checks.push(check(
            "runtime",
            false,
            "No runtime settings configured".into(),
            "Configure QEMU, firmware and private NVRAM before booting.",
        ));
        return report;
    };
    let paths = qemu::runtime_dir(path)
        .and_then(|dir| qemu::arguments(&config, base, &dir.join("qmp.sock")));
    report.checks.push(check("images_and_paths", paths.is_ok(),
        match paths { Ok(_) => "Images and backend paths passed structural checks; image architecture and Secure Boot remain unverified".into(), Err(e) => e.to_string() },
        "Correct missing, empty or conflicting image/firmware/NVRAM/share/TPM paths; preserve existing guest data."));
    report.checks.push(check(
        "qemu_executable",
        executable(&runtime.binary, base),
        format!("Configured backend: {}", runtime.binary.display()),
        "Install QEMU or select an executable backend path available to this process.",
    ));
    if let Some(tpm) = &runtime.tpm {
        report.checks.push(check(
            "tpm_executable",
            executable(&tpm.binary, base),
            format!("Configured TPM backend: {}", tpm.binary.display()),
            "Install swtpm or correct its executable path.",
        ));
    }
    let accelerated = runtime.accelerator == qemu::Accelerator::Hvf;
    report.checks.push(check("acceleration_host", !accelerated || crate::supported_host(),
        if accelerated { "HVF requires Apple Silicon macOS; framework entitlement and actual acceleration still require runtime testing".into() }
        else { "TCG selected explicitly; hardware acceleration is not claimed".into() },
        "Use Apple Silicon macOS for HVF, or explicitly choose TCG for software emulation."));
    report.preflight_passed = report.checks.iter().all(|c| c.state == State::Passed);
    report.checks.push(Check {id: "guest_readiness", state: State::NotChecked,
        detail: "No backend, agent or VM was started or contacted. OS boot, drivers, agent compatibility and display/input are not verified.".into(), remedy: None});
    report
}
