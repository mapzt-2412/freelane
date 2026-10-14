//! Local host-to-guest client. The private QEMU socket carries versioned requests, never shell commands.
use crate::{qemu, Error, Result};
use freelane_agent_protocol::{Action, Request, Response, MAX_FRAME, VERSION};
use std::{
    io::{BufRead, BufReader},
    os::fd::AsRawFd,
    os::unix::net::UnixStream,
    path::Path,
    time::{Duration, Instant},
};
pub fn request(config: &Path, action: Action) -> Result<Response> {
    request_vm(config, action, VERSION)
}
pub(crate) fn request_vm(config: &Path, action: Action, version: u16) -> Result<Response> {
    request_vm_cancellable(config, action, version, &|| false)
}
fn cancellation(cancelled: &impl Fn() -> bool) -> Result<()> {
    if cancelled() {
        Err(Error("Clipboard sharing disabled; no request sent".into()))
    } else {
        Ok(())
    }
}
fn request_vm_cancellable(
    config: &Path,
    action: Action,
    version: u16,
    cancelled: &impl Fn() -> bool,
) -> Result<Response> {
    cancellation(cancelled)?;
    use fs2::FileExt;
    use std::{
        fs::{self, OpenOptions},
        os::unix::fs::{MetadataExt, OpenOptionsExt},
    };
    let directory = qemu::runtime_dir(config)?;
    let metadata = fs::symlink_metadata(&directory)
        .map_err(|e| Error(format!("Guest agent unavailable: {e}")))?;
    if !metadata.is_dir()
        || metadata.mode() & 0o077 != 0
        || metadata.uid()
            != fs::metadata(config)
                .map_err(|e| Error(e.to_string()))?
                .uid()
    {
        return Err(Error(
            "Guest agent runtime must be private and owned by VM owner".into(),
        ));
    }
    // The serial port has one reader. Independent Dock viewers and MCP clients
    // must lease it across the entire request/response, not only the write.
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(directory.join("agent-client.lock"))
        .map_err(|e| Error(e.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        cancellation(cancelled)?;
        match lock.try_lock_exclusive() {
            Ok(()) => break,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20))
            }
            Err(e) => {
                return Err(Error(format!(
                    "Guest agent channel busy; no request sent: {e}"
                )))
            }
        }
    }
    cancellation(cancelled)?;
    request_socket_cancellable(&directory.join("agent.sock"), action, version, cancelled)
}
pub fn request_socket(socket: &Path, action: Action) -> Result<Response> {
    request_socket_version(socket, action, VERSION)
}
pub fn window_request(config: &Path, action: Action) -> Result<Response> {
    request_vm(config, action, freelane_agent_protocol::WINDOW_VERSION)
}
pub fn request_socket_version(socket: &Path, action: Action, version: u16) -> Result<Response> {
    request_socket_cancellable(socket, action, version, &|| false)
}
fn request_socket_cancellable(
    socket: &Path,
    action: Action,
    version: u16,
    cancelled: &impl Fn() -> bool,
) -> Result<Response> {
    cancellation(cancelled)?;
    // Independent CLI invocations share a persistent serial channel: never reuse ID 1.
    let id = loop {
        let id = getrandom::u64()
            .map_err(|error| Error(format!("Cannot generate request ID: {error}")))?;
        if id != 0 {
            break id;
        }
    };
    let request = Request {
        version,
        id,
        action,
    };
    request.validate().map_err(|e| Error(e.to_string()))?;
    let mut stream = UnixStream::connect(socket).map_err(|e| {
        Error(format!(
            "Guest agent unavailable: {e}; enable guest_agent and start the agent in the guest session"
        ))
    })?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| Error(e.to_string()))?;
    cancellation(cancelled)?;
    freelane_agent_protocol::write_frame(&mut stream, &request)
        .map_err(|e| Error(e.to_string()))?;
    let mut reader = BufReader::new(stream);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut bytes = Vec::new();
    loop {
        if Instant::now() >= deadline {
            return Err(Error("Guest agent response deadline exceeded".into()));
        }
        if reader.buffer().is_empty() {
            crate::socket_wait::readable(reader.get_ref().as_raw_fd(), deadline).map_err(|e| {
                Error(format!(
                    "Guest agent response deadline or socket error: {e}"
                ))
            })?;
        }
        let chunk = reader.fill_buf().map_err(|error| {
            if matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ) {
                Error("Guest agent response deadline exceeded".into())
            } else {
                Error(format!("Guest agent response: {error}"))
            }
        })?;
        if chunk.is_empty() {
            return Err(Error("Guest agent disconnected before replying".into()));
        }
        let newline = chunk.iter().position(|b| *b == b'\n');
        let count = newline.map_or(chunk.len(), |n| n + 1);
        if bytes.len() + count > MAX_FRAME {
            return Err(Error("Guest agent response exceeds 64 KiB".into()));
        }
        bytes.extend_from_slice(&chunk[..count]);
        reader.consume(count);
        if newline.is_some() {
            break;
        }
    }
    let response: Response =
        serde_json::from_slice(&bytes).map_err(|_| Error("Invalid guest agent response".into()))?;
    if response.version != version || response.id != request.id {
        return Err(Error("Guest agent version/response ID mismatch".into()));
    }
    if let freelane_agent_protocol::Outcome::Failed { message } = &response.outcome {
        return Err(Error(
            if version == freelane_agent_protocol::CLIPBOARD_VERSION {
                "Guest clipboard unavailable or refused".into()
            } else {
                format!("Guest agent: {message}")
            },
        ));
    }
    let expected = matches!(
        (&request.action, &response.outcome),
        (
            Action::ClipboardGet,
            freelane_agent_protocol::Outcome::Clipboard { .. }
        ) | (
            Action::ClipboardSet { .. },
            freelane_agent_protocol::Outcome::ClipboardAccepted { .. }
        ) | (Action::Ping, freelane_agent_protocol::Outcome::Pong { .. })
            | (
                Action::Ping,
                freelane_agent_protocol::Outcome::Capabilities { .. }
            )
            | (
                Action::Ping,
                freelane_agent_protocol::Outcome::AgentInfo { .. }
            )
            | (
                Action::Windows { .. },
                freelane_agent_protocol::Outcome::Windows { .. }
            )
            | (
                Action::CaptureWindow { .. },
                freelane_agent_protocol::Outcome::WindowFrame { .. }
            )
            | (
                Action::WindowClick { .. } | Action::WindowText { .. } | Action::WindowKey { .. },
                freelane_agent_protocol::Outcome::WindowInputAccepted
            )
            | (
                Action::Launch { .. },
                freelane_agent_protocol::Outcome::Launched { .. }
            )
    );
    let expected = expected
        && match (&request.action, &response.outcome) {
            (Action::ClipboardGet, freelane_agent_protocol::Outcome::Clipboard { snapshot }) => {
                snapshot.validate().is_ok()
            }
            (
                Action::ClipboardSet {
                    source, sequence, ..
                },
                freelane_agent_protocol::Outcome::ClipboardAccepted {
                    source: actual,
                    sequence: seq,
                },
            ) => source == actual && sequence == seq,
            (Action::Ping, freelane_agent_protocol::Outcome::AgentInfo { info }) => {
                version == freelane_agent_protocol::CAPABILITY_VERSION && info.validate().is_ok()
            }
            (Action::Ping, freelane_agent_protocol::Outcome::Pong { .. }) => version == VERSION,
            (Action::Ping, freelane_agent_protocol::Outcome::Capabilities { .. }) => {
                version == freelane_agent_protocol::WINDOW_VERSION
            }
            (
                Action::CaptureWindow { target },
                freelane_agent_protocol::Outcome::WindowFrame {
                    target: actual,
                    width,
                    height,
                    jpeg_hex,
                },
            ) => {
                target == actual
                    && *width > 0
                    && *height > 0
                    && *width <= 640
                    && *height <= 480
                    && jpeg_hex.len() <= 56000
                    && jpeg_hex.len() % 2 == 0
                    && jpeg_hex.bytes().all(|b| b.is_ascii_hexdigit())
            }
            (Action::Windows { pid }, freelane_agent_protocol::Outcome::Windows { windows }) => {
                windows.len() <= 128
                    && windows.iter().all(|w| {
                        w.target.pid == *pid
                            && w.target.handle != 0
                            && w.target.created != 0
                            && w.title.len() <= 2048
                    })
            }
            _ => true,
        };
    if !expected {
        return Err(Error(
            "Guest agent response does not match requested action".into(),
        ));
    }
    Ok(response)
}

pub fn clipboard_request(config: &Path, action: Action) -> Result<Response> {
    if !matches!(action, Action::ClipboardGet | Action::ClipboardSet { .. }) {
        return Err(Error("Invalid clipboard action".into()));
    }
    request_vm(config, action, freelane_agent_protocol::CLIPBOARD_VERSION)
}

pub fn clipboard_request_cancellable(
    config: &Path,
    action: Action,
    cancelled: &impl Fn() -> bool,
) -> Result<Response> {
    if !matches!(action, Action::ClipboardGet | Action::ClipboardSet { .. }) {
        return Err(Error("Invalid clipboard action".into()));
    }
    request_vm_cancellable(
        config,
        action,
        freelane_agent_protocol::CLIPBOARD_VERSION,
        cancelled,
    )
}
