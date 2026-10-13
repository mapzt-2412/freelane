use freelane::diagnostics::{inspect, State};
use std::{fs, os::unix::fs::PermissionsExt};

#[test]
fn diagnostic_is_readonly_and_never_executes_configured_backend() {
    let root = std::env::temp_dir().join(format!("fl-diag-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    for name in ["disk.img", "firmware.fd", "nvram.fd"] {
        fs::write(root.join(name), b"fixture only").unwrap();
    }
    let binary = root.join("fake-qemu");
    fs::write(&binary, "#!/bin/sh\ntouch MUST_NOT_EXECUTE\n").unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    let config = root.join("vm.toml");
    fs::write(&config,"name='Diagnostic'\ncpus=2\nmemory_mib=2048\ndisk='disk.img'\n[runtime]\nfirmware='firmware.fd'\nnvram='nvram.fd'\nbinary='./fake-qemu'\naccelerator='tcg'\nheadless=true\n").unwrap();
    fs::create_dir(root.join("tpm")).unwrap();
    let text = fs::read_to_string(&config).unwrap()
        + "\n[runtime.tpm]\nstate='tpm'\nbinary='./fake-qemu'\n";
    fs::write(&config, text).unwrap();
    let before = fs::read(&config).unwrap();
    let report = inspect(&config);
    assert!(report.preflight_passed, "{:?}", report.checks);
    assert!(!report.guest_runtime_verified);
    assert!(!root.join("MUST_NOT_EXECUTE").exists());
    assert!(!std::path::Path::new("MUST_NOT_EXECUTE").exists());
    assert_eq!(fs::read(&config).unwrap(), before);
    assert!(!root.join("diagnostic-only.sock").exists());
    fs::set_permissions(binary, fs::Permissions::from_mode(0o600)).unwrap();
    let report = inspect(&config);
    assert!(!report.preflight_passed);
    assert!(report
        .checks
        .iter()
        .any(|c| c.id == "qemu_executable" && c.state == State::Failed && c.remedy.is_some()));
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn missing_configuration_is_a_structured_failure_without_creation() {
    let path = std::env::temp_dir().join(format!("fl-diag-missing-{}.toml", std::process::id()));
    let report = inspect(&path);
    assert!(!report.preflight_passed);
    assert_eq!(report.checks[0].state, State::Failed);
    assert!(!path.exists());
}

#[test]
fn oversized_configuration_is_rejected_before_reading() {
    let path = std::env::temp_dir().join(format!("fl-diag-large-{}.toml", std::process::id()));
    let file = fs::File::create(&path).unwrap();
    file.set_len(2 * 1024 * 1024).unwrap();
    drop(file);
    let report = inspect(&path);
    assert!(!report.preflight_passed);
    assert!(report.checks[0].detail.contains("1 MiB"));
    assert_eq!(fs::metadata(&path).unwrap().len(), 2 * 1024 * 1024);
    fs::remove_file(path).unwrap();
}
