//! Targeted pointer messages and bounded, foreground-guarded keyboard chords.
use super::*;
use freelane_agent_protocol::window_input::{Button, Gesture, Position};
#[repr(C)]
#[derive(Clone, Copy)]
struct Keyboard {
    key: u16,
    scan: u16,
    flags: u32,
    time: u32,
    extra: usize,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Mouse {
    x: i32,
    y: i32,
    data: u32,
    flags: u32,
    time: u32,
    extra: usize,
}
#[repr(C)]
union Payload {
    keyboard: Keyboard,
    mouse: Mouse,
}
#[repr(C)]
struct Input {
    kind: u32,
    payload: Payload,
}
#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> Handle;
    fn GetAsyncKeyState(key: i32) -> i16;
    fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
    fn GetWindowRect(window: Handle, rect: *mut Rect) -> i32;
    fn SetWindowPos(
        window: Handle,
        after: Handle,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
}
fn location(root: Handle, p: Position) -> io::Result<(Handle, Point)> {
    let mut rect = Rect::default();
    if unsafe { GetClientRect(root, &mut rect) } == 0 || rect.right <= 0 || rect.bottom <= 0 {
        return Err(invalid("Window has no client area"));
    }
    let mut point = Point {
        x: i32::from(p.x) * (rect.right - 1) / 32767,
        y: i32::from(p.y) * (rect.bottom - 1) / 32767,
    };
    let mut window = root;
    for _ in 0..32 {
        let child = unsafe { ChildWindowFromPointEx(window, point, 7) };
        if child == 0 || child == window {
            break;
        }
        if unsafe { GetAncestor(child, 2) } != root {
            return Err(invalid("Pointer child changed ownership"));
        }
        if unsafe { ClientToScreen(window, &mut point) } == 0
            || unsafe { ScreenToClient(child, &mut point) } == 0
        {
            return Err(invalid("Cannot map pointer position"));
        }
        window = child;
    }
    Ok((window, point))
}
fn packed(point: Point) -> io::Result<isize> {
    if i16::try_from(point.x).is_err() || i16::try_from(point.y).is_err() {
        return Err(invalid(
            "Pointer position exceeds signed message coordinates",
        ));
    }
    Ok((((point.y as u16 as u32) << 16) | point.x as u16 as u32) as isize)
}
fn messages(button: Button) -> (u32, u32, u32, usize) {
    match button {
        Button::Primary => (0x201, 0x202, 0x203, 1),
        Button::Secondary => (0x204, 0x205, 0x206, 2),
    }
}
fn pointer(root: Handle, target: &WindowTarget, gesture: &Gesture) -> io::Result<()> {
    match gesture {
        Gesture::Click { at, button, count } => {
            let (window, point) = location(root, *at)?;
            let position = packed(point)?;
            let (down, up, double, flag) = messages(*button);
            checked(target)?;
            // count=2 is the second observed click, after the first click's
            // down/up was already delivered. Do not synthesize a third click.
            post(
                window,
                if *count == 2 { double } else { down },
                flag,
                position,
            )?;
            post(window, up, 0, position)?;
        }
        Gesture::Wheel {
            at,
            horizontal,
            delta,
        } => {
            let (window, mut point) = location(root, *at)?;
            if unsafe { ClientToScreen(window, &mut point) } == 0 {
                return Err(invalid("Cannot map wheel"));
            }
            let position = packed(point)?;
            checked(target)?;
            post(
                window,
                if *horizontal { 0x20e } else { 0x20a },
                ((*delta as u16 as u32) << 16) as usize,
                position,
            )?;
        }
        Gesture::Drag { from, to, button } => {
            let (window, begin) = location(root, *from)?;
            let (end_window, mut end) = location(root, *to)?;
            if unsafe { ClientToScreen(end_window, &mut end) } == 0
                || unsafe { ScreenToClient(window, &mut end) } == 0
            {
                return Err(invalid("Cannot map drag endpoint"));
            }
            let begin_pos = packed(begin)?;
            let end_pos = packed(end)?;
            let (down, up, _, flag) = messages(*button);
            checked(target)?;
            post(window, down, flag, begin_pos)?;
            let movement: io::Result<()> = (|| {
                for step in 1..=16 {
                    checked(target)?;
                    let point = Point {
                        x: begin.x + (end.x - begin.x) * step / 16,
                        y: begin.y + (end.y - begin.y) * step / 16,
                    };
                    post(window, 0x200, flag, packed(point)?)?;
                }
                Ok(())
            })();
            // A failed move must not leave an injected button down in the target queue.
            let release = post(window, up, 0, end_pos);
            movement?;
            release?;
        }
    }
    Ok(())
}
fn keyboard(key: u16, up: bool) -> Input {
    let scan = unsafe { MapVirtualKeyW(key as u32, 0) };
    let extended = matches!(key, 33..=40 | 45 | 46);
    Input {
        kind: 1,
        payload: Payload {
            keyboard: Keyboard {
                key,
                scan: scan as u16,
                flags: if up { 2 } else { 0 } | u32::from(extended),
                time: 0,
                extra: 0,
            },
        },
    }
}
fn shortcut(root: Handle, target: &WindowTarget, key: u16, modifiers: u8) -> io::Result<()> {
    let mut keys = Vec::with_capacity(4);
    for (bit, vk) in [(1, 0x11), (2, 0x10), (4, 0x12)] {
        if modifiers & bit != 0 {
            keys.push(vk);
        }
    }
    keys.push(key);
    // No AttachThreadInput, foreground stealing, key-state forcing or retry.
    // Refuse if the user is already holding a modifier or the requested key.
    for vk in [0x10, 0x11, 0x12, 0x5b, 0x5c, key] {
        if unsafe { GetAsyncKeyState(vk as i32) } < 0 {
            return Err(invalid("Release guest keys before sending a shortcut"));
        }
    }
    checked(target)?;
    if unsafe { GetForegroundWindow() } != root {
        return Err(invalid(
            "Selected guest window must be foreground for shortcuts",
        ));
    }
    let inputs: Vec<_> = keys
        .iter()
        .map(|k| keyboard(*k, false))
        .chain(keys.iter().rev().map(|k| keyboard(*k, true)))
        .collect();
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<Input>() as i32,
        )
    } as usize;
    if sent != inputs.len() {
        if sent > 0 {
            let release: Vec<_> = keys
                .iter()
                .take(sent.min(keys.len()))
                .rev()
                .map(|k| keyboard(*k, true))
                .collect();
            unsafe {
                SendInput(
                    release.len() as u32,
                    release.as_ptr(),
                    std::mem::size_of::<Input>() as i32,
                );
            }
        }
        return Err(invalid(
            "Shortcut was partially sent or refused; inspect guest before retrying",
        ));
    }
    Ok(())
}
pub(super) fn handle(action: &Action) -> io::Result<Outcome> {
    freelane_agent_protocol::window_input::validate(action)?;
    let target = freelane_agent_protocol::window_input::target(action).unwrap();
    let root = checked(target)?;
    match action {
        Action::WindowPointer { gesture, .. } => pointer(root, target, gesture)?,
        Action::WindowShortcut { key, modifiers, .. } => shortcut(root, target, *key, *modifiers)?,
        Action::WindowResize { width, height, .. } => {
            let mut client = Rect::default();
            let mut outer = Rect::default();
            if unsafe { GetClientRect(root, &mut client) } == 0
                || unsafe { GetWindowRect(root, &mut outer) } == 0
            {
                return Err(io::Error::last_os_error());
            }
            let width =
                i32::from(*width) + (outer.right - outer.left) - (client.right - client.left);
            let height =
                i32::from(*height) + (outer.bottom - outer.top) - (client.bottom - client.top);
            checked(target)?;
            if unsafe {
                SetWindowPos(
                    root,
                    0,
                    0,
                    0,
                    width,
                    height,
                    0x0002 | 0x0004 | 0x0010 | 0x4000,
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
        }
        _ => unreachable!(),
    }
    Ok(Outcome::WindowInputAccepted)
}
