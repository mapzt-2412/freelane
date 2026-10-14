//! Observed-change policy; clipboard contents never enter diagnostics.
use crate::{invalid, ClipboardSnapshot};
use std::io;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Idle,
    Push,
    Pull,
    Conflict,
    Restarted,
}
#[derive(Default, Clone)]
pub struct SyncState {
    baseline: Option<(ClipboardSnapshot, ClipboardSnapshot, String)>,
}
impl SyncState {
    pub fn commit(
        &mut self,
        host: &ClipboardSnapshot,
        guest: &ClipboardSnapshot,
        instance: &str,
    ) -> io::Result<()> {
        host.validate()?;
        guest.validate()?;
        if instance.len() != 32 || !instance.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid("Invalid clipboard instance"));
        }
        self.baseline = Some((host.clone(), guest.clone(), instance.into()));
        Ok(())
    }
    pub fn plan(
        &mut self,
        host: &ClipboardSnapshot,
        guest: &ClipboardSnapshot,
        instance: &str,
    ) -> io::Result<Decision> {
        host.validate()?;
        guest.validate()?;
        if instance.len() != 32 || !instance.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid("Invalid clipboard instance"));
        }
        if self
            .baseline
            .as_ref()
            .is_some_and(|(_, _, old)| old != instance)
        {
            self.baseline = None;
            return Ok(Decision::Restarted);
        }
        if host.text == guest.text {
            self.commit(host, guest, instance)?;
            return Ok(Decision::Idle);
        }
        let Some((h, g, _)) = &self.baseline else {
            return Ok(Decision::Conflict);
        };
        Ok(match (host != h, guest != g) {
            (true, false) => Decision::Push,
            (false, true) => Decision::Pull,
            (true, true) => Decision::Conflict,
            (false, false) => Decision::Conflict,
        })
    }
}
/// One serialized poll on an explicitly supplied backend; never retry a mutation.
pub fn tick<B: crate::clipboard::Backend>(
    machine: &mut crate::clipboard::Machine<B>,
    policy: &mut SyncState,
    rpc: &mut impl FnMut(crate::Action) -> io::Result<crate::Outcome>,
    cancelled: &impl Fn() -> bool,
) -> io::Result<Decision> {
    use crate::{Action, Outcome};
    let allowed = || {
        if cancelled() {
            Err(invalid("Clipboard synchronization stopped"))
        } else {
            Ok(())
        }
    };
    allowed()?;
    let host = machine.read()?;
    let Outcome::ClipboardSync {
        instance,
        snapshot: guest,
    } = rpc(Action::ClipboardSyncGet)?
    else {
        return Err(invalid("Clipboard synchronization unavailable"));
    };
    allowed()?;
    let decision = policy.plan(&host, &guest, &instance)?;
    match decision {
        Decision::Push => {
            if machine.read()? != host {
                return Ok(Decision::Conflict);
            }
            let sent = machine.snapshot_for_send()?;
            if sent.text != host.text {
                return Ok(Decision::Conflict);
            }
            allowed()?;
            match rpc(Action::ClipboardSyncSet {
                expected: guest,
                update: sent.clone(),
            })? {
                Outcome::ClipboardAccepted { source, sequence }
                    if source == sent.source && sequence == sent.sequence =>
                {
                    policy.commit(&sent, &sent, &instance)?
                }
                Outcome::ClipboardConflict => return Ok(Decision::Conflict),
                _ => return Err(invalid("Clipboard acknowledgment refused")),
            }
        }
        Decision::Pull => {
            allowed()?;
            if !machine.accept_if_unchanged(&host, guest.clone())? {
                return Ok(Decision::Conflict);
            }
            policy.commit(&guest, &guest, &instance)?;
        }
        _ => {}
    }
    Ok(decision)
}
