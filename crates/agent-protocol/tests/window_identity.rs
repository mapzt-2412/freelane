use freelane_agent_protocol::{Action, Request, WindowTarget, WINDOW_IDENTITY_VERSION};
#[test]
fn upgraded_window_requests_require_incarnation_and_keep_legacy_wire_compatible() {
    let mut target = WindowTarget {
        handle: 1,
        pid: 2,
        created: 3,
        incarnation: 0,
    };
    let legacy = serde_json::to_value(&target).unwrap();
    assert!(legacy.get("incarnation").is_none());
    assert!(Request {
        version: WINDOW_IDENTITY_VERSION,
        id: 1,
        action: Action::CaptureWindow {
            target: target.clone()
        }
    }
    .validate()
    .is_err());
    target.incarnation = 4;
    Request {
        version: WINDOW_IDENTITY_VERSION,
        id: 1,
        action: Action::CaptureWindow { target },
    }
    .validate()
    .unwrap();
    Request {
        version: WINDOW_IDENTITY_VERSION,
        id: 1,
        action: Action::WindowInventory { pid: 2 },
    }
    .validate()
    .unwrap();
    for executable in [
        "notepad.exe",
        r"\\server\app.exe",
        "C:relative.exe",
        "C:\\bad\n.exe",
    ] {
        assert!(Request {
            version: WINDOW_IDENTITY_VERSION,
            id: 1,
            action: Action::FindAppWindows {
                executable: executable.into()
            }
        }
        .validate()
        .is_err());
    }
    Request {
        version: WINDOW_IDENTITY_VERSION,
        id: 1,
        action: Action::FindAppWindows {
            executable: r"C:\Windows\System32\notepad.exe".into(),
        },
    }
    .validate()
    .unwrap();
    assert!(Request {
        version: WINDOW_IDENTITY_VERSION,
        id: 1,
        action: Action::Launch {
            executable: "/bin/true".into(),
            arguments: vec![]
        }
    }
    .validate()
    .is_err());
}
