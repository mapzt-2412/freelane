use freelane::qemu;
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn command(stream: &mut BufReader<UnixStream>, reply: Value) -> Value {
    let mut line = String::new();
    assert_ne!(stream.read_line(&mut line).unwrap(), 0);
    let request: Value = serde_json::from_str(&line).unwrap();
    writeln!(
        stream.get_mut(),
        "{}",
        json!({"return":reply,"id":request["id"]})
    )
    .unwrap();
    request
}

fn accept(listener: &UnixListener) -> UnixStream {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match listener.accept() {
            Ok((socket, _)) => {
                // macOS may inherit O_NONBLOCK from the listening socket.
                socket.set_nonblocking(false).unwrap();
                return socket;
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "expected input connection");
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("{error}"),
        }
    }
}

fn mouse_session(running: bool, lose_press_ack: bool) -> (freelane::Result<()>, Vec<Value>) {
    let base = std::env::temp_dir().join(format!(
        "fl-input-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&base).unwrap();
    let config = base.join("vm.toml");
    // RPC controls identify the canonical configuration path; no VM is booted here.
    fs::write(&config, "").unwrap();
    let runtime = qemu::runtime_dir(&config).unwrap();
    fs::create_dir(&runtime).unwrap();
    let listener = UnixListener::bind(runtime.join("qmp.sock")).unwrap();
    listener.set_nonblocking(true).unwrap();
    let worker = thread::spawn(move || {
        let socket = accept(&listener);
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut stream = BufReader::new(socket);
        writeln!(stream.get_mut(), "{}", json!({"QMP":{}})).unwrap();
        assert_eq!(
            command(&mut stream, json!({}))["execute"],
            "qmp_capabilities"
        );
        assert_eq!(
            command(&mut stream, json!({"running":running}))["execute"],
            "query-status"
        );
        if running {
            if lose_press_ack {
                let mut line = String::new();
                stream.read_line(&mut line).unwrap();
                let pressed: Value = serde_json::from_str(&line).unwrap();
                // The press may have reached the VM, but its acknowledgment was lost.
                drop(stream);
                let socket = accept(&listener);
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut recovery = BufReader::new(socket);
                writeln!(recovery.get_mut(), "{}", json!({"QMP":{}})).unwrap();
                assert_eq!(
                    command(&mut recovery, json!({}))["execute"],
                    "qmp_capabilities"
                );
                return vec![pressed, command(&mut recovery, json!({}))];
            }
            vec![
                command(&mut stream, json!({})),
                command(&mut stream, json!({})),
            ]
        } else {
            let mut line = String::new();
            assert_eq!(
                stream.read_line(&mut line).unwrap(),
                0,
                "paused guest must receive no input"
            );
            vec![]
        }
    });
    let result = qemu::click(&config, 0, 32767);
    let events = worker.join().unwrap();
    fs::remove_dir_all(runtime).unwrap();
    fs::remove_dir_all(base).unwrap();
    (result, events)
}

#[test]
fn mouse_input_sends_coordinates_then_releases_button() {
    let (result, requests) = mouse_session(true, false);
    result.unwrap();
    assert_eq!(requests[0]["execute"], "input-send-event");
    assert_eq!(
        requests[0]["arguments"]["events"],
        json!([
            {"type":"abs","data":{"axis":"x","value":0}},
            {"type":"abs","data":{"axis":"y","value":32767}},
            {"type":"btn","data":{"button":"left","down":true}}
        ])
    );
    assert_eq!(requests[1]["execute"], "input-send-event");
    assert_eq!(
        requests[1]["arguments"]["events"],
        json!([
            {"type":"btn","data":{"button":"left","down":false}}
        ])
    );
}

#[test]
fn paused_guest_receives_no_mouse_button_events() {
    let (result, requests) = mouse_session(false, false);
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Guest must be running"));
    assert!(requests.is_empty());
}

#[test]
fn lost_mouse_acknowledgment_releases_without_repeating_click() {
    let (result, requests) = mouse_session(true, true);
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Delivery may be unknown"));
    assert_eq!(requests[0]["arguments"]["events"][2]["data"]["down"], true);
    assert_eq!(
        requests[1]["arguments"]["events"],
        json!([
            {"type":"btn","data":{"button":"left","down":false}}
        ])
    );
}
