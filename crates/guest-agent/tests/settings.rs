use freelane_guest_agent::settings::Settings;
use std::fs;
#[test]
fn absent_manifest_disables_transfers_and_bounded_install_manifest_loads_only_selections() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agent-install.json");
    let s = Settings::load(&path).unwrap();
    assert!(s.inbox.is_none() && s.outbox.is_none());
    let root = dir.path().join("explicit outbox");
    fs::create_dir(&root).unwrap();
    let document = serde_json::json!({"schema":1,"sha256":"a".repeat(64),"backup":null,"transfer":{"inbox":null,"outbox":root}});
    fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    let s = Settings::load(&path).unwrap();
    assert!(s.inbox.is_none());
    assert_eq!(s.outbox, Some(root));
    // Older manifests remain readable and never infer a folder.
    fs::write(
        &path,
        b"{\"schema\":1,\"sha256\":\"ignored-installation-metadata\"}",
    )
    .unwrap();
    let s = Settings::load(&path).unwrap();
    assert!(s.inbox.is_none() && s.outbox.is_none());
}
#[test]
fn corrupt_oversized_and_invalid_settings_are_refused_without_creating_roots() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agent-install.json");
    for bad in [
        "{}".to_string(),
        "{\"schema\":2}".into(),
        "x".repeat(4097),
        "{\"schema\":1,\"transfer\":{\"outbox\":\"relative\"}}".into(),
        "{\"schema\":1,\"transfer\":{\"inbox\":\"\"}}".into(),
        "{\"schema\":1,\"transfer\":{\"outbxo\":null}}".into(),
    ] {
        fs::write(&path, bad).unwrap();
        assert!(Settings::load(&path).is_err());
    }
    let missing = dir.path().join("missing");
    fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({"schema":1,"transfer":{"outbox":missing}})).unwrap(),
    )
    .unwrap();
    let s = Settings::load(&path).unwrap();
    assert!(!missing.exists());
    assert!(freelane_guest_agent::TransferLauncher::new(NoLaunch, None)
        .unwrap()
        .with_export_root(s.outbox.as_deref())
        .is_err());
    assert!(!missing.exists());
}
struct NoLaunch;
impl freelane_guest_agent::Launcher for NoLaunch {
    fn launch(&mut self, _: &str, _: &[String]) -> std::io::Result<u32> {
        panic!("settings never launch")
    }
}
#[cfg(unix)]
#[test]
fn settings_symlinks_and_nonregular_files_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("original");
    fs::write(&original, b"{\"schema\":1}").unwrap();
    let path = dir.path().join("agent-install.json");
    std::os::unix::fs::symlink(original, &path).unwrap();
    assert!(Settings::load(&path).is_err());
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(Settings::load(&path).is_err());
}
