use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
fn fixture(label: &str) -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("fl-settings-{label}-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    for name in ["disk.qcow2", "code.fd", "vars.fd"] {
        fs::write(root.join(name), format!("owned fixture {name}")).unwrap();
    }
    let config = root.join("vm.toml");
    fs::write(&config,"name='Original VM'\ncpus=2\nmemory_mib=4096\ndisk='disk.qcow2'\n[runtime]\nfirmware='code.fd'\nnvram='vars.fd'\ndisk_format='qcow2'\naccelerator='tcg'\nnetwork=false\n").unwrap();
    (root, config)
}
fn cleanup(root: &Path, config: &Path) {
    let runtime = freelane::qemu::runtime_dir(config).unwrap();
    if runtime.exists() {
        fs::remove_dir_all(runtime).unwrap();
    }
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn settings_roundtrip_preserves_vm_storage_identity_and_creates_backup() {
    let (root, config) = fixture("roundtrip");
    let original = fs::read(&config).unwrap();
    let payload = root.join("settings.json");
    freelane::settings_bundle::export_saved(&config, &payload).unwrap();
    assert_eq!(
        fs::metadata(&payload).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&payload).unwrap()).unwrap();
    assert!(value.get("disk").is_none());
    assert!(value.get("firmware").is_none());
    assert!(value.get("tpm").is_none());
    value["cpus"] = 4.into();
    value["memory_mib"] = 8192.into();
    value["network"] = true.into();
    fs::write(&payload, serde_json::to_vec(&value).unwrap()).unwrap();
    let backup = freelane::settings_bundle::import_saved(&config, &payload).unwrap();
    assert_eq!(fs::read(backup).unwrap(), original);
    let updated = freelane::Config::parse(&fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(updated.cpus, 4);
    assert_eq!(updated.memory_mib, 8192);
    assert!(updated.runtime.as_ref().unwrap().network);
    assert_eq!(updated.disk, PathBuf::from("disk.qcow2"));
    assert_eq!(
        updated.runtime.as_ref().unwrap().nvram,
        PathBuf::from("vars.fd")
    );
    for name in ["disk.qcow2", "code.fd", "vars.fd"] {
        assert_eq!(
            fs::read(root.join(name)).unwrap(),
            format!("owned fixture {name}").as_bytes()
        );
    }
    cleanup(&root, &config);
}
#[test]
fn unknown_identity_fields_and_invalid_resources_never_change_target() {
    let (root, config) = fixture("invalid");
    let original = fs::read(&config).unwrap();
    let payload = root.join("settings.json");
    freelane::settings_bundle::export_saved(&config, &payload).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&payload).unwrap()).unwrap();
    for changed in [
        {
            let mut v = value.clone();
            v["disk"] = "/tmp/another-user.qcow2".into();
            v
        },
        {
            let mut v = value.clone();
            v["cpus"] = 0.into();
            v
        },
        {
            let mut v = value.clone();
            v["version"] = 99.into();
            v
        },
        {
            let mut v = value.clone();
            v["guest_os"] = "ubuntu".into();
            v
        },
        {
            let mut v = value.clone();
            v["installer"] = "relative.iso".into();
            v
        },
    ] {
        fs::write(&payload, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(freelane::settings_bundle::import_saved(&config, &payload).is_err());
        assert_eq!(fs::read(&config).unwrap(), original);
    }
    cleanup(&root, &config);
}
#[test]
fn export_never_overwrites_existing_user_file() {
    let (root, config) = fixture("existing");
    let payload = root.join("settings.json");
    fs::write(&payload, b"existing user data").unwrap();
    assert!(freelane::settings_bundle::export_saved(&config, &payload).is_err());
    assert_eq!(fs::read(&payload).unwrap(), b"existing user data");
    cleanup(&root, &config);
}

#[test]
fn busy_vm_refuses_import_and_keeps_original_configuration() {
    use fs2::FileExt;
    let (root, config) = fixture("busy");
    let original = fs::read(&config).unwrap();
    let payload = root.join("settings.json");
    freelane::settings_bundle::export_saved(&config, &payload).unwrap();
    let runtime = freelane::qemu::runtime_dir(&config).unwrap();
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let lock = fs::OpenOptions::new()
        .write(true)
        .read(true)
        .create_new(true)
        .open(runtime.join("lock"))
        .unwrap();
    lock.lock_exclusive().unwrap();
    assert!(freelane::settings_bundle::import_saved(&config, &payload)
        .unwrap_err()
        .to_string()
        .contains("Shut down"));
    assert_eq!(fs::read(&config).unwrap(), original);
    drop(lock);
    cleanup(&root, &config);
}

#[test]
fn hvf_import_clears_tcg_threading_and_preserves_identity() {
    let (root, path) = fixture("hvf");
    let payload = root.join("settings.json");
    freelane::settings_bundle::export_saved(&path, &payload).unwrap();
    let mut bundle = freelane::settings_bundle::load(&payload).unwrap();
    bundle.accelerator = freelane::qemu::Accelerator::Hvf;
    let mut target = freelane::Config::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    target.runtime.as_mut().unwrap().tpm = Some(freelane::qemu::TpmConfig {
        state: root.join("tpm"),
        binary: "swtpm".into(),
    });
    for threading in [
        freelane::qemu::TcgThreading::Single,
        freelane::qemu::TcgThreading::Multi,
    ] {
        target.runtime.as_mut().unwrap().tcg_threading = Some(threading);
        let updated = bundle.apply(&target).unwrap();
        let before = target.runtime.as_ref().unwrap();
        let after = updated.runtime.as_ref().unwrap();
        assert!(after.tcg_threading.is_none());
        assert_eq!(updated.disk, target.disk);
        assert_eq!(after.firmware, before.firmware);
        assert_eq!(after.nvram, before.nvram);
        assert_eq!(
            serde_json::to_value(&after.tpm).unwrap(),
            serde_json::to_value(&before.tpm).unwrap()
        );
    }
    cleanup(&root, &path);
}

#[test]
fn untrusted_input_is_bounded_and_parse_errors_do_not_echo_content() {
    let (root, config) = fixture("bounded");
    let payload = root.join("settings.json");
    fs::write(&payload, b"{\"private-secret-value\": broken}").unwrap();
    let error = freelane::settings_bundle::load(&payload)
        .unwrap_err()
        .to_string();
    assert!(error.contains("line"));
    assert!(!error.contains("private-secret-value"));
    fs::write(&payload, vec![b' '; 1024 * 1024 + 1]).unwrap();
    assert!(freelane::settings_bundle::load(&payload)
        .unwrap_err()
        .to_string()
        .contains("1 MiB"));
    let fifo = root.join("fifo");
    let cpath = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
    assert!(freelane::settings_bundle::load(&fifo)
        .unwrap_err()
        .to_string()
        .contains("regular file"));
    cleanup(&root, &config);
}
