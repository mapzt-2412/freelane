use freelane_agent_protocol::{Action, Outcome, Request, Response, VERSION};
use std::{
    io::BufReader,
    os::unix::net::UnixListener,
    sync::atomic::{AtomicU64, Ordering},
    thread,
};
static NEXT: AtomicU64 = AtomicU64::new(1);
fn with_server<F: FnOnce(Request) -> Response + Send + 'static>(
    respond: F,
    action: Action,
) -> freelane::Result<Response> {
    let path = std::env::temp_dir().join(format!(
        "fl-agent-{}-{}.sock",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let listener = UnixListener::bind(&path).unwrap();
    let worker = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let request =
            freelane_agent_protocol::read_frame(&mut BufReader::new(socket.try_clone().unwrap()))
                .unwrap();
        freelane_agent_protocol::write_frame(&mut socket, &respond(request)).unwrap();
    });
    let result = freelane::agent::request_socket(&path, action);
    worker.join().unwrap();
    std::fs::remove_file(path).unwrap();
    result
}
#[test]
fn host_and_guest_round_trip_preserves_launch_arguments() {
    let reply = with_server(
        |request| {
            assert_eq!(
                request.action,
                Action::Launch {
                    executable: "C:\\Program Files\\App\\app.exe".into(),
                    arguments: vec!["a & b.txt".into(), "two words".into()]
                }
            );
            Response {
                version: VERSION,
                id: request.id,
                outcome: Outcome::Launched { pid: 321 },
            }
        },
        Action::Launch {
            executable: "C:\\Program Files\\App\\app.exe".into(),
            arguments: vec!["a & b.txt".into(), "two words".into()],
        },
    )
    .unwrap();
    assert_eq!(reply.outcome, Outcome::Launched { pid: 321 });
}
#[test]
fn host_rejects_uncorrelated_reply_and_guest_errors() {
    assert!(with_server(
        |r| Response {
            version: VERSION,
            id: r.id.wrapping_add(1),
            outcome: Outcome::Launched { pid: 1 }
        },
        Action::Ping
    )
    .unwrap_err()
    .to_string()
    .contains("mismatch"));
    assert!(with_server(
        |r| Response {
            version: VERSION,
            id: r.id,
            outcome: Outcome::Failed {
                message: "Executable not found".into()
            }
        },
        Action::Ping
    )
    .unwrap_err()
    .to_string()
    .contains("Executable not found"));
}
#[test]
fn unavailable_guest_agent_is_an_error() {
    let path = std::env::temp_dir().join(format!("fl-no-agent-{}.sock", std::process::id()));
    assert!(freelane::agent::request_socket(&path, Action::Ping)
        .unwrap_err()
        .to_string()
        .contains("unavailable"));
}

#[test]
fn host_rejects_success_payload_for_wrong_operation() {
    assert!(with_server(
        |request| Response {
            version: VERSION,
            id: request.id,
            outcome: Outcome::Launched { pid: 321 }
        },
        Action::Ping
    )
    .unwrap_err()
    .to_string()
    .contains("requested action"));
}

#[test]
fn independent_cli_requests_do_not_reuse_response_ids() {
    use std::{
        fs,
        process::Command,
        time::{Duration, Instant},
    };
    let base = std::env::temp_dir().join(format!("fl-agent-nonce-{}", std::process::id()));
    fs::create_dir(&base).unwrap();
    let config = base.join("vm.toml");
    fs::write(
        &config,
        "name='nonce test'\ncpus=2\nmemory_mib=4096\ndisk='disk.qcow2'\n",
    )
    .unwrap();
    let runtime = freelane::qemu::runtime_dir(&config).unwrap();
    fs::create_dir(&runtime).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let listener = UnixListener::bind(runtime.join("agent.sock")).unwrap();
    listener.set_nonblocking(true).unwrap();
    let worker = thread::spawn(move || {
        let mut ids = Vec::new();
        for _ in 0..2 {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => {
                        // BSD/macOS may inherit O_NONBLOCK from the listening socket.
                        socket.set_nonblocking(false).unwrap();
                        break socket;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "CLI did not connect");
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("{error}"),
                }
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let request: Request = freelane_agent_protocol::read_frame(&mut BufReader::new(
                socket.try_clone().unwrap(),
            ))
            .unwrap();
            assert_ne!(request.id, 0);
            ids.push(request.id);
            freelane_agent_protocol::write_frame(
                &mut socket,
                &Response {
                    version: VERSION,
                    id: request.id,
                    outcome: Outcome::Pong {
                        os: "windows".into(),
                        arch: "aarch64".into(),
                    },
                },
            )
            .unwrap();
        }
        ids
    });
    for _ in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_freelane"))
            .arg("agent-ping")
            .arg(&config)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let ids = worker.join().unwrap();
    fs::remove_dir_all(runtime).unwrap();
    fs::remove_dir_all(base).unwrap();
    assert_ne!(
        ids[0], ids[1],
        "a delayed reply must not match the next CLI process"
    );
}

