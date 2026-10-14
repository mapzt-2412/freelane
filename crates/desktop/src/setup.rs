//! One host/media setup job; user passwords stay in the system Terminal.
use super::*;
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Ubuntu {
    #[default]
    Latest,
    Noble,
}
impl Ubuntu {
    pub fn label(self) -> &'static str {
        match self {
            Self::Latest => "Ubuntu Server 26.04.1 LTS",
            Self::Noble => "Ubuntu Server 24.04.5 LTS",
        }
    }
    fn release(self) -> &'static str {
        match self {
            Self::Latest => "26.04",
            Self::Noble => "24.04.5",
        }
    }
    fn filename(self) -> &'static str {
        match self {
            Self::Latest => "ubuntu-26.04.1-live-server-arm64.iso",
            Self::Noble => "ubuntu-24.04.5-live-server-arm64.iso",
        }
    }
}
use std::sync::atomic::{AtomicBool, Ordering};
pub type Cancel = Arc<AtomicBool>;
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn checksum(manifest: &str, filename: &str) -> Result<String, String> {
    let rows: Vec<_> = manifest
        .lines()
        .filter_map(|line| {
            let (hash, file) = line.split_once("  ").or_else(|| line.split_once(" *"))?;
            (file == filename).then_some(hash)
        })
        .collect();
    if rows.len() != 1 || rows[0].len() != 64 || !rows[0].bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Ubuntu checksum manifest is invalid".into());
    }
    Ok(rows[0].to_ascii_lowercase())
}
fn check(cancel: &Cancel) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Setup cancelled".into())
    } else {
        Ok(())
    }
}
fn run(
    command: &mut Command,
    cancel: &Cancel,
    timeout: Duration,
    progress: &mpsc::Sender<String>,
    file: Option<&std::fs::File>,
) -> Result<(), String> {
    check(cancel)?;
    use std::os::unix::process::CommandExt;
    command.process_group(0);
    let mut child = command
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + timeout;
    let mut tick = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err("Setup command failed. Check your connection and retry.".into())
                }
            }
            Ok(None) if !cancel.load(Ordering::Relaxed) && Instant::now() < deadline => {}
            _ => {
                unsafe {
                    libc::kill(-(child.id() as libc::pid_t), libc::SIGTERM);
                }
                std::thread::sleep(Duration::from_millis(200));
                unsafe {
                    libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
                }
                let _ = child.wait();
                return Err(if cancel.load(Ordering::Relaxed) {
                    "Setup cancelled"
                } else {
                    "Setup timed out. Retry when the connection is available."
                }
                .into());
            }
        }
        if tick.elapsed() >= Duration::from_secs(1) {
            if let Some(file) = file {
                let _ = progress.send(format!(
                    "Downloading Ubuntu: {} MiB",
                    file.metadata().map(|m| m.len()).unwrap_or(0) / (1024 * 1024)
                ));
            }
            tick = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn fetch(
    url: &str,
    file: std::fs::File,
    max: u64,
    cancel: &Cancel,
    progress: &mpsc::Sender<String>,
) -> Result<(), String> {
    // Bind curl output to an already-open file, even if selected storage disappears.
    run(
        Command::new("/usr/bin/curl")
            .stdin(Stdio::null())
            .args([
                "--fail",
                "--silent",
                "--show-error",
                "--location",
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
                "--connect-timeout",
                "30",
                "--max-time",
                "7200",
                "--max-filesize",
                &max.to_string(),
                url,
            ])
            .stdout(file.try_clone().map_err(|e| e.to_string())?),
        cancel,
        Duration::from_secs(7210),
        progress,
        (max > 1024 * 1024).then_some(&file),
    )?;
    file.sync_all().map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > max {
        return Err("Download exceeds its size limit".into());
    }
    Ok(())
}
pub fn ubuntu_image(
    version: Ubuntu,
    parent: &Path,
    cancel: &Cancel,
    progress: &mpsc::Sender<String>,
) -> Result<PathBuf, String> {
    use std::io::Write;
    let stage = freelane::storage::DownloadStage::new(parent).map_err(|e| e.to_string())?;
    let base = format!(
        "https://cdimage.ubuntu.com/releases/{}/release",
        version.release()
    );
    let _ = progress.send("Checking Ubuntu signature".into());
    fetch(
        &format!("{base}/SHA256SUMS"),
        stage.file("SHA256SUMS", true).map_err(|e| e.to_string())?,
        1024 * 1024,
        cancel,
        progress,
    )?;
    fetch(
        &format!("{base}/SHA256SUMS.gpg"),
        stage
            .file("SHA256SUMS.gpg", true)
            .map_err(|e| e.to_string())?,
        65536,
        cancel,
        progress,
    )?;
    stage
        .file("keyring.gpg", true)
        .map_err(|e| e.to_string())?
        .write_all(include_bytes!("../assets/ubuntu-cdimage-keyring.gpg"))
        .map_err(|e| e.to_string())?;
    stage.verify().map_err(|e| e.to_string())?;
    run(
        Command::new(freelane::host_tools::binary("gpg"))
            .stdin(Stdio::null())
            .arg("--homedir")
            .arg(stage.path())
            .args([
                "--no-options",
                "--batch",
                "--no-auto-key-retrieve",
                "--no-default-keyring",
            ])
            .arg("--keyring")
            .arg(stage.path().join("keyring.gpg"))
            .arg("--verify")
            .arg(stage.path().join("SHA256SUMS.gpg"))
            .arg(stage.path().join("SHA256SUMS"))
            .stdout(Stdio::null()),
        cancel,
        Duration::from_secs(30),
        progress,
        None,
    )
    .map_err(|error| {
        if cancel.load(Ordering::Relaxed) {
            error
        } else {
            "Ubuntu signature verification failed; download refused".into()
        }
    })?;
    stage.verify().map_err(|e| e.to_string())?;
    let mut manifest = String::new();
    stage
        .file("SHA256SUMS", false)
        .map_err(|e| e.to_string())?
        .read_to_string(&mut manifest)
        .map_err(|e| e.to_string())?;
    let expected = checksum(&manifest, version.filename())?;
    let image = stage.path().join(version.filename());
    let _ = progress.send("Downloading Ubuntu Server".into());
    fetch(
        &format!("{base}/{}", version.filename()),
        stage
            .file(version.filename(), true)
            .map_err(|e| e.to_string())?,
        8 * 1024 * 1024 * 1024,
        cancel,
        progress,
    )?;
    let _ = progress.send("Verifying Ubuntu SHA256".into());
    let digest = stage.file("digest", true).map_err(|e| e.to_string())?;
    let mut cmd = if cfg!(target_os = "macos") {
        let mut cmd = Command::new("/usr/bin/shasum");
        cmd.args(["-a", "256"]);
        cmd
    } else {
        Command::new("sha256sum")
    };
    run(
        cmd.stdin(
            stage
                .file(version.filename(), false)
                .map_err(|e| e.to_string())?,
        )
        .stdout(digest),
        cancel,
        Duration::from_secs(600),
        progress,
        None,
    )?;
    let mut actual = String::new();
    stage
        .file("digest", false)
        .map_err(|e| e.to_string())?
        .read_to_string(&mut actual)
        .map_err(|e| e.to_string())?;
    if actual.split_whitespace().next() != Some(expected.as_str()) {
        return Err("Ubuntu SHA256 verification failed; retry the download".into());
    }
    stage.verify().map_err(|e| e.to_string())?;
    check(cancel)?;
    freelane::media::verify_arm64_iso(&image).map_err(|e| e.to_string())?;
    // The VM references verified installation media until ejected.
    stage.retain().map_err(|e| e.to_string())?;
    Ok(image)
}

pub fn install_components(cancel: &Cancel, progress: &mpsc::Sender<String>) -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("Automatic component installation requires macOS".into());
    }
    let _ = progress.send("Installing VM components".into());
    if let Some(brew) = super::wizard::brew_path() {
        return run(
            Command::new(brew)
                .stdin(Stdio::null())
                .args(["install", "qemu", "swtpm", "7zip", "gnupg"])
                .env("HOMEBREW_NO_AUTO_UPDATE", "1")
                .stdout(Stdio::null()),
            cancel,
            Duration::from_secs(3600),
            progress,
            None,
        );
    }
    // Pinned upstream script is bundled; Terminal owns administrator/CLT prompts.
    let stage = tempfile::Builder::new()
        .prefix("freelane-host-setup-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let script = stage.path().join("homebrew-install.sh");
    std::fs::write(&script, include_bytes!("../assets/homebrew-install.sh"))
        .map_err(|e| e.to_string())?;
    let done = stage.path().join("complete");
    let terminal_command = format!(
        "/bin/bash {} && /opt/homebrew/bin/brew install qemu swtpm 7zip gnupg && /usr/bin/touch {}",
        shell_quote(&script.display().to_string()),
        shell_quote(&done.display().to_string())
    );
    // Terminal starts asynchronously; retain this private script until it finishes.
    let stage_path = stage.keep();
    let apple_string = terminal_command.replace('\\', "\\\\").replace('"', "\\\"");
    run(
        Command::new("/usr/bin/osascript")
            .stdin(Stdio::null())
            .args([
                "-e",
                &format!("tell application \"Terminal\" to do script \"{apple_string}\""),
                "-e",
                "tell application \"Terminal\" to activate",
            ])
            .stdout(Stdio::null()),
        cancel,
        Duration::from_secs(30),
        progress,
        None,
    )?;
    let _ = progress.send("Complete the macOS installation prompts in Terminal".into());
    let deadline = Instant::now() + Duration::from_secs(3600);
    while Instant::now() < deadline {
        if cancel.load(Ordering::Relaxed) {
            return Err("Setup cancelled. Close the installation tab in Terminal to stop host installation.".into());
        }
        if done.is_file()
            && freelane::host_tools::discover().is_some()
            && Path::new("/opt/homebrew/bin/gpg").is_file()
        {
            let _ = std::fs::remove_dir_all(&stage_path);
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err("Component installation timed out. Check Homebrew before retrying.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_requires_one_exact_filename_and_sha256() {
        let hash = "a".repeat(64);
        assert_eq!(
            checksum(&format!("{hash} *ubuntu.iso\n"), "ubuntu.iso").unwrap(),
            hash
        );
        assert!(checksum(&format!("{hash} *other-ubuntu.iso\n"), "ubuntu.iso").is_err());
        assert!(checksum("bad *ubuntu.iso\n", "ubuntu.iso").is_err());
        assert!(checksum(
            &format!("{hash} *ubuntu.iso\n{hash} *ubuntu.iso\n"),
            "ubuntu.iso"
        )
        .is_err());
    }
    #[test]
    fn shell_paths_are_literal() {
        assert_eq!(
            shell_quote("/tmp/a'b $(touch x)"),
            "'/tmp/a'\\''b $(touch x)'"
        );
    }
    #[test]
    fn cancellation_terminates_owned_descendants() {
        let root = tempfile::tempdir().unwrap();
        let marker = root.path().join("terminated");
        let script = root.path().join("installer.sh");
        std::fs::write(&script,b"#!/bin/sh\n(trap 'echo yes > \"$1\"; exit 0' TERM; while :; do sleep 0.05; done) &\nwait\n").unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let trigger = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            flag.store(true, Ordering::Relaxed);
        });
        let (tx, _) = mpsc::channel();
        let result = run(
            Command::new("/bin/sh")
                .arg(script)
                .arg(&marker)
                .stdin(Stdio::null())
                .stdout(Stdio::null()),
            &cancel,
            Duration::from_secs(5),
            &tx,
            None,
        );
        trigger.join().unwrap();
        assert_eq!(result.unwrap_err(), "Setup cancelled");
        assert!(marker.exists(), "owned descendant must receive termination");
    }
    #[test]
    #[ignore = "Downloads official Ubuntu Server ARM64; requires network and several GiB"]
    fn official_server_download_acceptance() {
        let root = Path::new("/workspace/quick-setup-acceptance");
        std::fs::create_dir_all(root).unwrap();
        let parent = freelane::storage::prepare_vm_parent(root).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::channel();
        let logger = std::thread::spawn(move || {
            while let Ok(line) = rx.recv() {
                eprintln!("{line}");
            }
        });
        let image = ubuntu_image(Ubuntu::Noble, &parent, &cancel, &tx).unwrap();
        std::fs::write(
            root.join("verified-iso-path.txt"),
            image.display().to_string(),
        )
        .unwrap();
        drop(tx);
        logger.join().unwrap();
    }
}
