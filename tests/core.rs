use freelane::{Backend, Config, Error, GuestOs, Result, State, Vm};
#[test]
fn guest_os_preserves_legacy_windows_and_accepts_only_supported_profiles() {
    let legacy = "name='VM'\ncpus=4\nmemory_mib=4096\ndisk='disk.qcow2'";
    assert_eq!(Config::parse(legacy).unwrap().guest_os, GuestOs::Windows);
    let ubuntu = Config::parse(&format!("{legacy}\nguest_os='ubuntu'")).unwrap();
    assert_eq!(ubuntu.guest_os, GuestOs::Ubuntu);
    assert_eq!(
        Config::parse(&toml::to_string(&ubuntu).unwrap())
            .unwrap()
            .guest_os,
        GuestOs::Ubuntu
    );
    assert!(Config::parse(&format!("{legacy}\nguest_os='other'")).is_err());
}
fn config() -> Config {
    Config::parse("name='Windows'\ncpus=4\nmemory_mib=4096\ndisk='windows.vhdx'").unwrap()
}
#[derive(Default)]
struct Fake {
    fail_start: bool,
}
impl Backend for Fake {
    fn prepare(&mut self, _: &Config) -> Result<()> {
        Ok(())
    }
    fn start(&mut self) -> Result<()> {
        if self.fail_start {
            Err(Error("backend failure".into()))
        } else {
            Ok(())
        }
    }
    fn pause(&mut self) -> Result<()> {
        Ok(())
    }
    fn resume(&mut self) -> Result<()> {
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        Ok(())
    }
}
#[test]
fn lifecycle_enforces_order() {
    let mut vm = Vm::new(Fake::default());
    assert!(vm.start().is_err());
    assert!(vm.stop().is_err());
    vm.prepare(&config()).unwrap();
    assert!(vm.pause().is_err());
    vm.start().unwrap();
    assert!(vm.start().is_err());
    vm.pause().unwrap();
    assert_eq!(vm.state(), State::Paused);
    vm.resume().unwrap();
    vm.stop().unwrap();
    assert_eq!(vm.state(), State::Stopped);
    assert!(vm.resume().is_err());
}
#[test]
fn backend_failure_preserves_state() {
    let mut vm = Vm::new(Fake { fail_start: true });
    vm.prepare(&config()).unwrap();
    assert!(vm.start().is_err());
    assert_eq!(vm.state(), State::Ready);
    vm.stop().unwrap();
}
#[test]
fn rejects_invalid_config_and_unknown_fields() {
    for invalid in [
        "cpus=0",
        "cpus=65",
        "memory_mib=1024",
        "name=''",
        "disk=''",
        "installer=''",
        "unexpected=true",
    ] {
        let mut c = config();
        match invalid {
            "cpus=0" => c.cpus = 0,
            "cpus=65" => c.cpus = 65,
            "memory_mib=1024" => c.memory_mib = 1024,
            "name=''" => c.name.clear(),
            "disk=''" => c.disk = "".into(),
            "installer=''" => c.installer = Some("".into()),
            _ => {
                assert!(Config::parse(
                    "name='x'\ncpus=2\nmemory_mib=4096\ndisk='x'\nunexpected=true"
                )
                .is_err());
                continue;
            }
        }
        assert!(c.validate().is_err(), "{invalid}");
    }
}
#[test]
fn missing_images_fail_without_creating_files() {
    let c = config();
    let base = std::env::temp_dir().join(format!("freelane-missing-{}", std::process::id()));
    assert!(c.check_images(&base).is_err());
    assert!(!base.exists());
}
#[test]
fn unsupported_native_probe_is_explicit() {
    if !freelane::supported_host() {
        assert!(freelane::probe_hypervisor()
            .unwrap_err()
            .to_string()
            .contains("Apple Silicon"));
    }
}
#[test]
fn cli_reports_unsupported_host() {
    if !freelane::supported_host() {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_freelane"))
            .arg("doctor")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("Apple Silicon"));
    }
}

#[test]
fn cli_resolves_images_relative_to_configuration() {
    let base = std::env::temp_dir().join(format!("freelane-images-{}", std::process::id()));
    std::fs::create_dir(&base).unwrap();
    {
        std::fs::write(base.join("disk.img"), b"not a bootable disk").unwrap();
        let path = base.join("vm.toml");
        std::fs::write(
            &path,
            "name='test'\ncpus=2\nmemory_mib=4096\ndisk='disk.img'",
        )
        .unwrap();
        let invoke = || {
            std::process::Command::new(env!("CARGO_BIN_EXE_freelane"))
                .arg("validate")
                .arg(&path)
                .current_dir("/")
                .output()
                .unwrap()
        };
        assert!(invoke().status.success());
        std::fs::write(base.join("disk.img"), b"").unwrap();
        assert!(!invoke().status.success());
        std::fs::remove_file(base.join("disk.img")).unwrap();
        assert!(!invoke().status.success());
        assert!(!base.join("disk.img").exists());
    }
    std::fs::remove_dir_all(base).unwrap();
}
