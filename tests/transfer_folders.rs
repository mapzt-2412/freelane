use freelane_agent_protocol::{Action, Request};
use std::{io::BufReader, os::unix::net::UnixListener};
#[test]
fn folder_reports_reject_malformed_states_paths_and_mismatched_responses() {
    for case in 0..6 {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("agent.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let peer = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let r: Request = freelane_agent_protocol::read_frame(&mut BufReader::new(
                stream.try_clone().unwrap(),
            ))
            .unwrap();
            let mut value = serde_json::json!({"version":13,"id":r.id,"outcome":{"status":"transfer_folders","report":{"inbox":"ready","outbox":"unavailable","busy":false}}});
            match case {
                1 => value["outcome"]["report"]["outbox"] = "/arbitrary".into(),
                2 => value["outcome"]["report"]["path"] = "/arbitrary".into(),
                3 => value["outcome"]["report"]["busy"] = "false".into(),
                4 => value["outcome"] = serde_json::json!({"status":"export_closed"}),
                5 => value["version"] = 12.into(),
                _ => {}
            }
            freelane_agent_protocol::write_frame(&mut stream, &value).unwrap();
        });
        let result = freelane::agent::request_socket_version(
            &path,
            Action::TransferFolders { reconnect: false },
            13,
        );
        assert_eq!(result.is_ok(), case == 0, "case {case}: {result:?}");
        peer.join().unwrap();
    }
}
