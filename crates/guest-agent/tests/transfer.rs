#![cfg(any(target_os = "linux", windows))]
use freelane_guest_agent::transfer::Transfer;
use sha2::{Digest, Sha256};
use std::fs;
fn hash(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
#[test]
fn verified_unicode_large_transfer_preserves_conflicts() {
    let root = tempfile::tempdir().unwrap();
    let data = vec![42; 3 * 1024 * 1024];
    let mut t = Transfer::begin(
        root.path(),
        "Tài liệu mới.txt",
        data.len() as u64,
        &hash(&data),
    )
    .unwrap();
    assert!(!root.path().join("Tài liệu mới.txt").exists());
    for (n, chunk) in data.chunks(16384).enumerate() {
        t.write((n * 16384) as u64, chunk).unwrap();
    }
    t.finish().unwrap();
    assert_eq!(
        fs::read(root.path().join("Tài liệu mới.txt")).unwrap(),
        data
    );
    let mut conflict = Transfer::begin(root.path(), "Tài liệu mới.txt", 1, &hash(b"x")).unwrap();
    conflict.write(0, b"x").unwrap();
    assert!(conflict.finish().is_err());
    assert_eq!(
        fs::read(root.path().join("Tài liệu mới.txt")).unwrap(),
        data
    );
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}
#[test]
fn cancellation_bad_hash_and_replayed_chunk_leave_no_destination() {
    let root = tempfile::tempdir().unwrap();
    let mut t = Transfer::begin(root.path(), "a.txt", 3, &hash(b"abc")).unwrap();
    t.write(0, b"a").unwrap();
    assert!(t.write(0, b"a").is_err());
    assert!(t.write(1, b"bcd").is_err());
    drop(t);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    let mut t = Transfer::begin(root.path(), "a.txt", 3, &hash(b"xyz")).unwrap();
    t.write(0, b"abc").unwrap();
    assert!(t.finish().is_err());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
}
#[test]
fn paths_and_windows_aliases_are_refused() {
    let root = tempfile::tempdir().unwrap();
    for name in [
        "../a",
        "a/b",
        "a\\b",
        "a:ads",
        "CON.txt",
        "Lpt1",
        "a.",
        "a ",
        ".",
        "",
        ".freelane-stage-user",
    ] {
        assert!(
            Transfer::begin(root.path(), name, 0, &hash(b"")).is_err(),
            "{name}"
        );
    }
}
#[cfg(unix)]
#[test]
fn renamed_root_does_not_redirect_writes() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("inbox");
    fs::create_dir(&root).unwrap();
    let mut t = Transfer::begin(&root, "a.txt", 3, &hash(b"abc")).unwrap();
    fs::rename(&root, parent.path().join("original")).unwrap();
    fs::create_dir(&root).unwrap();
    t.write(0, b"abc").unwrap();
    t.finish().unwrap();
    assert!(!root.join("a.txt").exists());
    assert_eq!(
        fs::read(parent.path().join("original/a.txt")).unwrap(),
        b"abc"
    );
}
#[cfg(target_os = "linux")]
#[test]
fn in_progress_bytes_have_no_replaceable_directory_entry() {
    let root = tempfile::tempdir().unwrap();
    let mut t = Transfer::begin(root.path(), "a.txt", 3, &hash(b"abc")).unwrap();
    t.write(0, b"abc").unwrap();
    assert_eq!(
        fs::read_dir(root.path()).unwrap().count(),
        0,
        "staging must not be replaceable by an inbox writer"
    );
    t.finish().unwrap();
    assert_eq!(fs::read(root.path().join("a.txt")).unwrap(), b"abc");
}
