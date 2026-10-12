//! Execute only in the guest's interactive user session. Never install as a privileged host service.
use freelane_agent_protocol::{Action, Outcome, Request, Response, VERSION};
use std::io::{self, BufRead, Write};
#[cfg(windows)]
mod windows;
pub trait Launcher {
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
            Action::Ping if request.version == freelane_agent_protocol::WINDOW_VERSION => {
                Outcome::Capabilities {
                    window_snapshots: launcher.window_support(),
                }
            }
            Action::Ping => Outcome::Pong {
                os: std::env::consts::OS.into(),
                arch: std::env::consts::ARCH.into(),
            },
            Action::Launch {
                executable,
                arguments,
            } => match launcher.launch(&executable, &arguments) {
                Ok(pid) => Outcome::Launched { pid },
                Err(error) => Outcome::Failed {
                    message: format!("Launch failed ({:?})", error.kind()),
                },
            },
            ref action => launcher
                .window_action(action)
                .unwrap_or_else(|e| Outcome::Failed {
                    message: e.to_string(),
                }),
        },
    };
    Response {
        version: if request.version == freelane_agent_protocol::WINDOW_VERSION {
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
