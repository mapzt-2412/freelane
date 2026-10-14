//! Controlled protocol fixtures: these checks do not establish native Windows app behavior.
use freelane::app_launcher::{open, AppProfile};
use freelane_agent_protocol::{
    Action, AgentInfo, Feature, Outcome, Request, Response, WindowInfo, WindowTarget,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    thread,
    time::{Duration, Instant},
};
fn accept(listener: &UnixListener) -> std::os::unix::net::UnixStream {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        match listener.accept() {
            Ok((s, _)) => return s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "Fixture request missing");
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("{e}"),
        }
    }
}
#[test]
fn existing_app_is_reattached_without_launch_only_when_arguments_are_empty() {
    for with_arguments in [false, true] {
        let root =
            std::env::temp_dir().join(format!("fl-reattach-{:016x}", getrandom::u64().unwrap()));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let config = root.join("vm.toml");
        fs::write(root.join("disk.qcow2"), b"Nonbootable protocol fixture").unwrap();
        fs::write(&config,"name='Reattach fixture'\nguest_os='windows'\ncpus=2\nmemory_mib=2048\ndisk='disk.qcow2'\n[runtime]\nfirmware='code.fd'\nnvram='vars.fd'\nguest_agent=true\n").unwrap();
        let runtime = freelane::qemu::runtime_dir(&config).unwrap();
        fs::create_dir(&runtime).unwrap();
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        let qmp = UnixListener::bind(runtime.join("qmp.sock")).unwrap();
        qmp.set_nonblocking(true).unwrap();
        let control = thread::spawn(move || {
            for _ in 0..2 {
                let mut socket = BufReader::new(accept(&qmp));
                writeln!(socket.get_mut(), "{}", json!({"QMP":{"version":{}}})).unwrap();
                for _ in 0..2 {
                    let mut line = String::new();
                    socket.read_line(&mut line).unwrap();
                    let req: Value = serde_json::from_str(&line).unwrap();
                    writeln!(
                        socket.get_mut(),
                        "{}",
                        json!({"id":req["id"],"return":{"running":true}})
                    )
                    .unwrap();
                }
            }
        });
        let listener = UnixListener::bind(runtime.join("agent.sock")).unwrap();
        listener.set_nonblocking(true).unwrap();
        let agent = thread::spawn(move || {
            let mut launches = 0;
            for _ in 0..if with_arguments { 5 } else { 4 } {
                let mut socket = accept(&listener);
                let req: Request = freelane_agent_protocol::read_frame(&mut BufReader::new(
                    socket.try_clone().unwrap(),
                ))
                .unwrap();
                let windows = || {
                    vec![WindowInfo {
                        target: WindowTarget {
                            handle: 10,
                            pid: 20,
                            created: 30,
                            incarnation: 40,
                        },
                        title: "Synthetic window metadata".into(),
                    }]
                };
                let outcome = match req.action {
                    Action::Ping if req.version == 1 => Outcome::Pong {
                        os: "windows".into(),
                        arch: "aarch64".into(),
                    },
                    Action::Ping if req.version == 2 => Outcome::Capabilities {
                        window_snapshots: true,
                    },
                    Action::Ping if req.version == 3 => Outcome::AgentInfo {
                        info: AgentInfo {
                            agent_version: "0.1.0".into(),
                            os: "windows".into(),
                            arch: "aarch64".into(),
                            protocols: vec![1, 2, 3, 9],
                            features: vec![Feature {
                                name: "window_identity".into(),
                                protocol: 9,
                            }],
                            max_frame: 65536,
                        },
                    },
                    Action::FindAppWindows { .. } => {
                        assert!(!with_arguments);
                        Outcome::Windows { windows: windows() }
                    }
                    Action::Launch { arguments, .. } => {
                        assert!(with_arguments);
                        assert_eq!(arguments, vec!["literal document.txt"]);
                        launches += 1;
                        Outcome::Launched { pid: 20 }
                    }
                    Action::WindowInventory { pid: 20 } => Outcome::Windows { windows: windows() },
                    action => panic!("Unexpected action: {action:?}"),
                };
                freelane_agent_protocol::write_frame(
                    &mut socket,
                    &Response {
                        version: req.version,
                        id: req.id,
                        outcome,
                    },
                )
                .unwrap();
            }
            launches
        });
        let profile = AppProfile {
            name: "Fixture".into(),
            vm: config,
            executable: r"C:\Windows\System32\notepad.exe".into(),
            arguments: if with_arguments {
                vec!["literal document.txt".into()]
            } else {
                vec![]
            },
            boot_timeout_seconds: 10,
        };
        let (pid, windows) = open(&profile, &root.join("never-launch-cli")).unwrap();
        assert_eq!(pid, 20);
        assert_eq!(windows[0].target.incarnation, 40);
        control.join().unwrap();
        assert_eq!(agent.join().unwrap(), u32::from(with_arguments));
        fs::remove_dir_all(runtime).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
