use freelane::{
    qemu::ShareMode,
    sharing::{inspect, HostState},
    Config,
};
use std::{fs, path::Path};
fn config() -> Config {
    Config::parse("name='share test'\nguest_os='ubuntu'\ncpus=1\nmemory_mib=2048\ndisk='disk'\n[runtime]\nfirmware='code.fd'\nnvram='vars.fd'\nshared_directory='shared'\nshare_mode='virtio9p_readonly'\n").unwrap()
}
#[test]
fn availability_does_not_infer_mount_or_recreate_missing_paths() {
    let root = std::env::temp_dir().join(format!("fl-share-status-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let config = config();
    let report = inspect(&config, &root);
    assert_eq!(report.host, HostState::Missing);
    assert!(!root.join("shared").exists());
    assert_eq!(report.guest_mount, "unknown");
    fs::create_dir(root.join("shared")).unwrap();
    let report = inspect(&config, &root);
    assert_eq!(report.host, HostState::Available);
    assert_eq!(report.mode, Some(ShareMode::Virtio9pReadonly));
    assert_eq!(report.guest_mount, "unknown");
    fs::remove_dir(root.join("shared")).unwrap();
    fs::write(root.join("shared"), b"original").unwrap();
    assert_eq!(inspect(&config, &root).host, HostState::NotDirectory);
    assert_eq!(fs::read(root.join("shared")).unwrap(), b"original");
    fs::remove_dir_all(root).unwrap();
    let mut disabled = config.clone();
    disabled.runtime.as_mut().unwrap().shared_directory = None;
    assert_eq!(inspect(&disabled, Path::new(".")).host, HostState::Disabled);
}
