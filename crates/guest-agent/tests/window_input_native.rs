#![cfg(windows)]
//! USER32 owned-window fixture; never targets user applications or global mouse input.
use freelane_agent_protocol::{
    window_input::{Button, Gesture, Position},
    Action, Outcome, WindowTarget,
};
use freelane_guest_agent::{Launcher, WindowsLauncher};
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
#[derive(Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
#[derive(Default)]
struct Message {
    window: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: Point,
    private: u32,
}
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
    fn DestroyWindow(window: isize) -> i32;
    fn PeekMessageW(message: *mut Message, window: isize, min: u32, max: u32, remove: u32) -> i32;
    fn GetClientRect(window: isize, rect: *mut Rect) -> i32;
    fn GetForegroundWindow() -> isize;
    fn RemovePropW(window: isize, name: *const u16) -> isize;
}
struct Owned(isize);
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.0);
        }
    }
}
fn wide(v: &str) -> Vec<u16> {
    v.encode_utf16().chain(Some(0)).collect()
}
fn window() -> Owned {
    let handle = unsafe {
        CreateWindowExW(
            0,
            wide("STATIC").as_ptr(),
            wide("FreeLane owned input fixture").as_ptr(),
            0x90cf0000,
            40,
            40,
            300,
            200,
            0,
            0,
            0,
            std::ptr::null_mut(),
        )
    };
    assert_ne!(handle, 0);
    Owned(handle)
}
fn target(launcher: &mut WindowsLauncher, window: isize) -> WindowTarget {
    let Outcome::Windows { windows } = launcher
        .window_action(&Action::WindowInventory {
            pid: std::process::id(),
        })
        .unwrap()
    else {
        panic!("inventory")
    };
    windows
        .into_iter()
        .find(|w| w.target.handle == window as u64)
        .unwrap()
        .target
}
fn messages(window: isize) -> Vec<Message> {
    let mut result = vec![];
    loop {
        let mut message = Message::default();
        if unsafe { PeekMessageW(&mut message, window, 0x200, 0x20e, 1) } == 0 {
            break;
        }
        result.push(message);
        assert!(result.len() < 100);
    }
    result
}
#[test]
fn real_user32_pointer_resize_and_stale_shortcut_refusal() {
    let a = window();
    let b = window();
    let mut launcher = WindowsLauncher;
    let target_a = target(&mut launcher, a.0);
    messages(a.0);
    let at = Position { x: 10000, y: 12000 };
    for count in [1, 2] {
        assert_eq!(
            launcher
                .window_action(&Action::WindowPointer {
                    target: target_a.clone(),
                    gesture: Gesture::Click {
                        at,
                        button: Button::Primary,
                        count
                    }
                })
                .unwrap(),
            Outcome::WindowInputAccepted
        );
    }
    let observed: Vec<_> = messages(a.0).into_iter().map(|m| m.message).collect();
    assert_eq!(
        observed,
        vec![0x201, 0x202, 0x203, 0x202],
        "A double-click must not emit a third click"
    );
    launcher
        .window_action(&Action::WindowPointer {
            target: target_a.clone(),
            gesture: Gesture::Click {
                at,
                button: Button::Secondary,
                count: 1,
            },
        })
        .unwrap();
    assert_eq!(
        messages(a.0).iter().map(|m| m.message).collect::<Vec<_>>(),
        vec![0x204, 0x205]
    );
    launcher
        .window_action(&Action::WindowPointer {
            target: target_a.clone(),
            gesture: Gesture::Wheel {
                at,
                horizontal: false,
                delta: -120,
            },
        })
        .unwrap();
    let wheel = messages(a.0);
    assert_eq!(wheel.len(), 1);
    assert_eq!(wheel[0].message, 0x20a);
    assert_eq!((wheel[0].wparam >> 16) as u16 as i16, -120);
    launcher
        .window_action(&Action::WindowPointer {
            target: target_a.clone(),
            gesture: Gesture::Drag {
                from: at,
                to: Position { x: 24000, y: 25000 },
                button: Button::Primary,
            },
        })
        .unwrap();
    let drag = messages(a.0);
    assert_eq!(drag.len(), 18);
    assert_eq!(drag[0].message, 0x201);
    assert!(drag[1..17]
        .iter()
        .all(|m| m.message == 0x200 && m.wparam == 1));
    assert_eq!(drag[17].message, 0x202);
    launcher
        .window_action(&Action::WindowResize {
            target: target_a.clone(),
            width: 400,
            height: 240,
        })
        .unwrap();
    let mut rect = Rect::default();
    assert_ne!(unsafe { GetClientRect(a.0, &mut rect) }, 0);
    assert_eq!((rect.right, rect.bottom), (400, 240));
    // At least one owned root is not foreground. The shortcut must never activate it.
    let background = if unsafe { GetForegroundWindow() } == a.0 {
        b.0
    } else {
        a.0
    };
    let target_background = target(&mut launcher, background);
    assert!(launcher
        .window_action(&Action::WindowShortcut {
            target: target_background,
            key: 65,
            modifiers: 1
        })
        .unwrap_err()
        .to_string()
        .contains("foreground"));
    unsafe {
        RemovePropW(a.0, wide("FreeLane.Window.Incarnation.v9").as_ptr());
    }
    for action in [
        Action::WindowPointer {
            target: target_a.clone(),
            gesture: Gesture::Click {
                at,
                button: Button::Primary,
                count: 1,
            },
        },
        Action::WindowResize {
            target: target_a.clone(),
            width: 500,
            height: 300,
        },
        Action::WindowShortcut {
            target: target_a,
            key: 65,
            modifiers: 1,
        },
    ] {
        assert!(launcher.window_action(&action).is_err());
    }
    assert!(messages(a.0).is_empty(), "Stale input must not be posted");
}
