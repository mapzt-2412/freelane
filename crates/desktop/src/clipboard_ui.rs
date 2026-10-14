//! Explicit opt-in transfers. Clipboard text never enters notices or routine logs.
use super::*;
use freelane_agent_protocol::{
    clipboard::{Backend, Machine},
    CLIPBOARD_VERSION,
};
pub struct Native(arboard::Clipboard);
impl Backend for Native {
    fn read(&mut self) -> std::io::Result<String> {
        self.0
            .get_text()
            .map_err(|_| std::io::Error::other("Host text clipboard unavailable"))
    }
    fn write(&mut self, text: &str) -> std::io::Result<()> {
        self.0
            .set_text(text)
            .map_err(|_| std::io::Error::other("Cannot write host text clipboard"))
    }
}
pub type Session = Arc<Mutex<Machine<Native>>>;
fn session() -> Result<Session, String> {
    let board = arboard::Clipboard::new()
        .map_err(|_| "Host clipboard requires an interactive session".to_string())?;
    let source = freelane::clipboard_source().map_err(|e| e.to_string())?;
    let machine = Machine::new(Native(board), source).map_err(|e| e.to_string())?;
    Ok(Arc::new(Mutex::new(machine)))
}
impl Desktop {
    pub fn clipboard_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let lang = self.language;
        if ui
            .checkbox(
                &mut self.clipboard_enabled,
                lang.text("Share text clipboard"),
            )
            .changed()
        {
            self.stop_clipboard_auto();
            if let Some(permission) = &self.clipboard_permission {
                permission.store(false, std::sync::atomic::Ordering::Relaxed);
            }
            self.clipboard_permission = self
                .clipboard_enabled
                .then(|| Arc::new(std::sync::atomic::AtomicBool::new(true)));
        }
        if !self.clipboard_enabled {
            return;
        }
        if ui
            .checkbox(
                &mut self.clipboard_auto_enabled,
                lang.text("Automatically synchronize text"),
            )
            .changed()
        {
            let enabled = self.clipboard_auto_enabled;
            self.stop_clipboard_auto();
            if enabled {
                self.clipboard_auto_enabled = true;
                self.clipboard_auto_permission =
                    Some(Arc::new(std::sync::atomic::AtomicBool::new(true)));
                self.clipboard_auto_at = None;
            }
        }
        if self.clipboard_auto_paused {
            ui.colored_label(
                egui::Color32::from_rgb(160, 90, 25),
                lang.text("Clipboard conflict. Choose Send or Copy"),
            );
        }
        let mut push = None;
        ui.add_enabled_ui(self.pending.is_none(), |ui| {
            ui.horizontal(|ui| {
                if ui.button(lang.text("Send clipboard to guest")).clicked() {
                    push = Some(true);
                }
                if ui.button(lang.text("Copy clipboard from guest")).clicked() {
                    push = Some(false);
                }
            });
        });
        let Some(push) = push else { return };
        let Some(vm) = &self.vm else { return };
        let path = vm.path.clone();
        let current = self.clipboard_session.clone();
        let Some(permission) = self.clipboard_permission.clone() else {
            return;
        };
        self.work(ctx,move || {
          let transfer=(||->Result<Event,String> {
           let allowed=|| if permission.load(std::sync::atomic::Ordering::Relaxed) {Ok(())} else {Err("Clipboard sharing disabled".to_string())};
           allowed()?;
           let report=freelane::agent_capabilities::inspect(&path).map_err(|e|e.to_string())?;
           if report.supports("text_clipboard",CLIPBOARD_VERSION)!=Some(true) {return Err("Update the guest agent and run it in an interactive session for clipboard sharing".into());}
           allowed()?;
           let session=match current {Some(session)=>session,None=>session()?};
           let mut machine=session.lock().map_err(|_|"Clipboard state unavailable".to_string())?;
           if push {
            let snapshot=machine.snapshot_for_send().map_err(|e|e.to_string())?;
            allowed()?;
            let reply=freelane::agent::clipboard_request_cancellable(&path,Action::ClipboardSet{source:snapshot.source.clone(),sequence:snapshot.sequence,text:snapshot.text},&||!permission.load(std::sync::atomic::Ordering::Relaxed)).map_err(|e|e.to_string())?;
            match reply.outcome {Outcome::ClipboardAccepted{source,sequence} if source==snapshot.source && sequence==snapshot.sequence=>{},_=>return Err("Guest clipboard transfer failed; nothing is retried automatically".into())}
           } else {
            let reply=freelane::agent::clipboard_request_cancellable(&path,Action::ClipboardGet,&||!permission.load(std::sync::atomic::Ordering::Relaxed)).map_err(|e|e.to_string())?;
            let Outcome::Clipboard{snapshot}=reply.outcome else {return Err("Guest clipboard unavailable".into())};
            allowed()?;
            machine.accept_read(snapshot).map_err(|e|e.to_string())?;
           }
           drop(machine);Ok(Event::Clipboard(session,"Clipboard transferred".into()))
          })();
          Ok(transfer.unwrap_or_else(Event::ClipboardFailed))
        });
    }
}

