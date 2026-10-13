use freelane::{qemu, storage, GuestOs};
use std::{
    fs,
    os::unix::fs::{symlink, MetadataExt},
    path::Path,
};
#[test]
fn destination_capacity_follows_selected_folder_alias_without_creating_workspace() {
    let root = std::env::temp_dir().join(format!("fl-destination-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let storage = root.join("selected-storage");
    fs::create_dir(&storage).unwrap();
    let alias = root.join("folder-alias");
    symlink(&storage, &alias).unwrap();
    let destination = alias.join("new-vm/nested");
    let report = storage::inspect_destination(&destination).unwrap();
    assert_eq!(report.filesystem_path, fs::canonicalize(&storage).unwrap());
    assert!(report.available_bytes > 0);
    assert!(!destination.exists());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn real_qcow2_large_virtual_disk_uses_only_small_initial_physical_storage() {
    let root = std::env::temp_dir().join(format!("fl-sparse-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let firmware = root.join("code.fd");
    let variables = root.join("vars.fd");
    fs::write(&firmware, b"nonbootable fixture firmware").unwrap();
    fs::write(&variables, b"nonbootable fixture vars").unwrap();
    let config = qemu::initialize_guest_sized(
        &root.join("vm"),
        &firmware,
        &variables,
        Path::new("qemu-img"),
        GuestOs::Windows,
        8192,
    )
    .unwrap();
    let cfg = freelane::Config::parse(&fs::read_to_string(config).unwrap()).unwrap();
    let disk = root.join("vm").join(&cfg.disk);
    let meta = fs::metadata(&disk).unwrap();
    assert!(
        meta.blocks() * 512 < 64 * 1024 * 1024,
        "large virtual capacity allocated too much physical storage"
    );
    let info = std::process::Command::new("qemu-img")
        .args(["info", "--output=json"])
        .arg(&disk)
        .output()
        .unwrap();
    assert!(info.status.success());
    let value: serde_json::Value = serde_json::from_slice(&info.stdout).unwrap();
    assert_eq!(
        value["virtual-size"],
        serde_json::json!(8192u64 * 1024 * 1024 * 1024)
    );
    assert_eq!(fs::read(&variables).unwrap(), b"nonbootable fixture vars");
    fs::remove_dir_all(root).unwrap();
}
