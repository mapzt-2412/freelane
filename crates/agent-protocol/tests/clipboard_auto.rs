use freelane_agent_protocol::{
    clipboard::{Backend, Machine},
    clipboard_sync::{Decision, SyncState},
    ClipboardSnapshot as S,
};
use std::{
    io,
    sync::{Arc, Mutex},
};
fn snap(source: char, seq: u64, text: &str) -> S {
    S {
        source: source.to_string().repeat(32),
        sequence: seq,
        text: text.into(),
    }
}
#[test]
fn initial_difference_and_simultaneous_edits_preserve_both_sides() {
    let mut state = SyncState::default();
    let instance = "b".repeat(32);
    assert_eq!(
        state
            .plan(&snap('a', 1, "host"), &snap('b', 1, "guest"), &instance)
            .unwrap(),
        Decision::Conflict
    );
    assert_eq!(
        state
            .plan(&snap('a', 2, "same"), &snap('b', 2, "same"), &instance)
            .unwrap(),
        Decision::Idle
    );
    assert_eq!(
        state
            .plan(
                &snap('a', 3, "new host"),
                &snap('b', 3, "new guest"),
                &instance
            )
            .unwrap(),
        Decision::Conflict
    );
}
#[test]
fn one_sided_changes_require_ack_and_equal_text_avoids_echo() {
    let mut state = SyncState::default();
    let instance = "b".repeat(32);
    let h = snap('a', 1, "same");
    let g = snap('b', 1, "same");
    assert_eq!(state.plan(&h, &g, &instance).unwrap(), Decision::Idle);
    let changed = snap('a', 2, "Xin chào 👋");
    assert_eq!(state.plan(&changed, &g, &instance).unwrap(), Decision::Push);
    assert_eq!(state.plan(&changed, &g, &instance).unwrap(), Decision::Push);
    state.commit(&changed, &changed, &instance).unwrap();
    assert_eq!(
        state.plan(&changed, &changed, &instance).unwrap(),
        Decision::Idle
    );
    assert_eq!(
        state
            .plan(&changed, &snap('b', 2, "Từ máy ảo 🎉"), &instance)
            .unwrap(),
        Decision::Pull
    );
    assert_eq!(
        state.plan(&changed, &changed, &"c".repeat(32)).unwrap(),
        Decision::Restarted
    );
}
struct Memory(Arc<Mutex<String>>);
impl Backend for Memory {
    fn read(&mut self) -> io::Result<String> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn write(&mut self, text: &str) -> io::Result<()> {
        *self.0.lock().unwrap() = text.into();
        Ok(())
    }
}
#[test]
fn conditional_native_apply_refuses_changed_destination() {
    let data = Arc::new(Mutex::new("keep".into()));
    let mut machine = Machine::new(Memory(data.clone()), "b".repeat(32)).unwrap();
    let expected = machine.read().unwrap();
    *data.lock().unwrap() = "local edit".into();
    assert!(!machine
        .accept_if_unchanged(&expected, snap('a', 1, "remote"))
        .unwrap());
    assert_eq!(*data.lock().unwrap(), "local edit");
    let current = machine.read().unwrap();
    assert!(machine
        .accept_if_unchanged(&current, snap('a', 2, "Xin chào"))
        .unwrap());
    assert_eq!(*data.lock().unwrap(), "Xin chào");
}
#[test]
fn automatic_mutations_require_version6_and_bounded_expected_snapshot() {
    use freelane_agent_protocol::{Action, Request};
    let action = || Action::ClipboardSyncSet {
        expected: snap('b', 1, "keep"),
        update: snap('a', 1, "new"),
    };
    assert!(Request {
        version: 4,
        id: 1,
        action: action()
    }
    .validate()
    .is_err());
    assert!(Request {
        version: 6,
        id: 1,
        action: action()
    }
    .validate()
    .is_ok());
    assert!(Request {
        version: 6,
        id: 1,
        action: Action::Launch {
            executable: "must-not-run".into(),
            arguments: vec![]
        }
    }
    .validate()
    .is_err());
    assert!(Request {
        version: 6,
        id: 1,
        action: Action::ClipboardSyncSet {
            expected: snap('b', 1, &"x".repeat(8193)),
            update: snap('a', 1, "new")
        }
    }
    .validate()
    .is_err());
}
#[test]
fn host_and_guest_changes_observed_during_poll_are_not_overwritten() {
    use freelane_agent_protocol::{clipboard_sync::tick, Action, Outcome};
    let data = Arc::new(Mutex::new("same".into()));
    let mut machine = Machine::new(Memory(data.clone()), "a".repeat(32)).unwrap();
    let mut policy = SyncState::default();
    let guest = snap('b', 1, "same");
    let mut rpc = |_: Action| {
        Ok(Outcome::ClipboardSync {
            instance: "b".repeat(32),
            snapshot: guest.clone(),
        })
    };
    assert_eq!(
        tick(&mut machine, &mut policy, &mut rpc, &|| false).unwrap(),
        Decision::Idle
    );
    let mut rpc = |_: Action| {
        *data.lock().unwrap() = "local edit".into();
        Ok(Outcome::ClipboardSync {
            instance: "b".repeat(32),
            snapshot: snap('b', 2, "guest edit"),
        })
    };
    assert_eq!(
        tick(&mut machine, &mut policy, &mut rpc, &|| false).unwrap(),
        Decision::Conflict
    );
    assert_eq!(*data.lock().unwrap(), "local edit");
}
