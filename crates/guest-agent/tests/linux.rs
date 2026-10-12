#![cfg(target_os = "linux")]
use freelane_agent_protocol::{Action, Outcome, Request, Response, VERSION};
use freelane_guest_agent::{Launcher, LinuxLauncher};
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn real_linux_stdio_launch_preserves_arguments_without_protocol_output() {
    let directory =
        std::env::temp_dir().join(format!("freelane-linux-agent-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let script = directory.join("app");
    let output = directory.join("result.txt");
    fs::write(&script, b"#!/bin/sh\noutput=$1; shift\nprintf '%s\\n' \"$@\" > \"$output\"\necho CHILD_OUTPUT\necho CHILD_ERROR >&2\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    let literal = "spaces & $(touch SHOULD_NOT_EXIST) ; x";
    let mut agent = Command::new(env!("CARGO_BIN_EXE_freelane-guest-agent"))
        .arg("--stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = Vec::new();
    for (id, action) in [
        (1, Action::Ping),
        (
            2,
            Action::Launch {
                executable: script.to_str().unwrap().into(),
                arguments: vec![output.to_str().unwrap().into(), literal.into()],
            },
        ),
    ] {
        freelane_agent_protocol::write_frame(
            &mut input,
            &Request {
                version: VERSION,
                id,
                action,
            },
        )
        .unwrap();
    }
    agent.stdin.take().unwrap().write_all(&input).unwrap();
    let result = agent.wait_with_output().unwrap();
    assert!(result.status.success());
    assert!(result.stderr.is_empty());
    let mut reader = result.stdout.as_slice();
    let ping: Response = freelane_agent_protocol::read_frame(&mut reader).unwrap();
    assert_eq!(ping.id, 1);
    assert!(matches!(ping.outcome, Outcome::Pong { os, .. } if os == "linux"));
    let launch: Response = freelane_agent_protocol::read_frame(&mut reader).unwrap();
    assert_eq!(launch.id, 2);
    assert!(matches!(launch.outcome, Outcome::Launched { pid } if pid > 0));
    assert!(reader.is_empty(), "Child stdout leaked into RPC");
    let deadline = Instant::now() + Duration::from_secs(3);
    while !output.exists() {
        assert!(
            Instant::now() < deadline,
            "Launched process never wrote its file"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(fs::read_to_string(output).unwrap().trim_end(), literal);
    assert!(!directory.join("SHOULD_NOT_EXIST").exists());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn linux_launcher_rejects_relative_paths_and_reaps_children() {
    let mut launcher = LinuxLauncher;
    assert!(launcher.launch("sleep", &[]).is_err());
    let pid = launcher.launch("/bin/sleep", &["0.05".into()]).unwrap();
    let process = std::path::PathBuf::from(format!("/proc/{pid}"));
    let deadline = Instant::now() + Duration::from_secs(3);
    while process.exists() {
        assert!(Instant::now() < deadline, "Child remained as a zombie");
        std::thread::sleep(Duration::from_millis(10));
    }
}
