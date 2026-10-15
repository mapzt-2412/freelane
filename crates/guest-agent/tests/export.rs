#![cfg(any(target_os = "linux", windows))]
use freelane_agent_protocol::{
    export::{Operation as E, VERSION},
    Action, Outcome, Request,
};
use freelane_guest_agent::{handle, Launcher, TransferLauncher};
struct NoLaunch;
impl Launcher for NoLaunch {
    fn launch(&mut self, _: &str, _: &[String]) -> std::io::Result<u32> {
        panic!("read-only export")
    }
}
fn request(agent: &mut impl Launcher, op: E) -> Outcome {
    let response = handle(
        Request {
            version: VERSION,
            id: 1,
            action: Action::Export { operation: op },
        },
        agent,
    );
    assert_eq!(response.version, VERSION);
    response.outcome
}
#[test]
fn export_is_opt_in_bounded_sequential_and_disconnect_revokes_token() {
    let dir = tempfile::tempdir().unwrap();
    let data = vec![17; 40000];
    std::fs::write(dir.path().join("Tài liệu.txt"), &data).unwrap();
    let mut agent = TransferLauncher::new(NoLaunch, None).unwrap();
    assert!(matches!(
        request(
            &mut agent,
            E::Begin {
                name: "Tài liệu.txt".into()
            }
        ),
        Outcome::Failed { .. }
    ));
    let mut agent = agent.with_export_root(Some(dir.path())).unwrap();
    let (token, size) = match request(
        &mut agent,
        E::Begin {
            name: "Tài liệu.txt".into(),
        },
    ) {
        Outcome::ExportReady { token, size } => (token, size),
        x => panic!("{x:?}"),
    };
    assert_eq!(size, data.len() as u64);
    assert!(matches!(
        request(
            &mut agent,
            E::Read {
                token: token.clone(),
                offset: 1
            }
        ),
        Outcome::Failed { .. }
    ));
    let mut received = Vec::new();
    while received.len() < data.len() {
        match request(
            &mut agent,
            E::Read {
                token: token.clone(),
                offset: received.len() as u64,
            },
        ) {
            Outcome::ExportChunk { offset, data_hex } => {
                assert_eq!(offset, received.len() as u64);
                assert!(data_hex.len() <= 32768);
                for pair in data_hex.as_bytes().as_chunks::<2>().0.iter() {
                    received
                        .push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap());
                }
            }
            x => panic!("{x:?}"),
        }
    }
    assert_eq!(received, data);
    match request(
        &mut agent,
        E::Finish {
            token: token.clone(),
        },
    ) {
        Outcome::ExportFinished { sha256 } => {
            use sha2::{Digest, Sha256};
            assert_eq!(
                sha256,
                Sha256::digest(&data)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            );
        }
        x => panic!("{x:?}"),
    }
    let token = match request(
        &mut agent,
        E::Begin {
            name: "Tài liệu.txt".into(),
        },
    ) {
        Outcome::ExportReady { token, .. } => token,
        x => panic!("{x:?}"),
    };
    agent.disconnect();
    assert!(matches!(
        request(
            &mut agent,
            E::Read {
                token,
                offset: size
            }
        ),
        Outcome::Failed { .. }
    ));
    assert_eq!(
        std::fs::read(dir.path().join("Tài liệu.txt")).unwrap(),
        data
    );
}
#[cfg(unix)]
#[test]
fn export_refuses_escape_and_fifo_without_blocking() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("escape.txt")).unwrap();
    let fifo = std::ffi::CString::new(dir.path().join("pipe").to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    let mut agent = TransferLauncher::new(NoLaunch, None)
        .unwrap()
        .with_export_root(Some(dir.path()))
        .unwrap();
    for name in ["escape.txt", "pipe", "../secret", "sub/file"] {
        assert!(
            matches!(
                request(&mut agent, E::Begin { name: name.into() }),
                Outcome::Failed { .. }
            ),
            "{name}"
        );
    }
}
#[cfg(target_os = "linux")]
#[test]
fn large_sparse_begin_returns_before_the_host_rpc_deadline() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::File::create(dir.path().join("large.bin"))
        .unwrap()
        .set_len(8 * 1024 * 1024 * 1024)
        .unwrap();
    let mut agent = TransferLauncher::new(NoLaunch, None)
        .unwrap()
        .with_export_root(Some(dir.path()))
        .unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(request(
            &mut agent,
            E::Begin {
                name: "large.bin".into(),
            },
        ));
    });
    assert!(
        matches!(
            rx.recv_timeout(std::time::Duration::from_secs(4)),
            Ok(Outcome::ExportReady { .. })
        ),
        "Begin must not read/hash the entire file"
    );
}
#[test]
fn finish_refuses_incomplete_or_changed_source_and_releases_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.txt");
    std::fs::write(&path, b"abc").unwrap();
    let mut agent = TransferLauncher::new(NoLaunch, None)
        .unwrap()
        .with_export_root(Some(dir.path()))
        .unwrap();
    let begin = |agent: &mut TransferLauncher<NoLaunch>| match request(
        agent,
        E::Begin {
            name: "data.txt".into(),
        },
    ) {
        Outcome::ExportReady { token, .. } => token,
        x => panic!("{x:?}"),
    };
    let token = begin(&mut agent);
    assert!(matches!(
        request(&mut agent, E::Finish { token }),
        Outcome::Failed { .. }
    ));
    let token = begin(&mut agent);
    assert!(matches!(
        request(
            &mut agent,
            E::Read {
                token: token.clone(),
                offset: 0
            }
        ),
        Outcome::ExportChunk { .. }
    ));
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_len(4)
        .unwrap();
    assert!(matches!(
        request(&mut agent, E::Finish { token }),
        Outcome::Failed { .. }
    ));
    let token = begin(&mut agent);
    assert_eq!(
        request(&mut agent, E::Close { token }),
        Outcome::ExportClosed
    );
}
