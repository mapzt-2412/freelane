//! Bounded v10 window input. Coordinates are normalized to the client area.
use crate::{invalid, Action, WindowTarget};
use serde::{Deserialize, Serialize};
use std::io;
pub const VERSION: u16 = 10;
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    Primary,
    Secondary,
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Gesture {
    Click {
        at: Position,
        button: Button,
        /// Observed click ordinal: 1 sends down/up, 2 sends double-click/up.
        count: u8,
    },
    Wheel {
        at: Position,
        horizontal: bool,
        delta: i16,
    },
    Drag {
        from: Position,
        to: Position,
        button: Button,
    },
}
pub fn target(action: &Action) -> Option<&WindowTarget> {
    match action {
        Action::WindowPointer { target, .. }
        | Action::WindowShortcut { target, .. }
        | Action::WindowResize { target, .. } => Some(target),
        _ => None,
    }
}
pub fn validate(action: &Action) -> io::Result<()> {
    let Some(target) = target(action) else {
        return Err(invalid("Protocol v10 only supports extended window input"));
    };
    if target.pid == 0 || target.created == 0 || target.handle == 0 || target.incarnation == 0 {
        return Err(invalid(
            "Extended input requires observed window incarnation",
        ));
    }
    let position = |p: &Position| p.x <= 32767 && p.y <= 32767;
    let valid = match action {
        Action::WindowPointer { gesture, .. } => match gesture {
            Gesture::Click { at, count, .. } => position(at) && (1..=2).contains(count),
            Gesture::Wheel { at, delta, .. } => {
                position(at) && *delta != 0 && (-1200..=1200).contains(delta)
            }
            Gesture::Drag { from, to, .. } => position(from) && position(to),
        },
        Action::WindowShortcut { key, modifiers, .. } => {
            // ctrl=1, shift=2, alt=4. No Windows/system key or unbounded key list.
            (1..=7).contains(modifiers)
                && (matches!(*key, 8 | 9 | 13 | 27 | 32..=40 | 45 | 46 | 48..=57 | 65..=90 | 112..=135))
        }
        Action::WindowResize { width, height, .. } => {
            (64..=8192).contains(width) && (64..=8192).contains(height)
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(invalid("Invalid extended window input bounds"))
    }
}
