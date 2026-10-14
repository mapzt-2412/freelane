use freelane_agent_protocol::{Action, Outcome, Request, TransferAction};
use freelane_guest_agent::{handle, Launcher, TransferLauncher};
use std::io;
struct Fake;
impl Launcher for Fake {
    fn launch(&mut self, _: &str, _: &[String]) -> io::Result<u32> {
        Ok(1)
    }
}
fn call(l: &mut impl Launcher, id: u64, operation: TransferAction) -> Outcome {
    handle(
        Request {
            version: 5,
            id,
            action: Action::Transfer { operation },
        },
        l,
    )
    .outcome
}
#[test]
fn opt_in_transfer_rpc_uses_literal_names_and_rejects_replay() {
    let root = tempfile::tempdir().unwrap();
    let mut l = TransferLauncher::new(Fake, Some(root.path())).unwrap();
    let hash = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let token = match call(
        &mut l,
        1,
        TransferAction::Begin {
            name: "Tiếng Việt.txt".into(),
            size: 3,
            sha256: hash.into(),
        },
    ) {
        Outcome::TransferReady { token } => token,
        other => panic!("{other:?}"),
    };
    assert!(matches!(
        call(
            &mut l,
            2,
            TransferAction::Chunk {
                token: token.clone(),
                offset: 0,
                data_hex: "616263".into()
            }
        ),
        Outcome::TransferProgress { received: 3 }
    ));
    assert!(matches!(
        call(
            &mut l,
            3,
            TransferAction::Chunk {
                token: token.clone(),
                offset: 0,
                data_hex: "616263".into()
            }
        ),
        Outcome::Failed { .. }
    ));
    assert!(matches!(
        call(&mut l, 4, TransferAction::Finish { token }),
        Outcome::TransferComplete
    ));
    assert_eq!(
        std::fs::read(root.path().join("Tiếng Việt.txt")).unwrap(),
        b"abc"
    );
    let mut disabled = TransferLauncher::new(Fake, None).unwrap();
    assert!(matches!(
        call(
            &mut disabled,
            1,
            TransferAction::Begin {
                name: "a".into(),
                size: 0,
                sha256: hash.into()
            }
        ),
        Outcome::Failed { .. }
    ));
}
#[test]
fn document_open_requires_completed_receipt_and_unchanged_safe_file() {
    use std::sync::{Arc, Mutex};
    struct Documents(Arc<Mutex<Vec<std::path::PathBuf>>>);
    impl Launcher for Documents {
        fn launch(&mut self, _: &str, _: &[String]) -> io::Result<u32> {
            Err(io::ErrorKind::Unsupported.into())
        }
        fn open_document(&mut self, path: &std::path::Path) -> io::Result<()> {
            self.0.lock().unwrap().push(path.into());
            Ok(())
        }
    }
    let root = tempfile::tempdir().unwrap();
    let opened = Arc::new(Mutex::new(Vec::new()));
    let mut l = TransferLauncher::new(Documents(opened.clone()), Some(root.path())).unwrap();
    let hash = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let token = match call(
        &mut l,
        1,
        TransferAction::Begin {
            name: "a.txt".into(),
            size: 3,
            sha256: hash.into(),
        },
    ) {
        Outcome::TransferReady { token } => token,
        _ => panic!(),
    };
    assert!(matches!(
        call(
            &mut l,
            2,
            TransferAction::Open {
                token: token.clone()
            }
        ),
        Outcome::Failed { .. }
    ));
    call(
        &mut l,
        3,
        TransferAction::Chunk {
            token: token.clone(),
            offset: 0,
            data_hex: "616263".into(),
        },
    );
    call(
        &mut l,
        4,
        TransferAction::Finish {
            token: token.clone(),
        },
    );
    assert_eq!(
        call(
            &mut l,
            5,
            TransferAction::Open {
                token: token.clone()
            }
        ),
        Outcome::DocumentOpened
    );
    {
        let paths = opened.lock().unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(std::fs::read(&paths[0]).unwrap(), b"abc");
    }
    std::fs::write(root.path().join("a.txt"), b"xyz").unwrap();
    assert!(matches!(
        call(&mut l, 6, TransferAction::Open { token }),
        Outcome::Failed { .. }
    ));
    assert_eq!(opened.lock().unwrap().len(), 1);
}
#[test]
fn executable_script_and_unknown_associations_are_never_opened() {
    struct NeverOpen;
    impl Launcher for NeverOpen {
        fn launch(&mut self, _: &str, _: &[String]) -> io::Result<u32> {
            panic!("no executable launch")
        }
        fn open_document(&mut self, _: &std::path::Path) -> io::Result<()> {
            panic!("unsafe association must never launch")
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut l = TransferLauncher::new(NeverOpen, Some(root.path())).unwrap();
    for extension in [
        "exe", "ps1", "bat", "cmd", "sh", "desktop", "lnk", "url", "msi", "vbs", "html", "svg",
        "docm",
    ] {
        let token = match call(
            &mut l,
            1,
            TransferAction::Begin {
                name: format!("a.{extension}"),
                size: 3,
                sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
            },
        ) {
            Outcome::TransferReady { token } => token,
            _ => panic!(),
        };
        call(
            &mut l,
            2,
            TransferAction::Chunk {
                token: token.clone(),
                offset: 0,
                data_hex: "616263".into(),
            },
        );
        assert_eq!(
            call(
                &mut l,
                3,
                TransferAction::Finish {
                    token: token.clone()
                }
            ),
            Outcome::TransferComplete
        );
        assert!(
            matches!(
                call(&mut l, 4, TransferAction::Open { token }),
                Outcome::Failed { .. }
            ),
            "{extension}"
        );
    }
}
