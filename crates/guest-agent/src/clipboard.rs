use freelane_agent_protocol::{
    clipboard::{Backend, Machine},
    Action, ClipboardSnapshot, Outcome,
};
use std::{
    io,
    sync::{Mutex, OnceLock},
};
struct Native(arboard::Clipboard);
impl Backend for Native {
    fn read(&mut self) -> io::Result<String> {
        self.0.get_text().map_err(|_| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "Text clipboard unavailable in this user session",
            )
        })
    }
    fn write(&mut self, text: &str) -> io::Result<()> {
        self.0.set_text(text).map_err(|_| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Cannot write text clipboard in this user session",
            )
        })
    }
}
fn state() -> io::Result<std::sync::MutexGuard<'static, Option<Machine<Native>>>> {
    static CLIPBOARD: OnceLock<Mutex<Option<Machine<Native>>>> = OnceLock::new();
    let mut state = CLIPBOARD
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| io::Error::other("Clipboard state unavailable"))?;
    if state.is_none() {
        let board = arboard::Clipboard::new().map_err(|_| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "Clipboard requires an interactive user session",
            )
        })?;
        let mut id = [0u8; 16];
        getrandom::fill(&mut id)
            .map_err(|_| io::Error::other("Cannot initialize clipboard source"))?;
        let source = id.iter().map(|b| format!("{b:02x}")).collect();
        *state = Some(Machine::new(Native(board), source)?);
    }
    Ok(state)
}
pub fn available() -> bool {
    state().is_ok()
}
pub fn handle(action: &Action) -> io::Result<Outcome> {
    let mut state = state()?;
    let machine = state
        .as_mut()
        .ok_or_else(|| io::Error::other("Clipboard state unavailable"))?;
    match action {
        Action::ClipboardSyncGet => Ok(Outcome::ClipboardSync {
            instance: machine.source().into(),
            snapshot: machine.read()?,
        }),
        Action::ClipboardSyncSet { expected, update } => {
            if machine.accept_if_unchanged(expected, update.clone())? {
                Ok(Outcome::ClipboardAccepted {
                    source: update.source.clone(),
                    sequence: update.sequence,
                })
            } else {
                Ok(Outcome::ClipboardConflict)
            }
        }
        Action::ClipboardGet => Ok(Outcome::Clipboard {
            snapshot: machine.read()?,
        }),
        Action::ClipboardSet {
            source,
            sequence,
            text,
        } => {
            machine.write(ClipboardSnapshot {
                source: source.clone(),
                sequence: *sequence,
                text: text.clone(),
            })?;
            Ok(Outcome::ClipboardAccepted {
                source: source.clone(),
                sequence: *sequence,
            })
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid clipboard action",
        )),
    }
}
