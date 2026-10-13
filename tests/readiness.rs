use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    thread,
};
fn fixture(label: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("fl-readiness-{label}-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    let cfg = root.join("vm.toml");
    fs::write(
        &cfg,
        "name='fixture'\ncpus=2\nmemory_mib=4096\ndisk='unused.qcow2'\n",
    )
    .unwrap();
    (root, cfg)
}
#[test]
fn absent_backend_is_unknown_and_never_installed() {
    let (root, cfg) = fixture("absent");
    let original = fs::read(&cfg).unwrap();
    let runtime = freelane::qemu::runtime_dir(&cfg).unwrap();
    let report = freelane::readiness::inspect(&cfg).unwrap();
    assert_eq!(report.backend, "unknown");
    assert_eq!(report.agent, "disabled");
    assert_eq!(report.installation_complete, None);
    assert!(!runtime.exists());
    assert_eq!(fs::read(cfg).unwrap(), original);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn actual_qmp_running_is_not_installation_success() {
    let (root, cfg) = fixture("qmp");
    let runtime = freelane::qemu::runtime_dir(&cfg).unwrap();
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let listener = UnixListener::bind(runtime.join("qmp.sock")).unwrap();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(socket.try_clone().unwrap());
        socket.write_all(b"{\"QMP\":{}}\n").unwrap();
        for expected in ["qmp_capabilities", "query-status"] {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let request: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(request["execute"], expected);
            let value = if expected == "query-status" {
                serde_json::json!({"status":"running","running":true})
            } else {
                serde_json::json!({})
            };
            let reply = serde_json::json!({"return":value,"id":request["id"]});
            writeln!(socket, "{reply}").unwrap();
        }
    });
    let report = freelane::readiness::inspect(&cfg).unwrap();
    assert_eq!(report.backend, "running");
    assert_eq!(report.installation_complete, None);
    server.join().unwrap();
    fs::remove_dir_all(runtime).unwrap();
    fs::remove_dir_all(root).unwrap();
}
