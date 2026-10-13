//! Locate installed QEMU without relying on a Finder application's PATH.
use std::path::{Path, PathBuf};
#[derive(Debug, Clone)]
pub struct Tools {
    pub firmware: PathBuf,
    pub variables: PathBuf,
    pub image: PathBuf,
}
pub fn discover() -> Option<Tools> {
    let mut prefixes = vec![
        PathBuf::from("/opt/homebrew"),
        PathBuf::from("/usr/local"),
        PathBuf::from("/usr"),
    ];
    if let Some(paths) = std::env::var_os("PATH") {
        prefixes.extend(
            std::env::split_paths(&paths).filter_map(|p| p.parent().map(Path::to_path_buf)),
        );
    }
    discover_in(&prefixes)
}
fn discover_in(prefixes: &[PathBuf]) -> Option<Tools> {
    for prefix in prefixes {
        let image = prefix.join("bin/qemu-img");
        let share = prefix.join("share/qemu");
        let system = prefix.join("bin/qemu-system-aarch64");
        let tpm = prefix.join("bin/swtpm");
        let firmware = share.join("edk2-aarch64-code.fd");
        let variables = share.join("edk2-arm-vars.fd");
        if crate::diagnostics::executable(&image, Path::new("."))
            && crate::diagnostics::executable(&system, Path::new("."))
            && (!cfg!(target_os = "macos")
                || (crate::diagnostics::executable(&tpm, Path::new("."))
                    && ["7zz", "7z"].iter().any(|n| {
                        crate::diagnostics::executable(&prefix.join("bin").join(n), Path::new("."))
                    })))
            && [&firmware, &variables]
                .iter()
                .all(|p| p.metadata().is_ok_and(|m| m.is_file() && m.len() > 0))
        {
            return Some(Tools {
                firmware,
                variables,
                image,
            });
        }
    }
    None
}
pub fn binary(name: &str) -> PathBuf {
    #[cfg(target_os = "macos")]
    if let Some(tools) = discover() {
        if let Some(bin) = tools.image.parent() {
            let path = bin.join(name);
            if crate::diagnostics::executable(&path, Path::new(".")) {
                return path;
            }
        }
    }
    name.into()
}
/// Run an owned installer with a deadline; never interpolate paths in a shell.
pub fn run_installer(
    command: &mut std::process::Command,
    timeout: std::time::Duration,
) -> crate::Result<()> {
    use std::{os::unix::process::CommandExt, process::Stdio, time::Instant};
    command.process_group(0);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            crate::Error("Component installation failed. Check Homebrew and retry.".into())
        })?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(s)) => {
                return if s.success() {
                    Ok(())
                } else {
                    Err(crate::Error(
                        "Component installation failed. Check Homebrew and retry.".into(),
                    ))
                }
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20))
            }
            _ => {
                let group = -(child.id() as libc::pid_t);
                // The process group was created exclusively for this child.
                unsafe {
                    libc::kill(group, libc::SIGTERM);
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
                unsafe {
                    libc::kill(group, libc::SIGKILL);
                }
                let _ = child.wait();
                return Err(crate::Error(
                    "Component installation timed out. Check Homebrew before retrying.".into(),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn homebrew_layout_is_discovered_without_shell_path() {
        use std::{fs, os::unix::fs::PermissionsExt};
        let root = std::env::temp_dir().join(format!("fl-host-tools-{}", std::process::id()));
        fs::create_dir_all(root.join("share/qemu")).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        for n in ["edk2-aarch64-code.fd", "edk2-arm-vars.fd"] {
            fs::write(root.join("share/qemu").join(n), b"firmware").unwrap();
        }
        let image = root.join("bin/qemu-img");
        fs::write(&image, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&image, fs::Permissions::from_mode(0o700)).unwrap();
        for name in ["qemu-system-aarch64", "swtpm", "7zz"] {
            let path = root.join("bin").join(name);
            fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let partial = root.join("partial");
        fs::create_dir_all(partial.join("bin")).unwrap();
        let partial_system = partial.join("bin/qemu-system-aarch64");
        fs::write(&partial_system, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&partial_system, fs::Permissions::from_mode(0o700)).unwrap();
        let found =
            discover_in(&[partial.clone(), root.clone()]).expect("installed tools should be found");
        assert_eq!(
            found.image.parent().unwrap().join("qemu-system-aarch64"),
            root.join("bin/qemu-system-aarch64")
        );
        assert_eq!(found.image, image);
        assert_eq!(found.variables, root.join("share/qemu/edk2-arm-vars.fd"));
        fs::remove_file(root.join("share/qemu/edk2-arm-vars.fd")).unwrap();
        assert!(discover_in(std::slice::from_ref(&root)).is_none());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn installer_timeout_signals_owned_descendants() {
        let root = std::env::temp_dir().join(format!("fl-installer-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let marker = root.join("terminated");
        let script = root.join("installer.sh");
        std::fs::write(&script,b"#!/bin/sh\n(trap 'echo yes > \"$1\"; exit 0' TERM; while :; do sleep 0.05; done) &\nwait\n").unwrap();
        let result = run_installer(
            std::process::Command::new("/bin/sh")
                .arg(&script)
                .arg(&marker),
            std::time::Duration::from_millis(300),
        );
        assert!(result.is_err());
        assert!(
            marker.exists(),
            "owned installer child did not receive termination"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
