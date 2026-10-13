use freelane::{provision::Request, qemu::Accelerator, GuestOs};
use std::{fs, os::unix::fs::PermissionsExt};
#[test]
fn rejected_creation_preserves_existing_data_and_source_templates() {
    let root = std::env::temp_dir().join(format!("fl-provision-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    for name in ["installer.iso", "firmware.fd", "vars.fd"] {
        fs::write(root.join(name), b"validation fixture only").unwrap();
    }
    let tool = root.join("qemu-img");
    fs::write(&tool, "#!/bin/sh\nexit 99\n").unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
    let mut r = Request {
        name: "Ubuntu test".into(),
        guest_os: GuestOs::Ubuntu,
        directory: root.join("new"),
        installer: root.join("installer.iso"),
        firmware: root.join("firmware.fd"),
        variable_template: root.join("vars.fd"),
        qemu_img: tool,
        accelerator: Accelerator::Tcg,
        cpus: 2,
        memory_mib: 2048,
        disk_gib: 8,
        arm64_media_confirmed: false,
    };
    assert!(r.create().unwrap_err().to_string().contains("Confirm"));
    assert!(!r.directory.exists());
    r.arm64_media_confirmed = true;
    r.disk_gib = 7;
    assert!(r.create().is_err());
    assert!(!r.directory.exists());
    r.disk_gib = 8;
    let alias = root.join("alias.fd");
    fs::hard_link(&r.firmware, &alias).unwrap();
    r.variable_template = alias;
    assert!(r.create().unwrap_err().to_string().contains("distinct"));
    assert!(!r.directory.exists());
    r.variable_template = root.join("vars.fd");
    r.disk_gib = 4096;
    assert!(r
        .create()
        .unwrap_err()
        .to_string()
        .contains("storage insufficient"));
    assert!(!r.directory.exists());
    r.disk_gib = 8;
    fs::create_dir(&r.directory).unwrap();
    fs::write(r.directory.join("user.txt"), b"must survive").unwrap();
    assert!(r.create().unwrap_err().to_string().contains("existing"));
    assert_eq!(
        fs::read(r.directory.join("user.txt")).unwrap(),
        b"must survive"
    );
    assert_eq!(
        fs::read(&r.variable_template).unwrap(),
        b"validation fixture only"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn interrupted_creation_marker_refuses_backend_start_and_plan() {
    let root = std::env::temp_dir().join(format!("fl-creation-marker-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let cfg = root.join("vm.toml");
    fs::write(
        &cfg,
        "name='incomplete'\ncpus=2\nmemory_mib=4096\ndisk='unused.qcow2'\n",
    )
    .unwrap();
    fs::write(root.join("freelane-creation.json"), b"incomplete").unwrap();
    let error = match freelane::qemu::QemuBackend::new(&cfg) {
        Ok(_) => panic!("unfinished creation accepted"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("creation"));
    let parsed = freelane::Config::parse(&fs::read_to_string(&cfg).unwrap()).unwrap();
    assert!(
        freelane::qemu::arguments(&parsed, &root, &root.join("qmp.sock"))
            .unwrap_err()
            .to_string()
            .contains("creation")
    );
    assert!(root.join("freelane-creation.json").exists());
    fs::remove_dir_all(root).unwrap();
}