impl Desktop {
    pub fn stop_clipboard_auto(&mut self) {
        if let Some(permission) = &self.clipboard_auto_permission {
            permission.store(false, std::sync::atomic::Ordering::Relaxed);
        }
        self.clipboard_auto_enabled = false;
        self.clipboard_auto_permission = None;
        self.clipboard_auto_policy = Default::default();
        self.clipboard_auto_paused = false;
        self.clipboard_auto_checked = false;
        self.clipboard_auto_at = None;
    }
    pub fn poll_clipboard_auto(&mut self, ctx: &egui::Context) {
        if !self.clipboard_enabled || !self.clipboard_auto_enabled || self.clipboard_auto_paused {
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(750));
        if self.pending.is_some()
            || self
                .clipboard_auto_at
                .is_some_and(|at| at.elapsed() < Duration::from_millis(750))
        {
            return;
        }
        let Some(vm) = &self.vm else {
            return;
        };
        let path = vm.path.clone();
        let Some(permission) = self.clipboard_auto_permission.clone() else {
            return;
        };
        let Some(sharing) = self.clipboard_permission.clone() else {
            return;
        };
        let current = self.clipboard_session.clone();
        let mut policy = self.clipboard_auto_policy.clone();
        let checked = self.clipboard_auto_checked;
        self.clipboard_auto_at = Some(Instant::now());
        self.work(ctx, move || {
            let cancelled = || {
                !permission.load(std::sync::atomic::Ordering::Relaxed)
                    || !sharing.load(std::sync::atomic::Ordering::Relaxed)
            };
            if cancelled() {
                return Ok(Event::ClipboardFailed(String::new()));
            }
            let session = match current {
                Some(s) => s,
                None => match session() {
                    Ok(s) => s,
                    Err(e) => return Ok(Event::ClipboardFailed(e)),
                },
            };
            let result =
                (|| -> Result<freelane_agent_protocol::clipboard_sync::Decision, String> {
                    if !checked {
                        let report = freelane::agent_capabilities::inspect(&path)
                            .map_err(|_| "Clipboard synchronization unavailable".to_string())?;
                        if report.supports(
                            "clipboard_sync",
                            freelane_agent_protocol::CLIPBOARD_SYNC_VERSION,
                        ) != Some(true)
                        {
                            return Err("Update guest agent for automatic clipboard".into());
                        }
                    }
                    let mut machine = session
                        .lock()
                        .map_err(|_| "Clipboard state unavailable".to_string())?;
                    freelane_agent_protocol::clipboard_sync::tick(
                        &mut *machine,
                        &mut policy,
                        &mut |action| {
                            freelane::agent::clipboard_sync_request(&path, action, &cancelled)
                                .map(|r| r.outcome)
                                .map_err(|_| {
                                    std::io::Error::other("Clipboard synchronization unavailable")
                                })
                        },
                        &cancelled,
                    )
                    .map_err(|_| "Clipboard synchronization unavailable".to_string())
                })();
            Ok(Event::ClipboardAuto(session, policy, permission, result))
        });
    }
}
