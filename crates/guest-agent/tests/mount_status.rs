use freelane_guest_agent::mounts::parse;
#[test]
fn mountinfo_reports_only_fixed_share_and_decodes_spaces() {
    let text="20 1 0:1 / /mnt/Shared\\040Files rw,relatime - 9p freelane rw\n21 1 0:2 / /private ro - ext4 /dev/sda ro\n22 1 0:3 / /mnt/readonly ro - 9p freelane ro\n";
    let mounts = parse(text).unwrap();
    assert_eq!(mounts.len(), 2);
    assert_eq!(mounts[0].path, "/mnt/Shared Files");
    assert!(!mounts[0].readonly);
    assert!(mounts[1].readonly);
}
#[test]
fn malformed_or_overlong_share_metadata_is_not_reported_as_unmounted() {
    assert!(parse("20 1 0:1 / relative rw - 9p freelane rw\n").is_err());
    assert!(parse(&format!(
        "20 1 0:1 / /{} rw - 9p freelane rw\n",
        "x".repeat(1025)
    ))
    .is_err());
    assert!(parse(&"20 1 0:1 / /mnt ro - 9p freelane ro\n".repeat(17)).is_err());
}
