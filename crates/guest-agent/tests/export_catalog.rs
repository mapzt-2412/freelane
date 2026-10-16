#![cfg(any(target_os = "linux", windows))]
use freelane_agent_protocol::{
    export_catalog::{MAX_FILES, MAX_SCAN},
    Action, Outcome, Request,
};
use freelane_guest_agent::{handle, Launcher, TransferLauncher};
struct NoLaunch;
impl Launcher for NoLaunch {
    fn launch(&mut self, _: &str, _: &[String]) -> std::io::Result<u32> {
        panic!("listing never launches")
    }
}
fn list(agent: &mut impl Launcher) -> Outcome {
    let reply = handle(
        Request {
            version: 12,
            id: 72,
            action: Action::ExportCatalog,
        },
        agent,
    );
    assert_eq!(reply.version, 12);
    reply.outcome
}
#[test]
fn inventory_is_opt_in_sorted_and_only_lists_regular_portable_files() {
    let root = tempfile::tempdir().unwrap();
    let mut agent = TransferLauncher::new(NoLaunch, None).unwrap();
    assert!(matches!(list(&mut agent), Outcome::Failed { .. }));
    std::fs::write(root.path().join("Z.txt"), b"abc").unwrap();
    std::fs::write(root.path().join("Bản dịch.txt"), b"12").unwrap();
    std::fs::write(root.path().join(".private"), b"hide").unwrap();
    std::fs::create_dir(root.path().join("folder")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("Z.txt", root.path().join("link.txt")).unwrap();
    }
    let mut agent = agent.with_export_root(Some(root.path())).unwrap();
    match list(&mut agent) {
        Outcome::ExportCatalog { page } => {
            page.validate().unwrap();
            assert!(!page.limited);
            assert_eq!(
                page.files
                    .iter()
                    .map(|f| (&*f.name, f.size))
                    .collect::<Vec<_>>(),
                vec![("Bản dịch.txt", 2), ("Z.txt", 3)]
            );
        }
        x => panic!("{x:?}"),
    }
    assert_eq!(std::fs::read(root.path().join("Z.txt")).unwrap(), b"abc");
}
#[test]
fn inventory_is_bounded_and_reports_partial_results() {
    let root = tempfile::tempdir().unwrap();
    for n in 0..MAX_SCAN + 2 {
        std::fs::write(root.path().join(format!("f{n:04}.txt")), b"a").unwrap();
    }
    let mut agent = TransferLauncher::new(NoLaunch, None)
        .unwrap()
        .with_export_root(Some(root.path()))
        .unwrap();
    match list(&mut agent) {
        Outcome::ExportCatalog { page } => {
            page.validate().unwrap();
            assert!(page.limited);
            assert_eq!(page.files.len(), MAX_FILES);
        }
        x => panic!("{x:?}"),
    }
}
#[cfg(unix)]
#[test]
fn replaced_directory_does_not_redirect_listing() {
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("outbox");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("selected.txt"), b"a").unwrap();
    let mut agent = TransferLauncher::new(NoLaunch, None)
        .unwrap()
        .with_export_root(Some(&root))
        .unwrap();
    std::fs::rename(&root, parent.path().join("original")).unwrap();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("replacement.txt"), b"b").unwrap();
    match list(&mut agent) {
        Outcome::ExportCatalog { page } => assert_eq!(page.files[0].name, "selected.txt"),
        x => panic!("{x:?}"),
    }
}
