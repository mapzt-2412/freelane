use freelane_agent_protocol::{
    export_catalog::{Entry, Page},
    Action, Outcome, Response,
};
use std::{io::BufReader, os::unix::net::UnixListener};
#[test]
fn host_accepts_only_matching_bounded_outbox_inventory_replies() {
    for case in 0..5 {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("agent.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let peer = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request: freelane_agent_protocol::Request = freelane_agent_protocol::read_frame(
                &mut BufReader::new(stream.try_clone().unwrap()),
            )
            .unwrap();
            let entry = Entry {
                name: if case == 2 {
                    "../outside".into()
                } else {
                    "result.txt".into()
                },
                size: 3,
            };
            let files = if case == 1 {
                vec![entry.clone(), entry]
            } else {
                vec![entry]
            };
            freelane_agent_protocol::write_frame(
                &mut stream,
                &Response {
                    version: if case == 3 { 11 } else { 12 },
                    id: request.id,
                    outcome: if case == 4 {
                        Outcome::ExportClosed
                    } else {
                        Outcome::ExportCatalog {
                            page: Page {
                                files,
                                limited: false,
                            },
                        }
                    },
                },
            )
            .unwrap();
        });
        let result = freelane::agent::request_socket_version(&path, Action::ExportCatalog, 12);
        assert_eq!(result.is_ok(), case == 0, "case {case}: {result:?}");
        peer.join().unwrap();
    }
}
