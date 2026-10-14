use freelane_agent_protocol::{
    Action, Outcome, Request, Response, WindowInfo, WindowTarget, WINDOW_IDENTITY_VERSION,
};
use std::{io::BufReader, os::unix::net::UnixListener, thread};
fn inventory(target: WindowTarget) -> freelane::Result<Response> {
    let path = std::env::temp_dir().join(format!(
        "fl-window-id-{:016x}.sock",
        getrandom::u64().unwrap()
    ));
    let listener = UnixListener::bind(&path).unwrap();
    let worker = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let request: Request =
            freelane_agent_protocol::read_frame(&mut BufReader::new(socket.try_clone().unwrap()))
                .unwrap();
        freelane_agent_protocol::write_frame(
            &mut socket,
            &Response {
                version: request.version,
                id: request.id,
                outcome: Outcome::Windows {
                    windows: vec![WindowInfo {
                        target,
                        title: "Synthetic metadata".into(),
                    }],
                },
            },
        )
        .unwrap();
    });
    let result = freelane::agent::request_socket_version(
        &path,
        Action::WindowInventory { pid: 2 },
        WINDOW_IDENTITY_VERSION,
    );
    worker.join().unwrap();
    std::fs::remove_file(path).unwrap();
    result
}
#[test]
fn host_refuses_missing_incarnation_and_other_process_inventory() {
    assert!(inventory(WindowTarget {
        handle: 1,
        pid: 2,
        created: 3,
        incarnation: 0
    })
    .is_err());
    assert!(inventory(WindowTarget {
        handle: 1,
        pid: 99,
        created: 3,
        incarnation: 4
    })
    .is_err());
    assert!(inventory(WindowTarget {
        handle: 1,
        pid: 2,
        created: 3,
        incarnation: 4
    })
    .is_ok());
}
