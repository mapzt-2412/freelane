use freelane::qmp::Qmp;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    thread,
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn server<F: FnOnce(UnixStream) + Send + 'static>(handler: F) -> (PathBuf, thread::JoinHandle<()>) {
    let path = std::env::temp_dir().join(format!(
        "fl-qmp-{}-{}.sock",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let listener = UnixListener::bind(&path).unwrap();
    let cleanup = path.clone();
    let thread = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        handler(stream);
        std::fs::remove_file(cleanup).unwrap();
    });
    (path, thread)
}
fn negotiate(stream: &mut BufReader<UnixStream>) {
    writeln!(stream.get_mut(), "{}", json!({"QMP":{"version":{}}})).unwrap();
    let mut line = String::new();
    stream.read_line(&mut line).unwrap();
    let request: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(request["execute"], "qmp_capabilities");
    writeln!(
        stream.get_mut(),
        "{}",
        json!({"return":{},"id":request["id"]})
    )
    .unwrap();
}
#[test]
fn interleaved_events_do_not_replace_command_response() {
    let (path, thread) = server(|socket| {
        let mut stream = BufReader::new(socket);
        negotiate(&mut stream);
        let mut line = String::new();
        stream.read_line(&mut line).unwrap();
        let request: Value = serde_json::from_str(&line).unwrap();
        writeln!(stream.get_mut(), "{}", json!({"event":"STOP"})).unwrap();
        writeln!(
            stream.get_mut(),
            "{}",
            json!({"return":{"running":false},"id":request["id"]})
        )
        .unwrap();
    });
    assert_eq!(
        Qmp::connect(&path)
            .unwrap()
            .execute("query-status", None)
            .unwrap()["running"],
        false
    );
    thread.join().unwrap();
}
#[test]
fn mismatched_response_is_rejected() {
    let (path, thread) = server(|socket| {
        let mut stream = BufReader::new(socket);
        negotiate(&mut stream);
        let mut line = String::new();
        stream.read_line(&mut line).unwrap();
        writeln!(stream.get_mut(), "{}", json!({"return":{},"id":999})).unwrap();
    });
    assert!(Qmp::connect(&path)
        .unwrap()
        .execute("cont", None)
        .unwrap_err()
        .to_string()
        .contains("ID mismatch"));
    thread.join().unwrap();
}
#[test]
fn disconnected_monitor_is_not_a_successful_command() {
    let (path, thread) = server(|socket| {
        let mut stream = BufReader::new(socket);
        negotiate(&mut stream);
        let mut line = String::new();
        stream.read_line(&mut line).unwrap();
    });
    assert!(Qmp::connect(&path).unwrap().execute("cont", None).is_err());
    thread.join().unwrap();
}
#[test]
fn oversized_greeting_is_bounded() {
    let (path, thread) = server(|mut socket| {
        let _ = socket.write_all(&vec![b'x'; 1024 * 1024 + 1]);
    });
    assert!(Qmp::connect(&path)
        .err()
        .unwrap()
        .to_string()
        .contains("exceeds"));
    thread.join().unwrap();
}

#[test]
fn connected_peer_pid_is_kernel_identity() {
    let (path, thread) = server(|socket| {
        let mut stream = BufReader::new(socket);
        negotiate(&mut stream);
        let mut line = String::new();
        stream.read_line(&mut line).unwrap();
    });
    let client = Qmp::connect(&path).unwrap();
    assert_eq!(client.peer_pid().unwrap(), std::process::id());
    drop(client);
    thread.join().unwrap();
}
