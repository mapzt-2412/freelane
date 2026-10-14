//! Execute only in the guest's interactive user session. Never install as a privileged host service.
mod documents;
pub mod transfer;
use freelane_agent_protocol::{Action, Outcome, Request, Response, VERSION};
use std::io::{self, BufRead, Write};
#[cfg(any(windows, target_os = "linux"))]
mod clipboard;
#[cfg(windows)]
mod windows;
pub trait Launcher {
    fn open_document(&mut self, _: &std::path::Path) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn transfer_support(&self) -> bool {
        false
    }
    fn transfer_action(
        &mut self,
        _: &freelane_agent_protocol::TransferAction,
    ) -> io::Result<Outcome> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn clipboard_support(&self) -> bool {
        false
    }
    fn clipboard_action(&mut self, _action: &Action) -> io::Result<Outcome> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Clipboard unavailable",
        ))
    }
    fn window_action(&mut self, _action: &Action) -> io::Result<Outcome> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows interactive window extension unavailable",
        ))
    }
    fn window_support(&self) -> bool {
        false
    }
    fn launch(&mut self, executable: &str, arguments: &[String]) -> io::Result<u32>;
}
pub fn handle(request: Request, launcher: &mut impl Launcher) -> Response {
    let outcome = match request.validate() {
        Err(error) => Outcome::Failed {
            message: error.to_string(),
        },
        Ok(()) => match request.action {
            Action::Ping if request.version == freelane_agent_protocol::CAPABILITY_VERSION => {
                let mut features = vec![freelane_agent_protocol::Feature {
                    name: "launch".into(),
                    protocol: VERSION,
                }];
                if launcher.transfer_support() {
                    features.push(freelane_agent_protocol::Feature {
                        name: "file_transfer".into(),
                        protocol: freelane_agent_protocol::TRANSFER_VERSION,
                    });
                }
                if launcher.window_support() {
                    features.push(freelane_agent_protocol::Feature {
                        name: "window_snapshots".into(),
                        protocol: freelane_agent_protocol::WINDOW_VERSION,
                    });
                }
                if launcher.clipboard_support() {
                    features.push(freelane_agent_protocol::Feature {
                        name: "text_clipboard".into(),
                        protocol: freelane_agent_protocol::CLIPBOARD_VERSION,
                    });
                }
                Outcome::AgentInfo {
                    info: freelane_agent_protocol::AgentInfo {
                        agent_version: env!("CARGO_PKG_VERSION").into(),
                        os: std::env::consts::OS.into(),
                        arch: std::env::consts::ARCH.into(),
                        protocols: vec![
                            VERSION,
                            freelane_agent_protocol::WINDOW_VERSION,
                            freelane_agent_protocol::CAPABILITY_VERSION,
                        ]
                        .into_iter()
                        .chain(if launcher.clipboard_support() {
                            Some(freelane_agent_protocol::CLIPBOARD_VERSION)
                        } else {
                            None
                        })
                        .chain(
                            launcher
                                .transfer_support()
                                .then_some(freelane_agent_protocol::TRANSFER_VERSION),
                        )
                        .collect(),
                        features,
                        max_frame: freelane_agent_protocol::MAX_FRAME as u32,
                    },
                }
            }
            Action::Ping if request.version == freelane_agent_protocol::WINDOW_VERSION => {
                Outcome::Capabilities {
                    window_snapshots: launcher.window_support(),
                }
            }
            Action::Ping => Outcome::Pong {
                os: std::env::consts::OS.into(),
                arch: std::env::consts::ARCH.into(),
            },
            Action::Transfer { ref operation } => launcher
                .transfer_action(operation)
                .unwrap_or_else(|_| Outcome::Failed {
                    message: "File transfer unavailable or refused".into(),
                }),
            Action::Launch {
                executable,
                arguments,
            } => match launcher.launch(&executable, &arguments) {
                Ok(pid) => Outcome::Launched { pid },
                Err(error) => Outcome::Failed {
                    message: format!("Launch failed ({:?})", error.kind()),
                },
            },
            ref action @ (Action::ClipboardGet | Action::ClipboardSet { .. }) => launcher
                .clipboard_action(action)
                .unwrap_or_else(|_| Outcome::Failed {
                    message: "Text clipboard transfer unavailable or refused".into(),
                }),
            ref action => launcher
                .window_action(action)
                .unwrap_or_else(|e| Outcome::Failed {
                    message: e.to_string(),
                }),
        },
    };
    Response {
        version: if [
            freelane_agent_protocol::WINDOW_VERSION,
            freelane_agent_protocol::CAPABILITY_VERSION,
            freelane_agent_protocol::CLIPBOARD_VERSION,
            freelane_agent_protocol::TRANSFER_VERSION,
        ]
        .contains(&request.version)
        {
            request.version
        } else {
            VERSION
        },
        id: request.id,
        outcome,
    }
}
pub fn serve(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    launcher: &mut impl Launcher,
) -> io::Result<()> {
    loop {
        let request = match freelane_agent_protocol::read_frame(reader) {
            Ok(request) => request,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error),
        };
        freelane_agent_protocol::write_frame(writer, &handle(request, launcher))?;
    }
}
#[cfg(windows)]
pub struct WindowsLauncher;
#[cfg(windows)]
impl Launcher for WindowsLauncher {
    fn open_document(&mut self, p: &std::path::Path) -> io::Result<()> {
        documents::open(p)
    }
    fn clipboard_support(&self) -> bool {
        clipboard::available()
    }
    fn clipboard_action(&mut self, action: &Action) -> io::Result<Outcome> {
        clipboard::handle(action)
    }
    fn window_support(&self) -> bool {
        true
    }
    fn window_action(&mut self, action: &Action) -> io::Result<Outcome> {
        windows::handle(action)
    }
    fn launch(&mut self, executable: &str, arguments: &[String]) -> io::Result<u32> {
        if !std::path::Path::new(executable).is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Executable must be an absolute Windows path",
            ));
        }
        if !std::path::Path::new(executable)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Use an executable .exe path; batch files require an explicit interpreter",
            ));
        }
        // Command uses CreateProcess with literal argument quoting; no cmd.exe/PowerShell intermediary.
        let child = std::process::Command::new(executable)
            .args(arguments)
            // A child must not consume RPC input or inject console text into --stdio replies.
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        let id = child.id();
        drop(child);
        Ok(id)
    }
}
#[cfg(target_os = "linux")]
pub struct LinuxLauncher;
#[cfg(target_os = "linux")]
impl Launcher for LinuxLauncher {
    fn open_document(&mut self, p: &std::path::Path) -> io::Result<()> {
        documents::open(p)
    }
    fn clipboard_support(&self) -> bool {
        clipboard::available()
    }
    fn clipboard_action(&mut self, action: &Action) -> io::Result<Outcome> {
        clipboard::handle(action)
    }
    fn launch(&mut self, executable: &str, arguments: &[String]) -> io::Result<u32> {
        let path = std::path::Path::new(executable);
        if !path.is_absolute() || !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Use an absolute Linux executable file path",
            ));
        }
        let mut child = std::process::Command::new(path)
            .args(arguments)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        let pid = child.id();
        // Reap asynchronously: RPC must not wait for a long-lived GUI application.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake(Vec<(String, Vec<String>)>);
    impl Launcher for Fake {
        fn launch(&mut self, exe: &str, args: &[String]) -> io::Result<u32> {
            self.0.push((exe.into(), args.into()));
            Ok(42)
        }
    }
    #[test]
    fn window_capability_handshake_preserves_v1_ping_and_rejects_v1_mutations() {
        let mut fake = Fake(vec![]);
        let ping = |version| Request {
            version,
            id: 3,
            action: Action::Ping,
        };
        assert!(matches!(
            handle(ping(VERSION), &mut fake).outcome,
            Outcome::Pong { .. }
        ));
        let response = handle(ping(freelane_agent_protocol::WINDOW_VERSION), &mut fake);
        assert_eq!(response.version, freelane_agent_protocol::WINDOW_VERSION);
        assert_eq!(
            response.outcome,
            Outcome::Capabilities {
                window_snapshots: false
            }
        );
        assert!(matches!(
            handle(
                Request {
                    version: VERSION,
                    id: 4,
                    action: Action::Windows { pid: 42 }
                },
                &mut fake
            )
            .outcome,
            Outcome::Failed { .. }
        ));
        assert!(fake.0.is_empty());
    }
    #[test]
    fn invalid_request_never_reaches_process_launcher() {
        let mut fake = Fake(vec![]);
        let reply = handle(
            Request {
                version: 99,
                id: 3,
                action: Action::Launch {
                    executable: "x".into(),
                    arguments: vec![],
                },
            },
            &mut fake,
        );
        assert!(matches!(reply.outcome, Outcome::Failed { .. }));
        assert!(fake.0.is_empty());
    }
    #[test]
    fn launch_preserves_arguments_and_returns_correlated_pid() {
        let mut fake = Fake(vec![]);
        let reply = handle(
            Request {
                version: VERSION,
                id: 17,
                action: Action::Launch {
                    executable: "C:\\Program Files\\App\\app.exe".into(),
                    arguments: vec!["a & b".into()],
                },
            },
            &mut fake,
        );
        assert_eq!(reply.id, 17);
        assert_eq!(reply.outcome, Outcome::Launched { pid: 42 });
        assert_eq!(fake.0[0].1, vec!["a & b"]);
    }
    #[test]
    fn server_handles_multiple_framed_requests() {
        let mut input = Vec::new();
        for id in [1, 2] {
            freelane_agent_protocol::write_frame(
                &mut input,
                &Request {
                    version: VERSION,
                    id,
                    action: Action::Ping,
                },
            )
            .unwrap();
        }
        let mut output = Vec::new();
        serve(&mut input.as_slice(), &mut output, &mut Fake(vec![])).unwrap();
        let mut reader = output.as_slice();
        for id in [1, 2] {
            let reply: Response = freelane_agent_protocol::read_frame(&mut reader).unwrap();
            assert_eq!(reply.id, id);
            assert!(matches!(reply.outcome, Outcome::Pong { .. }));
        }
    }
}