#[test]
fn window_extension_rejects_legacy_reply_and_wrong_window_identity() {
    use freelane_agent_protocol::{WindowTarget, WINDOW_VERSION};
    fn exchange(reply_version: u16, actual: WindowTarget) -> freelane::Result<Response> {
        let path = std::env::temp_dir().join(format!(
            "fl-window-{}-{}.sock",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let listener = UnixListener::bind(&path).unwrap();
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let request: Request = freelane_agent_protocol::read_frame(&mut BufReader::new(
                socket.try_clone().unwrap(),
            ))
            .unwrap();
            assert_eq!(request.version, WINDOW_VERSION);
            freelane_agent_protocol::write_frame(
                &mut socket,
                &Response {
                    version: reply_version,
                    id: request.id,
                    outcome: Outcome::WindowFrame {
                        target: actual,
                        width: 1,
                        height: 1,
                        jpeg_hex: "ffd8ffd9".into(),
                    },
                },
            )
            .unwrap();
        });
        let result = freelane::agent::request_socket_version(
            &path,
            Action::CaptureWindow {
                target: WindowTarget {
                    handle: 10,
                    pid: 20,
                    created: 30,
                },
            },
            WINDOW_VERSION,
        );
        worker.join().unwrap();
        std::fs::remove_file(path).unwrap();
        result
    }
    assert!(exchange(
        VERSION,
        WindowTarget {
            handle: 10,
            pid: 20,
            created: 30
        }
    )
    .unwrap_err()
    .to_string()
    .contains("mismatch"));
    assert!(exchange(
        WINDOW_VERSION,
        WindowTarget {
            handle: 11,
            pid: 20,
            created: 30
        }
    )
    .unwrap_err()
    .to_string()
    .contains("requested action"));
}

#[test]
fn independent_app_clients_lease_the_serial_channel_until_reply() {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        sync::mpsc,
        time::{Duration, Instant},
    };
    let root = std::env::temp_dir().join(format!(
        "fl-lease-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let config = root.join("vm.toml");
    fs::write(&config, "fixture").unwrap();
    let runtime = freelane::qemu::runtime_dir(&config).unwrap();
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let listener = UnixListener::bind(runtime.join("agent.sock")).unwrap();
    listener.set_nonblocking(true).unwrap();
    let (captured, ready) = mpsc::channel();
    let (proceed, release) = mpsc::channel();
    let server = thread::spawn(move || {
        let accept = || {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                match listener.accept() {
                    Ok(s) => {
                        s.0.set_nonblocking(false).unwrap();
                        break s.0;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => panic!("{e}"),
                }
            }
        };
        let mut first = accept();
        let request: Request =
            freelane_agent_protocol::read_frame(&mut BufReader::new(first.try_clone().unwrap()))
                .unwrap();
        captured.send(()).unwrap();
        release.recv().unwrap();
        thread::sleep(Duration::from_millis(100));
        assert!(
            matches!(listener.accept(),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock),
            "Second client connected before first response completed"
        );
        freelane_agent_protocol::write_frame(
            &mut first,
            &Response {
                version: VERSION,
                id: request.id,
                outcome: Outcome::Pong {
                    os: "windows".into(),
                    arch: "aarch64".into(),
                },
            },
        )
        .unwrap();
        drop(first);
        let mut second = accept();
        let request: Request =
            freelane_agent_protocol::read_frame(&mut BufReader::new(second.try_clone().unwrap()))
                .unwrap();
        freelane_agent_protocol::write_frame(
            &mut second,
            &Response {
                version: VERSION,
                id: request.id,
                outcome: Outcome::Pong {
                    os: "windows".into(),
                    arch: "aarch64".into(),
                },
            },
        )
        .unwrap();
    });
    let c = config.clone();
    let first = thread::spawn(move || freelane::agent::request(&c, Action::Ping));
    ready.recv().unwrap();
    let c = config.clone();
    let second = thread::spawn(move || freelane::agent::request(&c, Action::Ping));
    proceed.send(()).unwrap();
    first.join().unwrap().unwrap();
    second.join().unwrap().unwrap();
    server.join().unwrap();
    fs::remove_dir_all(runtime).unwrap();
    fs::remove_dir_all(root).unwrap();
}
