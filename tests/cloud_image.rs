use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};
#[test]
fn import_creates_independent_disk_and_preserves_source() {
    let root = std::env::temp_dir().join(format!("fl-cloud-copy-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let source = root.join("source.qcow2");
    assert!(Command::new("qemu-img")
        .args(["create", "-f", "qcow2"])
        .arg(&source)
        .arg("1G")
        .status()
        .unwrap()
        .success());
    let original = fs::read(&source).unwrap();
    let destination = root.join("new.qcow2");
    freelane::cloud_image::import(Path::new("qemu-img"), &source, &destination, 8).unwrap();
    assert_eq!(fs::read(&source).unwrap(), original);
    let output = Command::new("qemu-img")
        .args(["info", "--output=json"])
        .arg(&destination)
        .output()
        .unwrap();
    assert!(output.status.success());
    let info: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        info["virtual-size"].as_u64().unwrap(),
        8 * 1024 * 1024 * 1024
    );
    assert!(info.get("backing-filename").is_none());
    let saved = fs::read(&destination).unwrap();
    assert!(
        freelane::cloud_image::import(Path::new("qemu-img"), &source, &destination, 8).is_err()
    );
    assert_eq!(fs::read(&destination).unwrap(), saved);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn reject_backing_images_invalid_formats_and_shrinking_before_output() {
    let root = std::env::temp_dir().join(format!("fl-cloud-reject-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let source = root.join("source.qcow2");
    let destination = root.join("new.qcow2");
    fs::write(&source, b"not a qcow image").unwrap();
    assert!(
        freelane::cloud_image::import(Path::new("qemu-img"), &source, &destination, 8).is_err()
    );
    assert!(!destination.exists());
    fs::remove_file(&source).unwrap();
    assert!(Command::new("qemu-img")
        .args(["create", "-f", "qcow2"])
        .arg(&source)
        .arg("16G")
        .status()
        .unwrap()
        .success());
    assert!(
        freelane::cloud_image::import(Path::new("qemu-img"), &source, &destination, 8).is_err()
    );
    assert!(!destination.exists());
    let backing = root.join("backing.qcow2");
    assert!(Command::new("qemu-img")
        .args(["create", "-f", "qcow2", "-F", "qcow2", "-b"])
        .arg(&source)
        .arg(&backing)
        .status()
        .unwrap()
        .success());
    assert!(
        freelane::cloud_image::import(Path::new("qemu-img"), &backing, &destination, 32).is_err()
    );
    assert!(!destination.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cloud_creation_publishes_independent_disk_without_iso_and_reports_import_stage() {
    let root = std::env::temp_dir().join(format!("fl-cloud-provision-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let source = root.join("source.qcow2");
    assert!(Command::new("qemu-img")
        .args(["create", "-f", "qcow2"])
        .arg(&source)
        .arg("1G")
        .status()
        .unwrap()
        .success());
    let original = fs::read(&source).unwrap();
    fs::write(root.join("code.fd"), b"nonbootable test firmware").unwrap();
    fs::write(root.join("vars.fd"), b"nonbootable test vars").unwrap();
    let request = root.join("create.toml");
    fs::write(&request,"name='Cloud test'\nguest_os='ubuntu'\nsource='ubuntu_cloud_image'\ndirectory='vm'\ninstaller='source.qcow2'\nfirmware='code.fd'\nvariable_template='vars.fd'\nqemu_img='qemu-img'\naccelerator='tcg'\ncpus=2\nmemory_mib=2048\ndisk_gib=8\narm64_media_confirmed=true\n").unwrap();
    let request = freelane::provision::load(&request).unwrap();
    let mut rejected = request.clone();
    rejected.guest_os = freelane::GuestOs::Windows;
    assert!(rejected
        .create()
        .unwrap_err()
        .to_string()
        .contains("only for Ubuntu"));
    assert!(!rejected.directory.exists());
    rejected.guest_os = freelane::GuestOs::Ubuntu;
    rejected.arm64_media_confirmed = false;
    assert!(rejected
        .create()
        .unwrap_err()
        .to_string()
        .contains("Confirm"));
    assert!(!rejected.directory.exists());
    let mut stages = Vec::new();
    let path = request
        .create_with_progress(|stage| stages.push(format!("{stage:?}")))
        .unwrap();
    assert!(stages.iter().any(|stage| stage == "ImportingImage"));
    let config = freelane::Config::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(config.installer.is_none());
    assert_eq!(fs::read(&source).unwrap(), original);
    assert!(!path
        .parent()
        .unwrap()
        .join(freelane::provision::MARKER)
        .exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reject_encrypted_external_dirty_headers_and_failed_conversion_preserves_source() {
    let root = std::env::temp_dir().join(format!("fl-cloud-failure-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let source = root.join("source.qcow2");
    assert!(Command::new("qemu-img")
        .args(["create", "-f", "qcow2"])
        .arg(&source)
        .arg("1G")
        .status()
        .unwrap()
        .success());
    let original = fs::read(&source).unwrap();
    for (offset, value) in [(35, 1), (79, 1), (79, 4)] {
        let mut changed = original.clone();
        changed[offset] = value;
        fs::write(&source, changed).unwrap();
        assert!(freelane::cloud_image::validate(&source, 8).is_err());
    }
    fs::write(&source, &original).unwrap();
    let tool = root.join("fail-qemu-img");
    fs::write(&tool, "#!/bin/sh\nexit 19\n").unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o700)).unwrap();
    let output = root.join("result.qcow2");
    assert!(freelane::cloud_image::import(&tool, &source, &output, 8).is_err());
    assert!(!output.exists());
    assert_eq!(fs::read(&source).unwrap(), original);
    assert!(!fs::read_dir(&root).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".cloud-")));
    fs::remove_dir_all(root).unwrap();
}