#[cfg(test)]
mod discovery_tests {
    use super::*;
    struct NoLaunch;
    impl Launcher for NoLaunch {
        fn launch(&mut self, _: &str, _: &[String]) -> io::Result<u32> {
            panic!("Discovery must not launch anything")
        }
    }
    #[test]
    fn v3_discovery_reports_versions_and_actual_features_without_mutating() {
        let mut launcher = NoLaunch;
        let response = handle(
            Request {
                version: 3,
                id: 91,
                action: Action::Ping,
            },
            &mut launcher,
        );
        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["version"], 3);
        assert_eq!(json["outcome"]["status"], "agent_info");
        assert_eq!(
            json["outcome"]["info"]["protocols"],
            serde_json::json!([1, 2, 3])
        );
        assert_eq!(
            json["outcome"]["info"]["features"],
            serde_json::json!([{"name":"launch","protocol":1}])
        );
        assert_eq!(json["outcome"]["info"]["max_frame"], 65536);
        let response = handle(
            Request {
                version: 3,
                id: 92,
                action: Action::Launch {
                    executable: "must-not-run".into(),
                    arguments: vec![],
                },
            },
            &mut launcher,
        );
        assert!(matches!(response.outcome, Outcome::Failed { .. }));
    }
}

/// Explicitly configured inbox; transfer state belongs to this agent session.
pub struct TransferLauncher<L> {
    inner: L,
    root: Option<cap_std::fs::Dir>,
    completed: Option<(String, String, u64, String)>,
    active: Option<(String, transfer::Transfer, std::time::Instant)>,
}
impl<L> TransferLauncher<L> {
    pub fn new(inner: L, root: Option<&std::path::Path>) -> io::Result<Self> {
        Ok(Self {
            inner,
            root: root
                .map(|p| cap_std::fs::Dir::open_ambient_dir(p, cap_std::ambient_authority()))
                .transpose()?,
            active: None,
            completed: None,
        })
    }
    pub fn disconnect(&mut self) {
        self.active = None;
        self.completed = None;
    }
}
impl<L: Launcher> Launcher for TransferLauncher<L> {
    fn launch(&mut self, e: &str, a: &[String]) -> io::Result<u32> {
        self.inner.launch(e, a)
    }
    fn window_support(&self) -> bool {
        self.inner.window_support()
    }
    fn window_action(&mut self, a: &Action) -> io::Result<Outcome> {
        self.inner.window_action(a)
    }
    fn clipboard_support(&self) -> bool {
        self.inner.clipboard_support()
    }
    fn clipboard_action(&mut self, a: &Action) -> io::Result<Outcome> {
        self.inner.clipboard_action(a)
    }
    fn transfer_support(&self) -> bool {
        self.root.is_some()
    }
    fn transfer_action(
        &mut self,
        a: &freelane_agent_protocol::TransferAction,
    ) -> io::Result<Outcome> {
        use freelane_agent_protocol::TransferAction as T;
        a.validate()?;
        if self
            .active
            .as_ref()
            .is_some_and(|(_, _, time)| time.elapsed() > std::time::Duration::from_secs(300))
        {
            self.active = None;
        }
        let root = self.root.as_ref().ok_or(io::ErrorKind::Unsupported)?;
        if let T::Open { token } = a {
            let (id, name, size, hash) = self.completed.as_ref().ok_or(io::ErrorKind::NotFound)?;
            if id != token {
                return Err(io::ErrorKind::NotFound.into());
            }
            let file = root.open(name)?.into_std();
            let path = documents::verified_path(file, name, *size, hash)?;
            self.inner.open_document(&path)?;
            return Ok(Outcome::DocumentOpened);
        }
        if let T::Begin { name, size, sha256 } = a {
            if self.active.is_some() {
                return Err(io::ErrorKind::WouldBlock.into());
            }
            let mut random = [0; 16];
            getrandom::fill(&mut random).map_err(|_| io::ErrorKind::Other)?;
            let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
            self.active = Some((
                token.clone(),
                transfer::Transfer::begin_dir(root.try_clone()?, name, *size, sha256)?,
                std::time::Instant::now(),
            ));
            return Ok(Outcome::TransferReady { token });
        }
        let token = match a {
            T::Chunk { token, .. } | T::Finish { token } | T::Cancel { token } => token,
            _ => unreachable!(),
        };
        if !self.active.as_ref().is_some_and(|(id, _, _)| id == token) {
            return Err(io::ErrorKind::NotFound.into());
        }
        match a {
            T::Chunk {
                offset, data_hex, ..
            } => {
                let data: Vec<u8> = data_hex
                    .as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                    .collect();
                let (_, t, time) = self.active.as_mut().unwrap();
                t.write(*offset, &data)?;
                *time = std::time::Instant::now();
                Ok(Outcome::TransferProgress {
                    received: t.received(),
                })
            }
            T::Finish { .. } => {
                let (token, t, _) = self.active.take().unwrap();
                let (name, size, hash) = t.receipt();
                t.finish()?;
                self.completed = Some((token, name, size, hash));
                Ok(Outcome::TransferComplete)
            }
            T::Cancel { .. } => {
                self.active = None;
                Ok(Outcome::TransferCanceled)
            }
            _ => unreachable!(),
        }
    }
}
