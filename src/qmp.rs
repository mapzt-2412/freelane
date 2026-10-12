//! Bounded, synchronous QMP client. Each connection negotiates capabilities before commands.
use crate::{Error, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

const MAX_LINE: usize = 1024 * 1024;
pub struct Qmp {
    reader: BufReader<UnixStream>,
    next_id: u64,
}
impl Qmp {
    pub fn connect(path: &Path) -> Result<Self> {
        let stream =
            UnixStream::connect(path).map_err(|e| Error(format!("QMP {}: {e}", path.display())))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .map_err(io_error)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(3)))
            .map_err(io_error)?;
        let mut client = Self {
            reader: BufReader::new(stream),
            next_id: 0,
        };
        let greeting = client.read_message(Instant::now() + Duration::from_secs(3))?;
        if !greeting["QMP"].is_object() {
            return Err(Error("Invalid QMP greeting".into()));
        }
        client.execute("qmp_capabilities", None)?;
        Ok(client)
    }
    fn read_message(&mut self, deadline: Instant) -> Result<Value> {
        let mut bytes = Vec::new();
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| Error("QMP message deadline exceeded".into()))?;
            self.reader
                .get_ref()
                .set_read_timeout(Some(remaining))
                .map_err(io_error)?;
            let chunk = self.reader.fill_buf().map_err(io_error)?;
            if chunk.is_empty() {
                return Err(Error("QMP connection closed".into()));
            }
            let newline = chunk.iter().position(|b| *b == b'\n');
            let count = newline.map_or(chunk.len(), |index| index + 1);
            if bytes.len() + count > MAX_LINE {
                return Err(Error("QMP message exceeds 1 MiB".into()));
            }
            bytes.extend_from_slice(&chunk[..count]);
            self.reader.consume(count);
            if newline.is_some() {
                break;
            }
        }
        serde_json::from_slice(&bytes).map_err(|e| Error(format!("Invalid QMP JSON: {e}")))
    }
    pub fn execute(&mut self, command: &str, arguments: Option<Value>) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        let mut request = json!({"execute": command, "id": id});
        if let Some(args) = arguments {
            request["arguments"] = args;
        }
        let stream = self.reader.get_mut();
        serde_json::to_writer(&mut *stream, &request).map_err(|e| Error(e.to_string()))?;
        stream.write_all(b"\n").map_err(io_error)?;
        stream.flush().map_err(io_error)?;
        // A deadline bounds event floods as well as socket inactivity.
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            let remaining = deadline
                .checked_duration_since(std::time::Instant::now())
                .ok_or_else(|| Error("QMP command deadline exceeded".into()))?;
            self.reader
                .get_ref()
                .set_read_timeout(Some(remaining))
                .map_err(io_error)?;
            let message = self.read_message(deadline)?;
            if message.get("event").is_some() {
                continue;
            }
            if message.get("id").and_then(Value::as_u64) != Some(id) {
                return Err(Error("QMP response ID mismatch".into()));
            }
            if let Some(error) = message.get("error") {
                return Err(Error(format!("QMP {command}: {error}")));
            }
            return message
                .get("return")
                .cloned()
                .ok_or_else(|| Error("QMP response missing return value".into()));
        }
    }
}
fn io_error(e: std::io::Error) -> Error {
    Error(format!("QMP I/O: {e}"))
}
