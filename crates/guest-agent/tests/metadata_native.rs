#![cfg(any(windows, target_os = "linux"))]
use freelane_agent_protocol::{
    Action, Outcome, Request, Response, CATALOG_VERSION, SHARE_STATUS_VERSION,
};
use std::{
    io::Write,
    process::{Command, Stdio},
};
#[test]
fn native_stdio_catalog_and_mount_status_are_bounded_read_only_metadata() {
    let mut agent = Command::new(env!("CARGO_BIN_EXE_freelane-guest-agent"))
        .arg("--stdio")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = Vec::new();
    for (id, version, action) in [
        (1, CATALOG_VERSION, Action::AppCatalog { cursor: 0 }),
        (2, SHARE_STATUS_VERSION, Action::ShareStatus),
    ] {
        freelane_agent_protocol::write_frame(
            &mut input,
            &Request {
                version,
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
    let catalog: Response = freelane_agent_protocol::read_frame(&mut reader).unwrap();
    assert_eq!((catalog.id, catalog.version), (1, CATALOG_VERSION));
    let Outcome::AppCatalog { page } = catalog.outcome else {
        panic!("Catalog unavailable")
    };
    assert!(page.apps.len() <= 12);
    for entry in page.apps {
        entry.validate().unwrap();
    }
    #[cfg(windows)]
    assert!(
        !freelane_guest_agent::catalog::entries().unwrap().is_empty(),
        "Windows system applications must be discovered"
    );
    let shares: Response = freelane_agent_protocol::read_frame(&mut reader).unwrap();
    assert_eq!((shares.id, shares.version), (2, SHARE_STATUS_VERSION));
    #[cfg(target_os = "linux")]
    {
        let Outcome::ShareMounts { mounts } = shares.outcome else {
            panic!("Mount metadata unavailable")
        };
        assert!(mounts.len() <= 16);
        for mount in mounts {
            mount.validate().unwrap();
        }
    }
    #[cfg(windows)]
    assert!(matches!(shares.outcome, Outcome::Failed { .. }));
    assert!(reader.is_empty());
}
