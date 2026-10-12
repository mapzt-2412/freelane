use freelane::{qemu, Config};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
#[test]
fn tcg_threading_is_explicit_and_preserves_guest_resources() {
    let images = Images::new();
    let mut config = images.config();
    for (mode, expected) in [
        (None, "tcg"),
        (Some(qemu::TcgThreading::Single), "tcg,thread=single"),
        (Some(qemu::TcgThreading::Multi), "tcg,thread=multi"),
    ] {
        config.runtime.as_mut().unwrap().tcg_threading = mode;
        let roundtrip = Config::parse(&toml::to_string(&config).unwrap()).unwrap();
        assert_eq!(roundtrip.runtime.as_ref().unwrap().tcg_threading, mode);
        let args = qemu::arguments(&roundtrip, &images.0, &images.0.join("qmp.sock")).unwrap();
        for (flag, value) in [
            ("-accel", expected),
            ("-cpu", "max"),
            ("-smp", "4"),
            ("-m", "4096"),
        ] {
            assert!(args
                .windows(2)
                .any(|pair| pair[0] == flag && pair[1] == value));
        }
    }
    config.runtime.as_mut().unwrap().accelerator = qemu::Accelerator::Hvf;
    assert!(config
        .validate()
        .unwrap_err()
        .to_string()
        .contains("tcg_threading"));
}
struct Images(PathBuf);
impl Images {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "fl-plan-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).unwrap();
        for filename in [
            "disk,with spaces.qcow2",
            "firmware.fd",
            "nvram.fd",
            "installer.iso",
        ] {
            fs::write(base.join(filename), b"fixture, not bootable").unwrap();
        }
        Self(base)
    }
    fn config(&self) -> Config {
        Config::parse("name='Windows'\ncpus=4\nmemory_mib=4096\ndisk='disk,with spaces.qcow2'\ninstaller='installer.iso'\n[runtime]\nfirmware='firmware.fd'\nnvram='nvram.fd'\naccelerator='tcg'\nheadless=true\ndisk_format='qcow2'").unwrap()
    }
}
impl Drop for Images {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn disk_paths_are_json_values_and_format_is_explicit() {
    let images = Images::new();
    let config = images.config();
    let args = qemu::arguments(&config, &images.0, &images.0.join("qmp.sock")).unwrap();
    let disk: serde_json::Value = args
        .windows(2)
        .filter(|pair| pair[0] == "-blockdev")
        .map(|pair| serde_json::from_str::<serde_json::Value>(&pair[1]).unwrap())
        .find(|v| v["node-name"] == "disk")
        .unwrap();
    assert_eq!(disk["driver"], "qcow2");
    assert_eq!(
        disk["file"]["filename"],
        images.0.join("disk,with spaces.qcow2").to_str().unwrap()
    );
    assert!(args.iter().any(|a| a == "none"));
    assert!(!args.iter().any(|a| a.contains("hostfwd")));
    assert!(
        !args.iter().any(|a| a == "-no-reboot"),
        "Windows installer must be allowed to reboot"
    );
}
#[test]
fn usb_network_is_opt_in_without_host_port_forwarding() {
    let images = Images::new();
    let mut config = images.config();
    let runtime = config.runtime.as_mut().unwrap();
    runtime.network_device = qemu::NetworkDevice::Usb;
    let disabled = qemu::arguments(&config, &images.0, &images.0.join("qmp.sock")).unwrap();
    assert!(!disabled.iter().any(|arg| arg.starts_with("usb-net")));
    config.runtime.as_mut().unwrap().network = true;
    let enabled = qemu::arguments(&config, &images.0, &images.0.join("qmp.sock")).unwrap();
    assert!(enabled.iter().any(|arg| arg == "usb-net,netdev=net0"));
    assert!(!enabled.iter().any(|arg| arg.contains("hostfwd")));
}
#[test]
fn writable_images_cannot_alias_firmware_even_through_hardlinks() {
    let images = Images::new();
    let config = images.config();
    fs::remove_file(images.0.join("nvram.fd")).unwrap();
    fs::hard_link(images.0.join("firmware.fd"), images.0.join("nvram.fd")).unwrap();
    assert!(
        qemu::arguments(&config, &images.0, &images.0.join("qmp.sock"))
            .unwrap_err()
            .to_string()
            .contains("alias")
    );
}
#[test]
fn installer_cannot_alias_guest_disk() {
    let images = Images::new();
    let config = images.config();
    fs::remove_file(images.0.join("installer.iso")).unwrap();
    fs::hard_link(
        images.0.join("disk,with spaces.qcow2"),
        images.0.join("installer.iso"),
    )
    .unwrap();
    assert!(qemu::arguments(&config, &images.0, &images.0.join("qmp.sock")).is_err());
}
#[test]
fn hvf_never_falls_back_silently_to_software_emulation() {
    if !freelane::supported_host() {
        let images = Images::new();
        let mut config = images.config();
        config.runtime.as_mut().unwrap().accelerator = qemu::Accelerator::Hvf;
        assert!(
            qemu::arguments(&config, &images.0, &images.0.join("qmp.sock"))
                .unwrap_err()
                .to_string()
                .contains("Apple Silicon")
        );
    }
}
#[test]
fn initialization_cannot_overwrite_existing_user_directory() {
    let images = Images::new();
    let bytes = fs::read(images.0.join("nvram.fd")).unwrap();
    assert!(qemu::initialize(
        &images.0,
        &images.0.join("firmware.fd"),
        &images.0.join("nvram.fd"),
        std::path::Path::new("missing-qemu-img")
    )
    .is_err());
    assert_eq!(fs::read(images.0.join("nvram.fd")).unwrap(), bytes);
}
#[test]
fn names_cannot_inject_qemu_suboptions() {
    let images = Images::new();
    let mut config = images.config();
    for name in ["Windows,debug-threads=on", "Windows\nVM", "Windows\0VM"] {
        config.name = name.into();
        assert!(config.validate().is_err());
    }
}

#[test]
fn arm_virtio_devices_disable_legacy_port_io() {
    let images = Images::new();
    let mut config = images.config();
    let runtime = config.runtime.as_mut().unwrap();
    runtime.network = true;
    runtime.guest_agent = true;
    let args = qemu::arguments(&config, &images.0, &images.0.join("qmp.sock")).unwrap();
    let devices: Vec<_> = args
        .windows(2)
        .filter(|pair| pair[0] == "-device")
        .map(|pair| pair[1].as_str())
        .filter(|device| device.starts_with("virtio-"))
        .collect();
    assert_eq!(devices.len(), 2);
    assert!(
        devices.iter().all(|device| device
            .split(',')
            .any(|option| option == "disable-legacy=on")),
        "Windows ARM needs memory-mapped modern virtio, not legacy port I/O"
    );
}
