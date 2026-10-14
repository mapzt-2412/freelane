//! Bounded text clipboard state; never log clipboard payloads.
use crate::{invalid, ClipboardSnapshot};
use std::{collections::BTreeMap, io};
pub trait Backend {
    fn read(&mut self) -> io::Result<String>;
    fn write(&mut self, text: &str) -> io::Result<()>;
}
pub struct Machine<B> {
    backend: B,
    source: String,
    sequence: u64,
    last: Option<ClipboardSnapshot>,
    seen: BTreeMap<String, u64>,
}
impl<B: Backend> Machine<B> {
    pub fn new(backend: B, source: String) -> io::Result<Self> {
        ClipboardSnapshot {
            source: source.clone(),
            sequence: 1,
            text: String::new(),
        }
        .validate()?;
        Ok(Self {
            backend,
            source,
            sequence: 0,
            last: None,
            seen: BTreeMap::new(),
        })
    }
    pub fn snapshot_for_send(&mut self) -> io::Result<ClipboardSnapshot> {
        let text = self.backend.read()?;
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| invalid("Clipboard sequence exhausted"))?;
        let snapshot = ClipboardSnapshot {
            source: self.source.clone(),
            sequence,
            text,
        };
        snapshot.validate()?;
        self.sequence = sequence;
        self.last = Some(snapshot.clone());
        Ok(snapshot)
    }
    /// Explicit read result, not an automatically replayed mutation request.
    pub fn accept_read(&mut self, snapshot: ClipboardSnapshot) -> io::Result<()> {
        snapshot.validate()?;
        self.backend.write(&snapshot.text)?;
        self.last = Some(snapshot);
        Ok(())
    }
    /// Refuse changes observed since polling. Native APIs cannot lock out all other apps.
    pub fn accept_if_unchanged(
        &mut self,
        expected: &ClipboardSnapshot,
        snapshot: ClipboardSnapshot,
    ) -> io::Result<bool> {
        expected.validate()?;
        snapshot.validate()?;
        if self.read()? != *expected {
            return Ok(false);
        }
        self.write(snapshot)?;
        Ok(true)
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn read(&mut self) -> io::Result<ClipboardSnapshot> {
        let text = self.backend.read()?;
        if let Some(last) = &self.last {
            if last.text == text {
                return Ok(last.clone());
            }
        }
        let next = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| invalid("Clipboard sequence exhausted"))?;
        let snapshot = ClipboardSnapshot {
            source: self.source.clone(),
            sequence: next,
            text,
        };
        snapshot.validate()?;
        self.sequence = next;
        self.last = Some(snapshot.clone());
        Ok(snapshot)
    }
    pub fn write(&mut self, snapshot: ClipboardSnapshot) -> io::Result<()> {
        snapshot.validate()?;
        if snapshot.source == self.source {
            return Err(invalid("Clipboard echo refused"));
        }
        if self
            .seen
            .get(&snapshot.source)
            .is_some_and(|seq| snapshot.sequence <= *seq)
        {
            return Err(invalid("Stale clipboard update refused"));
        }
        if self.seen.len() >= 64 && !self.seen.contains_key(&snapshot.source) {
            return Err(invalid("Clipboard source limit reached"));
        }
        self.backend.write(&snapshot.text)?;
        self.seen.insert(snapshot.source.clone(), snapshot.sequence);
        self.last = Some(snapshot);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Memory(String);
    impl Backend for Memory {
        fn read(&mut self) -> io::Result<String> {
            Ok(self.0.clone())
        }
        fn write(&mut self, text: &str) -> io::Result<()> {
            self.0 = text.into();
            Ok(())
        }
    }
    #[test]
    fn explicit_send_and_read_can_be_repeated_without_replaying_mutations() {
        let mut host = Machine::new(Memory("same".into()), "a".repeat(32)).unwrap();
        let first = host.snapshot_for_send().unwrap();
        let second = host.snapshot_for_send().unwrap();
        assert!(second.sequence > first.sequence);
        assert_eq!(first.text, second.text);
        let reply = ClipboardSnapshot {
            source: "b".repeat(32),
            sequence: 1,
            text: "guest".into(),
        };
        host.accept_read(reply.clone()).unwrap();
        host.backend.0 = "local edit".into();
        host.accept_read(reply).unwrap();
        assert_eq!(host.backend.0, "guest");
        assert_eq!(host.snapshot_for_send().unwrap().source, "a".repeat(32));
    }
    #[test]
    fn unicode_roundtrip_preserves_origin_and_rejects_echo_or_replay() {
        let host_id = "a".repeat(32);
        let guest_id = "b".repeat(32);
        let mut host = Machine::new(Memory("Xin chào anh 👋".into()), host_id).unwrap();
        let mut guest = Machine::new(Memory(String::new()), guest_id).unwrap();
        let sent = host.read().unwrap();
        guest.write(sent.clone()).unwrap();
        let received = guest.read().unwrap();
        assert_eq!(received, sent);
        assert!(host.write(received).is_err());
        assert!(guest.write(sent).is_err());
        assert_eq!(guest.read().unwrap().text, "Xin chào anh 👋");
        guest.backend.0 = "Từ máy ảo về Mac 🎉".into();
        let reply = guest.read().unwrap();
        host.write(reply.clone()).unwrap();
        assert_eq!(host.read().unwrap(), reply);
    }
    #[test]
    fn invalid_or_oversized_update_never_changes_native_data() {
        let mut machine = Machine::new(Memory("keep".into()), "a".repeat(32)).unwrap();
        assert!(machine
            .write(ClipboardSnapshot {
                source: "b".repeat(32),
                sequence: 1,
                text: "x".repeat(8193)
            })
            .is_err());
        assert_eq!(machine.read().unwrap().text, "keep");
    }
}
