#[cfg(target_os = "linux")]
#[test]
#[ignore = "Requires a dedicated authenticated X11 display; modifies only that display's clipboard"]
fn native_user_clipboard_roundtrip_and_replay_refusal() {
    use freelane_agent_protocol::{
        read_frame, write_frame, Action, ClipboardSnapshot, Outcome, Request, Response,
    };
    use std::{
        io::BufReader,
        process::{Command, Stdio},
    };
    struct Agent(std::process::Child);
    impl Drop for Agent {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    assert!(
        std::env::var_os("FREELANE_CLIPBOARD_TEST_DISPLAY").is_some(),
        "use a dedicated private test display"
    );
    let mut native = arboard::Clipboard::new().unwrap();
    let outward = "Xin chào anh 👋 — từ máy Mac";
    let inward = "Từ máy ảo về máy chủ 🎉\nDòng thứ hai";
    native.set_text(outward).unwrap();
    let mut agent = Agent(
        Command::new(env!("CARGO_BIN_EXE_freelane-guest-agent"))
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut reader = BufReader::new(agent.0.stdout.take().unwrap());
    let writer = agent.0.stdin.as_mut().unwrap();
    let mut id = 0;
    let mut call = |version, action| {
        id += 1;
        write_frame(
            writer,
            &Request {
                version,
                id,
                action,
            },
        )
        .unwrap();
        let response: Response = read_frame(&mut reader).unwrap();
        assert_eq!(response.id, id);
        response.outcome
    };
    let Outcome::AgentInfo { info } = call(3, Action::Ping) else {
        panic!("missing discovery")
    };
    assert!(info
        .features
        .iter()
        .any(|f| f.name == "text_clipboard" && f.protocol == 4));
    let Outcome::Clipboard { snapshot } = call(4, Action::ClipboardGet) else {
        panic!("missing native clipboard")
    };
    assert_eq!(snapshot.text, outward);
    let source = "a".repeat(32);
    assert!(matches!(
        call(
            4,
            Action::ClipboardSet {
                source: source.clone(),
                sequence: 1,
                text: inward.into()
            }
        ),
        Outcome::ClipboardAccepted { .. }
    ));
    assert_eq!(native.get_text().unwrap(), inward);
    let Outcome::Clipboard { snapshot } = call(4, Action::ClipboardGet) else {
        panic!("missing readback")
    };
    assert_eq!(
        snapshot,
        ClipboardSnapshot {
            source: source.clone(),
            sequence: 1,
            text: inward.into()
        }
    );
    assert!(matches!(
        call(
            4,
            Action::ClipboardSet {
                source: source.clone(),
                sequence: 1,
                text: "must not overwrite".into()
            }
        ),
        Outcome::Failed { .. }
    ));
    assert_eq!(native.get_text().unwrap(), inward);
    assert!(matches!(
        call(
            1,
            Action::ClipboardSet {
                source,
                sequence: 2,
                text: "legacy mutation refused".into()
            }
        ),
        Outcome::Failed { .. }
    ));
    assert_eq!(native.get_text().unwrap(), inward);
    info.validate().unwrap();
    assert!(info
        .features
        .iter()
        .any(|f| f.name == "clipboard_sync" && f.protocol == 6));
    let Outcome::ClipboardSync {
        instance,
        snapshot: expected,
    } = call(6, Action::ClipboardSyncGet)
    else {
        panic!("missing conditional clipboard")
    };
    assert_eq!(instance.len(), 32);
    native.set_text("concurrent guest fixture").unwrap();
    let update = ClipboardSnapshot {
        source: "a".repeat(32),
        sequence: 2,
        text: "Xin chào tự đồng bộ 👋".into(),
    };
    assert_eq!(
        call(
            6,
            Action::ClipboardSyncSet {
                expected,
                update: update.clone()
            }
        ),
        Outcome::ClipboardConflict
    );
    assert_eq!(native.get_text().unwrap(), "concurrent guest fixture");
    let Outcome::ClipboardSync {
        instance: again,
        snapshot: expected,
    } = call(6, Action::ClipboardSyncGet)
    else {
        panic!("missing sync read")
    };
    assert_eq!(instance, again);
    assert!(matches!(
        call(
            6,
            Action::ClipboardSyncSet {
                expected,
                update: update.clone()
            }
        ),
        Outcome::ClipboardAccepted { .. }
    ));
    assert_eq!(native.get_text().unwrap(), update.text);
    native.set_text("x".repeat(8193)).unwrap();
    assert!(matches!(
        call(4, Action::ClipboardGet),
        Outcome::Failed { .. }
    ));
}
