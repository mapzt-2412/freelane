//! Execute only in the guest's interactive user session. Never install as a privileged host service.
use freelane_agent_protocol::{Action, Outcome, Request, Response, VERSION};
use std::io::{self, BufRead, Write};
pub trait Launcher {
    fn launch(&mut self, executable: &str, arguments: &[String]) -> io::Result<u32>;
}
pub fn handle(request: Request, launcher: &mut impl Launcher) -> Response {
    let outcome = match request.validate() {
        Err(error) => Outcome::Failed {
            message: error.to_string(),
        },
        Ok(()) => match request.action {
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
        },
    };
    Response {
        version: VERSION,
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
