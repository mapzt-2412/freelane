//! Static ARM64 initramfs fixture: real Linux 9p reads/writes as UID 1000.
use std::{
    ffi::{c_char, c_void, CString},
    fs,
    io::{self, Write},
    os::unix::process::CommandExt,
    process::Command,
    time::{Duration, Instant},
};
unsafe extern "C" {
    fn mount(
        source: *const c_char,
        target: *const c_char,
        kind: *const c_char,
        flags: usize,
        data: *const c_void,
    ) -> i32;
    fn init_module(module: *const c_void, length: usize, args: *const c_char) -> i32;
    fn getuid() -> u32;
    fn sync();
    fn reboot(command: i32) -> i32;
}
fn worker() {
    // SAFETY: getuid has no pointers and reads the actual kernel credential.
    assert_eq!(unsafe { getuid() }, 1000);
    assert_eq!(fs::read("/share/host.txt").unwrap(), b"host-v1\n");
    let mut file = fs::File::create("/share/guest.txt").unwrap();
    file.write_all(b"guest-v1\n").unwrap();
    file.sync_all().unwrap();
    drop(file);
    println!("FREELANE_9P_GUEST_WRITE_OK");
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if fs::read("/share/host.txt").unwrap() == b"host-v2\n" {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Host change not visible in guest"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    fs::rename("/share/guest.txt", "/share/guest-renamed.txt").unwrap();
    fs::write("/share/deleted.txt", b"temporary").unwrap();
    fs::remove_file("/share/deleted.txt").unwrap();
    println!("FREELANE_9P_BIDIRECTIONAL_OK uid=1000");
}
fn main() {
    if std::env::args().any(|arg| arg == "--worker") {
        worker();
        return;
    }
    for name in ["netfs", "9pnet", "9pnet_virtio", "9p"] {
        let bytes = fs::read(format!("/modules/{name}.ko")).unwrap();
        // SAFETY: each module buffer is valid through the call; args is NUL-terminated.
        let result = unsafe { init_module(bytes.as_ptr().cast(), bytes.len(), c"".as_ptr()) };
        assert_eq!(result, 0, "load {name}: {}", io::Error::last_os_error());
    }
    fs::create_dir_all("/share").unwrap();
    let data = CString::new("trans=virtio,version=9p2000.L,cache=none").unwrap();
    // SAFETY: all strings are NUL-terminated and live through the mount syscall.
    assert_eq!(
        unsafe {
            mount(
                c"freelane".as_ptr(),
                c"/share".as_ptr(),
                c"9p".as_ptr(),
                0,
                data.as_ptr().cast(),
            )
        },
        0,
        "mount: {}",
        io::Error::last_os_error()
    );
    if fs::metadata("/readonly").is_ok() {
        // Mount requested rw above. Even guest root must be denied by the host backend.
        assert_eq!(fs::read("/share/host.txt").unwrap(), b"host-v1\n");
        for result in [
            fs::write("/share/guest.txt", b"forbidden"),
            fs::write("/share/host.txt", b"forbidden"),
            fs::rename("/share/host.txt", "/share/renamed.txt"),
            fs::remove_file("/share/host.txt"),
        ] {
            assert!(result.is_err(), "backend allowed guest write");
        }
        println!("FREELANE_9P_READONLY_READY");
        let deadline = Instant::now() + Duration::from_secs(60);
        while fs::read("/share/host.txt").unwrap() != b"host-v2\n" {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(100));
        }
        println!("FREELANE_9P_READONLY_OK uid=0");
        unsafe {
            sync();
            reboot(0x4321fedcu32 as i32);
        }
        panic!("Guest poweroff returned");
    }
    let status = Command::new("/init")
        .arg("--worker")
        .uid(1000)
        .gid(1000)
        .status()
        .unwrap();
    assert!(status.success(), "Unprivileged worker failed: {status}");
    // SAFETY: PID 1 executes the native Linux sync and poweroff after successful work.
    unsafe {
        sync();
        reboot(0x4321fedcu32 as i32);
    }
    panic!("Guest poweroff returned");
}
