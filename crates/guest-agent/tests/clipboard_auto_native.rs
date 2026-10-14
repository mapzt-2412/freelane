#![cfg(target_os = "linux")]
use freelane_agent_protocol::{
    clipboard::{Backend, Machine},
    clipboard_sync::{tick, Decision, SyncState},
    read_frame, write_frame, Action, Outcome, Request, Response,
};
use std::{
    io::{self, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Arc, Mutex},
};
struct Agent {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    id: u64,
}
impl Agent {
    fn new(display: &str, auth: &str) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_freelane-guest-agent"))
            .arg("--stdio")
            .env("DISPLAY", display)
            .env("XAUTHORITY", auth)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
            id: 0,
        }
    }
    fn call(&mut self, version: u16, action: Action) -> io::Result<Outcome> {
        self.id += 1;
        write_frame(
            &mut self.input,
            &Request {
                version,
                id: self.id,
                action,
            },
        )?;
        let response: Response = read_frame(&mut self.output)?;
        if response.id != self.id || response.version != version {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(response.outcome)
    }
    fn put(&mut self, sequence: u64, text: &str) {
        assert!(matches!(
            self.call(
                4,
                Action::ClipboardSet {
                    source: "d".repeat(32),
                    sequence,
                    text: text.into()
                }
            )
            .unwrap(),
            Outcome::ClipboardAccepted { .. }
        ));
    }
    fn text(&mut self) -> String {
        let Outcome::Clipboard { snapshot } = self.call(4, Action::ClipboardGet).unwrap() else {
            panic!("fixture clipboard read failed")
        };
        snapshot.text
    }
}
impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
struct NativeHost {
    agent: Arc<Mutex<Agent>>,
    sequence: u64,
}
impl Backend for NativeHost {
    fn read(&mut self) -> io::Result<String> {
        Ok(self.agent.lock().unwrap().text())
    }
    fn write(&mut self, text: &str) -> io::Result<()> {
        self.sequence += 1;
        let reply = self.agent.lock().unwrap().call(
            4,
            Action::ClipboardSet {
                source: "e".repeat(32),
                sequence: self.sequence,
                text: text.into(),
            },
        )?;
        if matches!(reply, Outcome::ClipboardAccepted { .. }) {
            Ok(())
        } else {
            Err(io::ErrorKind::Other.into())
        }
    }
}
#[test]
#[ignore = "Only two fresh authenticated fixture displays; never run against a personal clipboard"]
fn independent_native_clipboards_sync_unicode_without_echo_and_preserve_conflicts() {
    assert_eq!(
        std::env::var("FREELANE_AUTO_CLIPBOARD_FIXTURE").unwrap(),
        "two-owned-displays"
    );
    let host = Arc::new(Mutex::new(Agent::new(
        &std::env::var("FREELANE_FIXTURE_HOST_DISPLAY").unwrap(),
        &std::env::var("FREELANE_FIXTURE_HOST_AUTH").unwrap(),
    )));
    let mut guest = Agent::new(
        &std::env::var("FREELANE_FIXTURE_GUEST_DISPLAY").unwrap(),
        &std::env::var("FREELANE_FIXTURE_GUEST_AUTH").unwrap(),
    );
    host.lock().unwrap().put(1, "same fixture");
    guest.put(1, "same fixture");
    let mut machine = Machine::new(
        NativeHost {
            agent: host.clone(),
            sequence: 0,
        },
        "a".repeat(32),
    )
    .unwrap();
    let mut state = SyncState::default();
    let poll = |machine: &mut Machine<NativeHost>, state: &mut SyncState, guest: &mut Agent| {
        tick(machine, state, &mut |action| guest.call(6, action), &|| {
            false
        })
        .unwrap()
    };
    assert_eq!(poll(&mut machine, &mut state, &mut guest), Decision::Idle);
    host.lock().unwrap().put(2, "Xin chào tự đồng bộ 👋");
    assert_eq!(poll(&mut machine, &mut state, &mut guest), Decision::Push);
    assert_eq!(guest.text(), "Xin chào tự đồng bộ 👋");
    assert_eq!(poll(&mut machine, &mut state, &mut guest), Decision::Idle);
    guest.put(2, "Từ guest về host 🎉");
    assert_eq!(poll(&mut machine, &mut state, &mut guest), Decision::Pull);
    assert_eq!(host.lock().unwrap().text(), "Từ guest về host 🎉");
    assert_eq!(poll(&mut machine, &mut state, &mut guest), Decision::Idle);
    host.lock().unwrap().put(3, "host conflict fixture");
    guest.put(3, "guest conflict fixture");
    assert_eq!(
        poll(&mut machine, &mut state, &mut guest),
        Decision::Conflict
    );
    assert_eq!(host.lock().unwrap().text(), "host conflict fixture");
    assert_eq!(guest.text(), "guest conflict fixture");
}
