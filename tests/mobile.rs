use freelane::mobile::{Acceleration, Platform, Profile};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn android() -> Profile {
    Profile::parse("name='Phone'\nplatform='android'\n[android]\nsdk_root='sdk with spaces'\navd='FreeLane_Test'\n").unwrap()
}
fn ios() -> Profile {
    Profile::parse("name='Simulator'\nplatform='ios'\n[ios]\ndevice_udid='12345678-1234-1234-1234-123456789abc'\n").unwrap()
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "freelane-mobile-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(fs::canonicalize(p).unwrap())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn profiles_reject_ambiguous_platforms_aliases_and_bad_resource_targets() {
    let original = android();
    assert_eq!(
        original.android.as_ref().unwrap().acceleration,
        Acceleration::Hardware
    );
    for (port, cpus, ram) in [
        (5555, 2, 2048),
        (6000, 2, 2048),
        (5554, 0, 2048),
        (5554, 2, 128),
    ] {
        let mut p = original.clone();
        let a = p.android.as_mut().unwrap();
        a.console_port = port;
        a.cpus = cpus;
        a.memory_mib = ram;
        assert!(p.validate().is_err());
    }
    let mut p = original.clone();
    p.ios = ios().ios;
    assert!(p.validate().is_err());
    for name in [
        "booted",
        "all",
        "../bad",
        "12345678-1234-1234-1234-123456789xyz",
    ] {
        let mut p = ios();
        p.ios.as_mut().unwrap().device_udid = name.into();
        assert!(p.validate().is_err());
    }
    let mut p = original;
    p.android.as_mut().unwrap().avd = "test -wipe-data".into();
    assert!(p.validate().is_err());
}
#[test]
fn android_plan_preserves_explicit_acceleration_and_never_wipes_userdata() {
    let mut p = android();
    let plan = p.start_plan(Path::new("/tmp/mobile root")).unwrap();
    assert_eq!(
        plan.program,
        Path::new("/tmp/mobile root/sdk with spaces/emulator/emulator")
    );
    assert!(plan.arguments.windows(2).any(|p| p == ["-accel", "on"]));
    assert!(!plan.arguments.iter().any(|s| s == "-wipe-data"));
    p.android.as_mut().unwrap().acceleration = Acceleration::Software;
    assert!(p
        .start_plan(Path::new("/tmp"))
        .unwrap()
        .arguments
        .windows(2)
        .any(|p| p == ["-accel", "off"]));
}
#[test]
fn ios_commands_target_exact_uuid_and_simulator_app_not_ipa() {
    let p = ios();
    assert_eq!(p.platform, Platform::Ios);
    let plan = p.start_plan(Path::new("/tmp")).unwrap();
    assert_eq!(plan.program, Path::new("xcrun"));
    assert_eq!(
        plan.arguments,
        ["simctl", "boot", "12345678-1234-1234-1234-123456789abc"]
    );
    let plan = p
        .launch_plan(Path::new("/tmp"), "com.example.my-app")
        .unwrap();
    assert_eq!(plan.arguments[1], "launch");
    assert_eq!(plan.arguments[3], "com.example.my-app");
    let f = Fixture::new();
    fs::write(f.0.join("app.ipa"), b"fixture").unwrap();
    assert!(p.install_plan(&f.0, &f.0.join("app.ipa")).is_err());
    fs::create_dir(f.0.join("Simulator app.app")).unwrap();
    assert_eq!(
        p.install_plan(&f.0, &f.0.join("Simulator app.app"))
            .unwrap()
            .arguments[1],
        "install"
    );
}
#[test]
fn application_commands_reject_shell_metacharacters_and_preserve_literal_apk_path() {
    let p = android();
    for app in [
        "com.app/.Main;reboot",
        "com.app/$(id)",
        "-all",
        "com.app/",
        "com.app",
    ] {
        assert!(p.launch_plan(Path::new("/tmp"), app).is_err());
    }
    let f = Fixture::new();
    let apk = f.0.join("a & b.apk");
    fs::write(&apk, b"fixture").unwrap();
    let args = p.install_plan(&f.0, &apk).unwrap().arguments;
    assert_eq!(args.last().unwrap(), apk.to_str().unwrap());
}
fn fake_adb(f: &Fixture, script: &str) -> Profile {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(f.0.join("sdk with spaces/platform-tools")).unwrap();
    let binary = f.0.join("sdk with spaces/platform-tools/adb");
    fs::write(&binary, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(binary, fs::Permissions::from_mode(0o700)).unwrap();
    android()
}
#[test]
fn wrong_avd_target_prevents_mutating_commands() {
    let f = Fixture::new();
    let p = fake_adb(
        &f,
        "if [ \"$3\" = emu ]; then echo Another_AVD; exit 0; fi\necho MUTATION_SHOULD_NOT_RUN",
    );
    let error = p.launch(&f.0, "com.app/.Main").unwrap_err().to_string();
    assert!(error.contains("different AVD"));
    assert!(!error.contains("MUTATION_SHOULD_NOT_RUN"));
}
#[test]
fn android_activity_failure_is_not_success_even_when_tool_exits_zero() {
    let f = Fixture::new();
    let p=fake_adb(&f,"if [ \"$3\" = emu ]; then echo FreeLane_Test; echo OK; exit 0; fi\necho 'Error: Activity class does not exist'\nexit 0");
    assert!(p
        .launch(&f.0, "com.app/.Missing")
        .unwrap_err()
        .to_string()
        .contains("did not report Status: ok"));
}
#[cfg(not(target_os = "macos"))]
#[test]
fn ios_runtime_is_explicitly_rejected_on_linux_instead_of_faked() {
    assert!(ios()
        .status(Path::new("/tmp"))
        .unwrap_err()
        .to_string()
        .contains("requires macOS"));
}
