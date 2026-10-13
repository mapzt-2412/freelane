use freelane::{
    library::{self, Entry},
    GuestOs,
};
use std::{fs, os::unix::fs::PermissionsExt};
#[test]
fn bookmark_updates_preserve_other_vms_and_reject_corrupt_library() {
    let root = std::env::temp_dir().join(format!("fl-library-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let path = root.join("library.toml");
    assert!(library::read(&path).unwrap().is_empty());
    let entry = |name: &str, file: &str| Entry {
        path: root.join(file),
        name: name.into(),
        guest_os: GuestOs::Windows,
    };
    library::record(&path, entry("First", "first.toml")).unwrap();
    library::record(&path, entry("Second", "second.toml")).unwrap();
    library::record(&path, entry("Renamed", "first.toml")).unwrap();
    let entries = library::read(&path).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "Renamed");
    assert_eq!(entries[1].name, "Second");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(!root.join("first.toml").exists());
    fs::write(&path, b"user corrupt library must be preserved").unwrap();
    let original = fs::read(&path).unwrap();
    assert!(library::record(&path, entry("Third", "third.toml")).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    fs::remove_dir_all(root).unwrap();
}
