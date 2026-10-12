use freelane::app_launcher::{bundle_files, AppProfile};
use std::{fs, path::PathBuf};
fn profile(vm: PathBuf) -> AppProfile {
    AppProfile {
        name: "Notepad & Việt <test>".into(),
        vm,
        executable: r"C:\Windows\System32\notepad.exe".into(),
        arguments: vec!["C:\\File a & b.txt".into()],
        boot_timeout_seconds: 600,
    }
}
#[test]
fn app_profiles_reject_ambiguous_paths_and_preserve_literal_arguments() {
    let good = profile("vm.toml".into());
    good.validate().unwrap();
    for executable in [
        "notepad.exe",
        r"C:relative.exe",
        r"C:\test.cmd",
        r"C:\test.exe\0",
    ] {
        let mut bad = good.clone();
        bad.executable = executable.into();
        assert!(bad.validate().is_err());
    }
    let mut bad = good.clone();
    bad.arguments = vec!["bad\0argument".into()];
    assert!(bad.validate().is_err());
    let parsed = AppProfile::parse(&toml::to_string(&good).unwrap()).unwrap();
    assert_eq!(parsed.arguments, good.arguments);
    let mut bad = good;
    bad.boot_timeout_seconds = 0;
    assert!(bad.validate().is_err());
}
#[test]
fn offline_bundle_has_literal_profile_escaped_metadata_and_refuses_overwrite() {
    let root = std::env::temp_dir().join(format!("freelane-app-bundle-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let vm = root.join("vm.toml");
    fs::write(&vm, "fixture").unwrap();
    let binary = root.join("fake-binary");
    fs::write(&binary, "offline fixture, not Mach-O").unwrap();
    let app = root.join("Test & App.app");
    let p = profile(vm.clone());
    bundle_files(&p, &app, &binary, &binary).unwrap();
    let saved = AppProfile::load(&app.join("Contents/Resources/app.toml")).unwrap();
    assert_eq!(saved.vm, fs::canonicalize(vm).unwrap());
    assert_eq!(saved.arguments, p.arguments);
    let plist = fs::read_to_string(app.join("Contents/Info.plist")).unwrap();
    assert!(plist.contains("Notepad &amp; Việt &lt;test&gt;"));
    assert!(!plist.contains("<string>Notepad & Việt"));
    assert!(bundle_files(&p, &app, &binary, &binary).is_err());
    let duplicate = root.join("Broken.app");
    assert!(bundle_files(&p, &duplicate, &root.join("missing"), &binary).is_err());
    assert!(!duplicate.exists());
    fs::remove_dir_all(root).unwrap();
}
