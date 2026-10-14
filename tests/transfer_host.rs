use freelane::{file_transfer, Error};
use freelane_agent_protocol::{Outcome, TransferAction as T};
#[test]
fn source_change_or_cancel_never_commits_and_requests_cleanup() {
    let root = std::env::temp_dir().join(format!("freelane-transfer-host-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("Tiếng Việt.txt");
    std::fs::write(&path, b"abc").unwrap();
    let mut canceled = false;
    let result = file_transfer::send_with(
        &path,
        &|| false,
        &mut |action| match action {
            T::Begin { .. } => {
                std::fs::write(&path, b"abcd").unwrap();
                Ok(Outcome::TransferReady {
                    token: "a".repeat(32),
                })
            }
            T::Cancel { .. } => {
                canceled = true;
                Ok(Outcome::TransferCanceled)
            }
            T::Finish { .. } => panic!("changed source must not finish"),
            _ => Err(Error("unexpected chunk".into())),
        },
        &mut |_, _| {},
    );
    assert!(result.is_err());
    assert!(canceled);
    std::fs::remove_dir_all(root).unwrap();
}
