//! Per-app launch profiles and native Dock bundles. No shell or implicit launch retry.
use crate::{agent, qemu, Config, Error, GuestOs, Result};
use freelane_agent_protocol::{Action, Outcome, Request, VERSION};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppProfile {
    pub name: String,
    pub vm: PathBuf,
    pub executable: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default = "timeout")]
    pub boot_timeout_seconds: u64,
}
fn timeout() -> u64 {
    600
}
impl AppProfile {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty()
            || self.name.len() > 128
            || self.name.chars().any(char::is_control)
            || self.vm.as_os_str().is_empty()
            || !(10..=900).contains(&self.boot_timeout_seconds)
        {
            return Err(Error(
                "Invalid app name, VM path or boot timeout (10–900 seconds)".into(),
            ));
        }
        let b = self.executable.as_bytes();
        if b.len() < 4
            || !b[0].is_ascii_alphabetic()
            || b[1] != b':'
            || !b"\\/".contains(&b[2])
            || !self.executable.to_ascii_lowercase().ends_with(".exe")
        {
            return Err(Error("Use an absolute Windows drive .exe path".into()));
        }
        Request {
            version: VERSION,
            id: 1,
            action: Action::Launch {
                executable: self.executable.clone(),
                arguments: self.arguments.clone(),
            },
        }
        .validate()
        .map_err(|e| Error(e.to_string()))
    }
    pub fn parse(text: &str) -> Result<Self> {
        let p: Self = toml::from_str(text).map_err(|e| Error(e.to_string()))?;
        p.validate()?;
        Ok(p)
    }
    pub fn load(path: &Path) -> Result<Self> {
        if fs::metadata(path).map_err(|e| Error(e.to_string()))?.len() > 1024 * 1024 {
            return Err(Error("App profile exceeds 1 MiB".into()));
        }
        let mut p = Self::parse(&fs::read_to_string(path).map_err(|e| Error(e.to_string()))?)?;
        p.vm = fs::canonicalize(path.parent().unwrap_or(Path::new(".")).join(&p.vm))
            .map_err(|e| Error(e.to_string()))?;
        Ok(p)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        use std::{io::Write, os::unix::fs::OpenOptionsExt};
        self.validate()?;
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| Error(e.to_string()))?;
        let text = toml::to_string_pretty(self).map_err(|e| Error(e.to_string()))?;
        f.write_all(text.as_bytes())
            .and_then(|()| f.sync_all())
            .map_err(|e| Error(e.to_string()))
    }
}
fn backend_path() -> std::ffi::OsString {
    let original = std::env::var_os("PATH").unwrap_or_default();
    if !cfg!(target_os = "macos") {
        return original;
    }
    let mut paths: Vec<PathBuf> = std::env::split_paths(&original).collect();
    for path in ["/opt/homebrew/bin", "/usr/local/bin"] {
        let path = PathBuf::from(path);
        if path.is_dir() && !paths.contains(&path) {
            paths.push(path);
        }
    }
    std::env::join_paths(paths).unwrap_or(original)
}
struct ReapSupervisor(Option<std::process::Child>);
impl Drop for ReapSupervisor {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}
pub fn ensure_vm(profile: &AppProfile, cli: &Path) -> Result<()> {
    use std::os::unix::{
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        process::CommandExt,
    };
    profile.validate()?;
    let text = fs::read_to_string(&profile.vm).map_err(|e| Error(e.to_string()))?;
    let config = Config::parse(&text)?;
    if config.guest_os != GuestOs::Windows
        || !config.runtime.as_ref().is_some_and(|r| r.guest_agent)
    {
        return Err(Error(
            "App windows require a Windows VM with guest_agent=true".into(),
        ));
    }
    config.check_images(
        profile
            .vm
            .parent()
            .ok_or_else(|| Error("Missing VM parent".into()))?,
    )?;
    let directory = qemu::runtime_dir(&profile.vm)?;
    match fs::create_dir(&directory) {
        Ok(()) => fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .map_err(|e| Error(e.to_string()))?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(Error(e.to_string())),
    }
    let metadata = fs::symlink_metadata(&directory).map_err(|e| Error(e.to_string()))?;
    if !metadata.is_dir()
        || metadata.mode() & 0o077 != 0
        || metadata.uid()
            != fs::metadata(&profile.vm)
                .map_err(|e| Error(e.to_string()))?
                .uid()
    {
        return Err(Error(
            "App runtime directory must be private and owned by VM owner".into(),
        ));
    }
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(directory.join("app-start.lock"))
        .map_err(|e| Error(e.to_string()))?;
    lock.try_lock_exclusive()
        .map_err(|_| Error("Another app is starting this VM; wait and open again".into()))?;
    let deadline = Instant::now() + Duration::from_secs(profile.boot_timeout_seconds);
    let mut child = ReapSupervisor(None);
    let start_needed = if directory.join("qmp.sock").exists() {
        match qemu::control(&profile.vm, "status") {
            Ok(status) if status["running"] == true => false,
            Ok(_) => {
                return Err(Error(
                    "VM is paused or stopping; resume it explicitly before opening an app".into(),
                ))
            }
            Err(error) => {
                // A crashed supervisor can leave socket files behind. Only recover
                // after taking the backend's own lock, never from socket age alone.
                let backend_lock = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .mode(0o600)
                    .open(directory.join("lock"))
                    .map_err(|e| Error(e.to_string()))?;
                backend_lock.try_lock_exclusive().map_err(|_| {
                    Error(format!(
                        "VM state unknown while its supervisor owns the lock: {error}"
                    ))
                })?;
                FileExt::unlock(&backend_lock).map_err(|e| Error(e.to_string()))?;
                true
            }
        }
    } else {
        true
    };
    if start_needed {
        let log = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(directory.join(format!(
                "app-start-{:016x}.log",
                getrandom::u64().map_err(|e| Error(e.to_string()))?
            )))
            .map_err(|e| Error(e.to_string()))?;
        let error = log.try_clone().map_err(|e| Error(e.to_string()))?;
        child.0 = Some(
            Command::new(cli)
                .args([
                    "start",
                    profile
                        .vm
                        .to_str()
                        .ok_or_else(|| Error("VM path must be UTF-8".into()))?,
                ])
                .env("PATH", backend_path())
                .process_group(0)
                .stdin(Stdio::null())
                .stdout(log)
                .stderr(error)
                .spawn()
                .map_err(|e| Error(e.to_string()))?,
        );
    }
    loop {
        if let Some(c) = child.0.as_mut() {
            if let Some(s) = c.try_wait().map_err(|e| Error(e.to_string()))? {
                return Err(Error(format!(
                    "VM supervisor exited {s}; inspect app-start log"
                )));
            }
        }
        if qemu::control(&profile.vm, "status").is_ok_and(|s| s["running"] == true)
            && agent::request(&profile.vm, Action::Ping)
                .is_ok_and(|r| matches!(r.outcome,Outcome::Pong{ref os,..} if os=="windows"))
        {
            break;
        }
        if Instant::now() >= deadline {
            return Err(Error("Windows/interactive agent readiness timed out; VM is left running. No app launch was sent".into()));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    // The guard reaps even on readiness errors; dropping it never kills the VM.
    Ok(())
}
pub fn open(
    profile: &AppProfile,
    cli: &Path,
) -> Result<(u32, Vec<freelane_agent_protocol::WindowInfo>)> {
    ensure_vm(profile, cli)?;
    match agent::window_request(&profile.vm, Action::Ping)?.outcome {
        Outcome::Capabilities {
            window_snapshots: true,
        } => {}
        _ => {
            return Err(Error(
                "Update and run the Windows interactive agent with window support".into(),
            ))
        }
    }
    let response = agent::request(
        &profile.vm,
        Action::Launch {
            executable: profile.executable.clone(),
            arguments: profile.arguments.clone(),
        },
    )?;
    let Outcome::Launched { pid } = response.outcome else {
        return Err(Error("Missing launched process ID".into()));
    };
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let reply = agent::window_request(&profile.vm, Action::Windows { pid })?;
        if let Outcome::Windows { windows } = reply.outcome {
            if !windows.is_empty() {
                return Ok((pid, windows));
            }
        }
        if Instant::now() >= deadline {
            return Err(Error(format!("Process {pid} launched once but no visible window appeared. Delegated/single-instance apps may use another PID; inspect the guest. No retry sent")));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
/// Builds bundle files without signing; useful for offline validation and the native export path.
pub fn bundle_files(profile: &AppProfile, output: &Path, desktop: &Path, cli: &Path) -> Result<()> {
    profile.validate()?;
    if output.extension().and_then(|s| s.to_str()) != Some("app") {
        return Err(Error(
            "Dock launcher output must be a new .app directory".into(),
        ));
    }
    let vm = fs::canonicalize(&profile.vm).map_err(|e| Error(e.to_string()))?;
    let mut profile = profile.clone();
    profile.vm = vm;
    fs::create_dir(output).map_err(|e| Error(e.to_string()))?;
    let result = (|| {
        fs::create_dir_all(output.join("Contents/MacOS")).map_err(|e| Error(e.to_string()))?;
        fs::create_dir_all(output.join("Contents/Resources")).map_err(|e| Error(e.to_string()))?;
        for (source, name) in [(desktop, "freelane-desktop"), (cli, "freelane")] {
            fs::copy(source, output.join("Contents/MacOS").join(name))
                .map_err(|e| Error(e.to_string()))?;
        }
        profile.save(&output.join("Contents/Resources/app.toml"))?;
        let name = xml(&profile.name);
        use sha2::{Digest, Sha256};
        // Name and output folder can change without changing LaunchServices identity.
        // Arguments distinguish separate document shortcuts; moving the VM changes identity.
        let identity = serde_json::to_vec(&(&profile.vm, &profile.executable, &profile.arguments))
            .map_err(|e| Error(e.to_string()))?;
        let id = Sha256::digest(identity)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>org.freelane.app.{id}</string><key>CFBundleName</key><string>{name}</string><key>CFBundleDisplayName</key><string>{name}</string><key>CFBundleExecutable</key><string>freelane-desktop</string><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleVersion</key><string>0.1.0</string><key>LSMinimumSystemVersion</key><string>14.0</string><key>NSHighResolutionCapable</key><true/></dict></plist>"#
        );
        fs::write(output.join("Contents/Info.plist"), plist).map_err(|e| Error(e.to_string()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(output);
    }
    result
}
pub fn export_dock(profile: &AppProfile, output: &Path, desktop: &Path, cli: &Path) -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(Error("Dock launcher export/signing requires macOS".into()));
    }
    bundle_files(profile, output, desktop, cli)?;
    for path in [
        output.join("Contents/MacOS/freelane"),
        output.join("Contents/MacOS/freelane-desktop"),
        output.into(),
    ] {
        let result = Command::new("codesign")
            .args(["--force", "--sign", "-"])
            .arg(&path)
            .output()
            .map_err(|e| Error(e.to_string()))?;
        if !result.status.success() {
            return Err(Error(format!(
                "Launcher signing failed; inspect {}: {}",
                output.display(),
                String::from_utf8_lossy(&result.stderr)
            )));
        }
    }
    let result = Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(output)
        .output()
        .map_err(|e| Error(e.to_string()))?;
    if !result.status.success() {
        return Err(Error("Dock bundle signature verification failed".into()));
    }
    Ok(())
}
