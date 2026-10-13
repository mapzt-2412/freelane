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

#[test]
fn relink_repairs_moved_workspace_without_changing_guest_files() {
    let root = std::env::temp_dir().join(format!("fl-relink-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let old = root.join("old");
    fs::create_dir(&old).unwrap();
    let text = "name='Moved'\ncpus=2\nmemory_mib=4096\ndisk='disk.qcow2'\n";
    fs::write(old.join("vm.toml"), text).unwrap();
    fs::write(old.join("disk.qcow2"), b"guest bytes").unwrap();
    let library = root.join("library.toml");
    let original = old.join("vm.toml");
    library::record(
        &library,
        Entry {
            path: original.clone(),
            name: "Moved".into(),
            guest_os: GuestOs::Windows,
        },
    )
    .unwrap();
    let moved = root.join("moved");
    fs::rename(&old, &moved).unwrap();
    library::relink(&library, &original, &moved.join("vm.toml")).unwrap();
    let entries = library::read(&library).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        entries[0].path,
        fs::canonicalize(moved.join("vm.toml")).unwrap()
    );
    assert_eq!(fs::read_to_string(moved.join("vm.toml")).unwrap(), text);
    assert_eq!(fs::read(moved.join("disk.qcow2")).unwrap(), b"guest bytes");
    let saved = fs::read(&library).unwrap();
    assert!(library::relink(&library, &moved.join("vm.toml"), &moved.join("vm.toml")).is_err());
    assert_eq!(fs::read(&library).unwrap(), saved);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_relink_preserves_library_for_invalid_duplicate_and_busy_targets() {
    use fs2::FileExt;
    let root = std::env::temp_dir().join(format!("fl-relink-fail-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let library = root.join("library.toml");
    let old = root.join("missing.toml");
    let new = root.join("new.toml");
    library::record(
        &library,
        Entry {
            path: old.clone(),
            name: "Missing".into(),
            guest_os: GuestOs::Windows,
        },
    )
    .unwrap();
    let original = fs::read(&library).unwrap();
    for content in [
        "invalid",
        "name='Other'\nguest_os='ubuntu'\ncpus=2\nmemory_mib=4096\ndisk='disk.qcow2'\n",
        "name='Moved'\ncpus=2\nmemory_mib=4096\ndisk='absent.qcow2'\n",
    ] {
        fs::write(&new, content).unwrap();
        fs::write(root.join("disk.qcow2"), b"disk").unwrap();
        assert!(library::relink(&library, &old, &new).is_err());
        assert_eq!(fs::read(&library).unwrap(), original);
    }
    fs::write(
        &new,
        "name='Moved'\ncpus=2\nmemory_mib=4096\ndisk='disk.qcow2'\n",
    )
    .unwrap();
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join("library.lock"))
        .unwrap();
    lock.lock_exclusive().unwrap();
    assert!(library::relink(&library, &old, &new)
        .unwrap_err()
        .to_string()
        .contains("busy"));
    assert_eq!(fs::read(&library).unwrap(), original);
    drop(lock);
    library::record(
        &library,
        Entry {
            path: new.clone(),
            name: "Already saved".into(),
            guest_os: GuestOs::Windows,
        },
    )
    .unwrap();
    let saved = fs::read(&library).unwrap();
    assert!(library::relink(&library, &old, &new).is_err());
    assert_eq!(fs::read(&library).unwrap(), saved);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn relink_rejects_an_existing_bookmark_through_a_directory_symlink() {
    let root = std::env::temp_dir().join(format!("fl-relink-alias-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let new = root.join("new.toml");
    fs::write(
        &new,
        "name='Moved'\ncpus=2\nmemory_mib=4096\ndisk='disk.qcow2'\n",
    )
    .unwrap();
    fs::write(root.join("disk.qcow2"), b"guest disk").unwrap();
    let alias = root.join("alias");
    std::os::unix::fs::symlink(&root, &alias).unwrap();
    let path = root.join("library.toml");
    let missing = root.join("missing.toml");
    for entry in [
        Entry {
            path: missing.clone(),
            name: "Missing".into(),
            guest_os: GuestOs::Windows,
        },
        Entry {
            path: alias.join("new.toml"),
            name: "Aliased".into(),
            guest_os: GuestOs::Windows,
        },
    ] {
        library::record(&path, entry).unwrap();
    }
    let original = fs::read(&path).unwrap();
    assert!(library::relink(&path, &missing, &new).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    fs::remove_dir_all(root).unwrap();
}
