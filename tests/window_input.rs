use freelane_agent_protocol::{Action, Outcome, Request, Response, WindowTarget};
use std::{io::BufReader, os::unix::net::UnixListener, thread, time::Duration};
fn exchange(version: u16, outcome: Outcome) -> freelane::Result<Response> {
    let path =
        std::env::temp_dir().join(format!("fl-input-{:016x}.sock", getrandom::u64().unwrap()));
    let listener = UnixListener::bind(&path).unwrap();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let request: Request =
            freelane_agent_protocol::read_frame(&mut BufReader::new(stream.try_clone().unwrap()))
                .unwrap();
        assert_eq!(request.version, 10);
        assert!(matches!(
            request.action,
            Action::WindowResize {
                width: 1280,
                height: 720,
                ..
            }
        ));
        freelane_agent_protocol::write_frame(
            &mut stream,
            &Response {
                id: request.id,
                version,
                outcome,
            },
        )
        .unwrap();
    });
    let result = freelane::agent::request_socket_version(
        &path,
        Action::WindowResize {
            target: WindowTarget {
                handle: 1,
                pid: 2,
                created: 3,
                incarnation: 4,
            },
            width: 1280,
            height: 720,
        },
        10,
    );
    worker.join().unwrap();
    std::fs::remove_file(path).unwrap();
    result
}
#[test]
fn extended_input_correlates_ack_and_never_accepts_legacy_or_wrong_outcome() {
    assert!(exchange(10, Outcome::WindowInputAccepted).is_ok());
    assert!(exchange(2, Outcome::WindowInputAccepted).is_err());
    assert!(exchange(10, Outcome::Launched { pid: 2 }).is_err());
    assert!(exchange(
        10,
        Outcome::Failed {
            message: "Unsupported version".into()
        }
    )
    .is_err());
}
