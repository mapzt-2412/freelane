use std::fs;
fn record(extent: u32, length: u32, name: &[u8], directory: bool) -> Vec<u8> {
    let n = 33 + name.len() + usize::from(name.len().is_multiple_of(2));
    let mut r = vec![0; n];
    r[0] = n as u8;
    r[2..6].copy_from_slice(&extent.to_le_bytes());
    r[6..10].copy_from_slice(&extent.to_be_bytes());
    r[10..14].copy_from_slice(&length.to_le_bytes());
    r[14..18].copy_from_slice(&length.to_be_bytes());
    r[25] = if directory { 2 } else { 0 };
    r[32] = name.len() as u8;
    r[33..33 + name.len()].copy_from_slice(name);
    r
}
fn fixture(machine: u16) -> Vec<u8> {
    let mut iso = vec![0; 36 * 2048];
    let p = 16 * 2048;
    iso[p] = 1;
    iso[p + 1..p + 6].copy_from_slice(b"CD001");
    iso[p + 6] = 1;
    let root = record(32, 2048, &[0], true);
    iso[p + 156..p + 156 + root.len()].copy_from_slice(&root);
    for (sector, entry) in [
        (32, record(33, 2048, b"EFI", true)),
        (33, record(34, 2048, b"BOOT", true)),
        (34, record(35, 512, b"BOOTAA64.EFI;1", false)),
    ] {
        iso[sector * 2048..sector * 2048 + entry.len()].copy_from_slice(&entry);
    }
    let e = 35 * 2048;
    iso[e..e + 2].copy_from_slice(b"MZ");
    iso[e + 60..e + 64].copy_from_slice(&128u32.to_le_bytes());
    iso[e + 128..e + 132].copy_from_slice(b"PE\0\0");
    iso[e + 132..e + 134].copy_from_slice(&machine.to_le_bytes());
    iso
}
#[test]
fn validates_arm64_loader_without_modifying_media() {
    let dir = std::env::temp_dir().join(format!("fl-media-arm-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("Tiếng Việt ARM.iso");
    let bytes = fixture(0xaa64);
    fs::write(&path, &bytes).unwrap();
    freelane::media::verify_arm64_iso(&path).unwrap();
    assert_eq!(fs::read(path).unwrap(), bytes);
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn refuses_wrong_architecture_missing_loader_and_out_of_bounds_extents() {
    let dir = std::env::temp_dir().join(format!("fl-media-bad-{}", std::process::id()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("bad.iso");
    for bytes in [fixture(0x8664), vec![0; 36 * 2048], {
        let mut b = fixture(0xaa64);
        b[32 * 2048 + 2..32 * 2048 + 6].copy_from_slice(&u32::MAX.to_le_bytes());
        b[32 * 2048 + 6..32 * 2048 + 10].copy_from_slice(&u32::MAX.to_be_bytes());
        b
    }] {
        fs::write(&path, &bytes).unwrap();
        assert!(freelane::media::verify_arm64_iso(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn zip_with_arm64_loader_is_never_accepted_as_cd_media() {
    let root = std::env::temp_dir().join(format!("fl-media-archive-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let path = root.join("misnamed.iso");
    fs::write(&path, include_bytes!("fixtures/not-an-iso-arm64.zip")).unwrap();
    let result = freelane::media::verify_arm64_iso(&path);
    fs::remove_dir_all(root).unwrap();
    assert!(result.is_err(), "ZIP archive accepted as raw ISO/CD media");
}
