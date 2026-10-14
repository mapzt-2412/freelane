#![cfg(windows)]
use freelane_agent_protocol::{Action, Outcome};
use freelane_guest_agent::{Launcher, WindowsLauncher};
#[link(name = "user32")]
extern "system" {
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: isize,
        menu: isize,
        instance: isize,
        param: *mut std::ffi::c_void,
    ) -> isize;
    fn RemovePropW(window: isize, name: *const u16) -> isize;
    fn DestroyWindow(window: isize) -> i32;
}
struct Owned(isize);
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.0);
        }
    }
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
#[test]
fn real_user32_inventory_refuses_removed_incarnation_and_finds_actual_process_image() {
    let class = wide("STATIC");
    let title = wide("FreeLane owned identity fixture");
    let handle = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            0x90000000,
            10,
            10,
            80,
            60,
            0,
            0,
            0,
            std::ptr::null_mut(),
        )
    };
    assert_ne!(handle, 0);
    let _owned = Owned(handle);
    let mut launcher = WindowsLauncher;
    let Outcome::Windows { windows } = launcher
        .window_action(&Action::WindowInventory {
            pid: std::process::id(),
        })
        .unwrap()
    else {
        panic!("Missing inventory")
    };
    let target = windows
        .into_iter()
        .find(|w| w.target.handle == handle as u64)
        .unwrap()
        .target;
    assert_ne!(target.incarnation, 0);
    let name = wide("FreeLane.Window.Incarnation.v9");
    assert_eq!(
        unsafe { RemovePropW(handle, name.as_ptr()) } as u64,
        target.incarnation
    );
    assert!(
        launcher
            .window_action(&Action::WindowClick {
                target: target.clone(),
                x: 1,
                y: 1
            })
            .is_err(),
        "Stale target must be refused before input"
    );
    let Outcome::Windows { windows } = launcher
        .window_action(&Action::WindowInventory {
            pid: std::process::id(),
        })
        .unwrap()
    else {
        panic!("Missing inventory")
    };
    let refreshed = windows
        .into_iter()
        .find(|w| w.target.handle == handle as u64)
        .unwrap()
        .target;
    assert_ne!(refreshed.incarnation, target.incarnation);
    let executable = std::env::current_exe()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let Outcome::Windows { windows } = launcher
        .window_action(&Action::FindAppWindows { executable })
        .unwrap()
    else {
        panic!("Missing process lookup")
    };
    assert!(windows.iter().any(|w| w.target == refreshed));
    let Outcome::Windows { windows } = launcher
        .window_action(&Action::Windows {
            pid: std::process::id(),
        })
        .unwrap()
    else {
        panic!("Missing legacy inventory")
    };
    assert!(windows.iter().all(|w| w.target.incarnation == 0));
}
