//! Translate focused viewer keys without duplicating text or inventing legacy support.
use eframe::egui;
use freelane_agent_protocol::{Action, WindowTarget};
pub fn key_action(event: &egui::Event, target: &WindowTarget, extended: bool) -> Option<Action> {
    let shortcut = |key, modifiers| {
        (extended && target.incarnation > 0).then(|| Action::WindowShortcut {
            target: target.clone(),
            key,
            modifiers,
        })
    };
    match event {
        egui::Event::Copy => shortcut(67, 1),
        egui::Event::Cut => shortcut(88, 1),
        egui::Event::Paste(_) => shortcut(86, 1),
        egui::Event::Text(text) if text.len() <= 1024 && !text.contains('\0') => {
            Some(Action::WindowText {
                target: target.clone(),
                text: text.clone(),
            })
        }
        egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => {
            let name = key.name();
            let code = match key {
                egui::Key::Backspace => 8,
                egui::Key::Tab => 9,
                egui::Key::Enter => 13,
                egui::Key::Escape => 27,
                egui::Key::Space => 32,
                egui::Key::PageUp => 33,
                egui::Key::PageDown => 34,
                egui::Key::End => 35,
                egui::Key::Home => 36,
                egui::Key::ArrowLeft => 37,
                egui::Key::ArrowUp => 38,
                egui::Key::ArrowRight => 39,
                egui::Key::ArrowDown => 40,
                egui::Key::Insert => 45,
                egui::Key::Delete => 46,
                _ if name.len() == 1 && name.as_bytes()[0].is_ascii_alphanumeric() => {
                    name.as_bytes()[0].to_ascii_uppercase() as u16
                }
                _ if name.starts_with('F') => name[1..]
                    .parse::<u16>()
                    .ok()
                    .filter(|n| (1..=24).contains(n))
                    .map(|n| 111 + n)
                    .unwrap_or(0),
                _ => 0,
            };
            if code == 0 {
                return None;
            }
            let printable = matches!(code, 32 | 48..=57 | 65..=90);
            if printable
                && !modifiers.ctrl
                && !modifiers.command
                && (!modifiers.alt || cfg!(target_os = "macos"))
            {
                return None; // Shift/Option text and dead keys belong to Text/IME.
            }
            let bits = u8::from(modifiers.ctrl || modifiers.command)
                | (u8::from(modifiers.shift) << 1)
                | (u8::from(modifiers.alt) << 2);
            if bits != 0 {
                shortcut(code, bits)
            } else if [8, 9, 13, 27, 33, 34, 35, 36, 37, 38, 39, 40, 46].contains(&code) {
                Some(Action::WindowKey {
                    target: target.clone(),
                    key: code,
                })
            } else {
                None
            } // Printable keys arrive as Text, never send them twice.
        }
        _ => None,
    }
}
pub fn key_actions(events: &[egui::Event], target: &WindowTarget, extended: bool) -> Vec<Action> {
    let text_input = events.iter().any(|e| matches!(e, egui::Event::Text(_)));
    events.iter().filter(|event| {
        !matches!(event,egui::Event::Key{key,modifiers,..} if text_input && modifiers.alt && !modifiers.ctrl && !modifiers.command && (key.name().len()==1 || *key==egui::Key::Space))
    }).filter_map(|event|key_action(event,target,extended)).collect()
}
pub fn wheel_delta(unit: egui::MouseWheelUnit, horizontal: bool, value: f32) -> i16 {
    let factor = match unit {
        egui::MouseWheelUnit::Point => 3.0,
        egui::MouseWheelUnit::Line => 120.0,
        egui::MouseWheelUnit::Page => 1200.0,
    };
    (value * factor * if horizontal { -1.0 } else { 1.0 })
        .round()
        .clamp(-1200.0, 1200.0) as i16
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shifted_printable_key_and_text_sequence_emits_only_one_character() {
        let events = [
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    shift: true,
                    ..Default::default()
                },
            },
            egui::Event::Text("A".into()),
        ];
        let actions: Vec<_> = events
            .iter()
            .filter_map(|e| key_action(e, &target(), true))
            .collect();
        assert_eq!(
            actions,
            vec![Action::WindowText {
                target: target(),
                text: "A".into()
            }]
        );
    }
    #[test]
    fn horizontal_wheel_inverts_content_direction_but_vertical_keeps_it() {
        assert_eq!(wheel_delta(egui::MouseWheelUnit::Line, true, 1.0), -120);
        assert_eq!(wheel_delta(egui::MouseWheelUnit::Line, false, 1.0), 120);
        assert_eq!(wheel_delta(egui::MouseWheelUnit::Line, true, -1.0), 120);
    }
    #[test]
    fn option_text_sequence_preserves_character_without_an_alt_menu_action() {
        let events = [
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    alt: true,
                    ..Default::default()
                },
            },
            egui::Event::Text("å".into()),
        ];
        assert_eq!(
            key_actions(&events, &target(), true),
            vec![Action::WindowText {
                target: target(),
                text: "å".into()
            }]
        );
    }
    #[test]
    fn shift_navigation_keeps_selection_shortcut() {
        let event = egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                shift: true,
                ..Default::default()
            },
        };
        assert_eq!(
            key_actions(&[event], &target(), true),
            vec![Action::WindowShortcut {
                target: target(),
                key: 37,
                modifiers: 2
            }]
        );
    }
    fn target() -> WindowTarget {
        WindowTarget {
            handle: 1,
            pid: 2,
            created: 3,
            incarnation: 4,
        }
    }
    #[test]
    fn mac_command_maps_to_control_and_legacy_does_not_fake_shortcuts() {
        let event = egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                command: true,
                mac_cmd: true,
                ..Default::default()
            },
        };
        assert_eq!(
            key_action(&event, &target(), true),
            Some(Action::WindowShortcut {
                target: target(),
                key: 65,
                modifiers: 1
            })
        );
        assert_eq!(key_action(&event, &target(), false), None);
    }
    #[test]
    fn copy_paste_are_guest_shortcuts_and_never_send_host_clipboard_text() {
        assert_eq!(
            key_action(&egui::Event::Copy, &target(), true),
            Some(Action::WindowShortcut {
                target: target(),
                key: 67,
                modifiers: 1
            })
        );
        assert_eq!(
            key_action(
                &egui::Event::Paste("host private fixture".into()),
                &target(),
                true
            ),
            Some(Action::WindowShortcut {
                target: target(),
                key: 86,
                modifiers: 1
            })
        );
        assert_eq!(
            key_action(&egui::Event::Text("Tiếng Việt".into()), &target(), false),
            Some(Action::WindowText {
                target: target(),
                text: "Tiếng Việt".into()
            })
        );
    }
}
