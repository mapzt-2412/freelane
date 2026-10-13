use freelane::{checkpoint, qemu, Config};
use fs2::FileExt;
use std::{
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
};
#[test]
fn running_workspace_lock_blocks_checkpoint_without_touching_output() {
    let root = std::env::temp_dir().join(format!("fl-checkpoint-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let cfg = root.join("vm.toml");
    fs::write(&cfg, "fixture: should never be parsed while lock held").unwrap();
    let runtime = qemu::runtime_dir(&cfg).unwrap();
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(runtime.join("lock"))
        .unwrap();
    lock.try_lock_exclusive().unwrap();
    let output = root.join("snapshot");
    let error = checkpoint::create(
        &cfg,
        &output,
        "test",
        std::path::Path::new("MUST_NOT_EXECUTE"),
    )
    .unwrap_err();
    assert!(error.to_string().contains("Stop the VM"));
    assert!(!output.exists());
    assert_eq!(
        fs::read_to_string(&cfg).unwrap(),
        "fixture: should never be parsed while lock held"
    );
    drop(lock);
    fs::remove_dir_all(runtime).unwrap();
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn checkpoint_marker_prevents_backend_plan_and_source_is_preserved() {
    let root = std::env::temp_dir().join(format!("fl-checkpoint-marker-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join(checkpoint::MARKER), b"incomplete").unwrap();
    let config =
        Config::parse("name='fixture'\ncpus=2\nmemory_mib=2048\ndisk='missing.qcow2'").unwrap();
    let error = qemu::arguments(&config, &root, &root.join("qmp.sock")).unwrap_err();
    assert!(error.to_string().contains("immutable"));
    assert_eq!(
        fs::read(root.join(checkpoint::MARKER)).unwrap(),
        b"incomplete"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn restore_rejects_traversal_manifest_before_creating_output() {
    let root = std::env::temp_dir().join(format!("fl-restore-path-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let files = [
        "../outside",
        "disk.qcow2",
        "firmware.fd",
        "nvram.fd",
        "vm.toml",
    ]
    .into_iter()
    .map(|n| (n.to_owned(), "0".repeat(64)))
    .collect::<std::collections::BTreeMap<_, _>>();
    let manifest = serde_json::json!({"version":1,"label":"test","created_unix_seconds":0,"kind":"stopped_workspace_copy","guest_runtime_verified":false,"files_sha256":files});
    fs::write(
        root.join(checkpoint::MARKER),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let output = root.join("fresh");
    let error = checkpoint::restore(&root, &output).unwrap_err();
    assert!(error.to_string().contains("Unsafe"));
    assert!(!output.exists());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn restore_never_overwrites_existing_user_directory() {
    let root = std::env::temp_dir().join(format!("fl-restore-existing-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::write(root.join("user.txt"), b"original user data").unwrap();
    let error = checkpoint::restore(std::path::Path::new("MUST_NOT_READ"), &root).unwrap_err();
    assert!(error.to_string().contains("new directory"));
    assert_eq!(
        fs::read(root.join("user.txt")).unwrap(),
        b"original user data"
    );
    fs::remove_dir_all(root).unwrap();
}
