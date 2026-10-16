#![cfg(any(target_os = "linux", windows))]
use std::{
    io::Write,
    process::{Command, Stdio},
};
#[test]
fn missing_outbox_does_not_stop_the_guest_agent_or_create_a_directory() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("unplugged-outbox");
    let mut child = Command::new(env!("CARGO_BIN_EXE_freelane-guest-agent"))
        .arg("--stdio")
        .arg("--export-root")
        .arg(&missing)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let _ = child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"{\"version\":1,\"id\":5,\"action\":{\"type\":\"ping\"}}\n");
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "Agent must stay usable without the missing folder: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reply: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(reply["outcome"]["status"], "pong");
    assert!(!missing.exists());
}
use freelane_agent_protocol::{
    export::Operation as E,
    transfer_folders::{Report, State},
    Action, Outcome, Request, TransferAction as T,
};
use freelane_guest_agent::{handle, settings::Settings, Launcher, TransferLauncher};
struct Fake;
impl Launcher for Fake {
    fn launch(&mut self, _: &str, _: &[String]) -> std::io::Result<u32> {
        Ok(42)
    }
}
fn call(agent: &mut impl Launcher, version: u16, action: Action) -> Outcome {
    let r = handle(
        Request {
            version,
            id: 77,
            action,
        },
        agent,
    );
    assert_eq!(r.version, version);
    r.outcome
}
fn folders(agent: &mut impl Launcher, reconnect: bool) -> Report {
    match call(agent, 13, Action::TransferFolders { reconnect }) {
        Outcome::TransferFolders { report } => report,
        x => panic!("{x:?}"),
    }
}
#[test]
fn directions_are_independent_and_reopen_only_on_explicit_request() {
    let root = tempfile::tempdir().unwrap();
    let inbox = root.path().join("inbox");
    std::fs::create_dir(&inbox).unwrap();
    let outbox = root.path().join("missing");
    let mut agent = TransferLauncher::configured(
        Fake,
        &Settings {
            inbox: Some(inbox),
            outbox: Some(outbox.clone()),
        },
    );
    assert_eq!(
        folders(&mut agent, false),
        Report {
            inbox: State::Ready,
            outbox: State::Unavailable,
            busy: false
        }
    );
    assert!(!outbox.exists());
    assert!(matches!(
        call(
            &mut agent,
            1,
            Action::Launch {
                executable: "fixture".into(),
                arguments: vec![]
            }
        ),
        Outcome::Launched { pid: 42 }
    ));
    std::fs::create_dir(&outbox).unwrap();
    assert_eq!(folders(&mut agent, false).outbox, State::Unavailable);
    assert_eq!(folders(&mut agent, true).outbox, State::Ready);
    let mut disabled = TransferLauncher::configured(Fake, &Settings::default());
    assert_eq!(
        folders(&mut disabled, true),
        Report {
            inbox: State::Disabled,
            outbox: State::Disabled,
            busy: false
        }
    );
}
#[test]
fn reconnect_refuses_active_upload_without_losing_bytes_or_opening_other_roots() {
    let root = tempfile::tempdir().unwrap();
    let outbox = root.path().join("missing");
    let mut agent = TransferLauncher::configured(
        Fake,
        &Settings {
            inbox: Some(root.path().into()),
            outbox: Some(outbox.clone()),
        },
    );
    let token = match call(
        &mut agent,
        5,
        Action::Transfer {
            operation: T::Begin {
                name: "result.txt".into(),
                size: 3,
                sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
            },
        },
    ) {
        Outcome::TransferReady { token } => token,
        x => panic!("{x:?}"),
    };
    std::fs::create_dir(&outbox).unwrap();
    assert!(folders(&mut agent, false).busy);
    assert!(matches!(
        call(&mut agent, 13, Action::TransferFolders { reconnect: true }),
        Outcome::Failed { .. }
    ));
    assert_eq!(folders(&mut agent, false).outbox, State::Unavailable);
    assert_eq!(
        call(
            &mut agent,
            5,
            Action::Transfer {
                operation: T::Chunk {
                    token: token.clone(),
                    offset: 0,
                    data_hex: "616263".into()
                }
            }
        ),
        Outcome::TransferProgress { received: 3 }
    );
    assert_eq!(
        call(
            &mut agent,
            5,
            Action::Transfer {
                operation: T::Finish { token }
            }
        ),
        Outcome::TransferComplete
    );
    assert_eq!(
        std::fs::read(root.path().join("result.txt")).unwrap(),
        b"abc"
    );
    assert_eq!(folders(&mut agent, true).outbox, State::Ready);
}
#[test]
fn reconnect_refuses_active_export_and_preserves_its_token() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("result.txt"), b"x").unwrap();
    let inbox = root.path().join("missing");
    let mut agent = TransferLauncher::configured(
        Fake,
        &Settings {
            inbox: Some(inbox.clone()),
            outbox: Some(root.path().into()),
        },
    );
    let token = match call(
        &mut agent,
        11,
        Action::Export {
            operation: E::Begin {
                name: "result.txt".into(),
            },
        },
    ) {
        Outcome::ExportReady { token, .. } => token,
        x => panic!("{x:?}"),
    };
    std::fs::create_dir(&inbox).unwrap();
    assert!(folders(&mut agent, false).busy);
    assert!(matches!(
        call(&mut agent, 13, Action::TransferFolders { reconnect: true }),
        Outcome::Failed { .. }
    ));
    assert!(matches!(
        call(
            &mut agent,
            11,
            Action::Export {
                operation: E::Read {
                    token: token.clone(),
                    offset: 0
                }
            }
        ),
        Outcome::ExportChunk { .. }
    ));
    assert!(matches!(
        call(
            &mut agent,
            11,
            Action::Export {
                operation: E::Finish { token }
            }
        ),
        Outcome::ExportFinished { .. }
    ));
    assert_eq!(folders(&mut agent, true).inbox, State::Ready);
}
#[cfg(unix)]
#[test]
fn reconnect_keeps_opened_directory_handles_when_paths_are_replaced() {
    let parent = tempfile::tempdir().unwrap();
    let inbox = parent.path().join("inbox");
    let outbox = parent.path().join("outbox");
    for path in [&inbox, &outbox] {
        std::fs::create_dir(path).unwrap();
    }
    std::fs::write(outbox.join("original.txt"), b"a").unwrap();
    let mut agent = TransferLauncher::configured(
        Fake,
        &Settings {
            inbox: Some(inbox.clone()),
            outbox: Some(outbox.clone()),
        },
    );
    for (path, name) in [(&inbox, "original-inbox"), (&outbox, "original-outbox")] {
        std::fs::rename(path, parent.path().join(name)).unwrap();
        std::fs::create_dir(path).unwrap();
    }
    std::fs::write(outbox.join("replacement.txt"), b"b").unwrap();
    assert_eq!(folders(&mut agent, true).outbox, State::Ready);
    match call(&mut agent, 12, Action::ExportCatalog) {
        Outcome::ExportCatalog { page } => assert_eq!(page.files[0].name, "original.txt"),
        x => panic!("{x:?}"),
    }
}
