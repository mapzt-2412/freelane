//! Versioned, bounded guest RPC. Launch requests contain an executable and literal arguments, never shell text.
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Read, Write};
pub const VERSION: u16 = 1;
pub const MAX_FRAME: usize = 64 * 1024;
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u16,
    pub id: u64,
    pub action: Action,
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Ping,
    Launch {
        executable: String,
        arguments: Vec<String>,
    },
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub version: u16,
    pub id: u64,
    pub outcome: Outcome,
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Pong { os: String, arch: String },
    Launched { pid: u32 },
    Failed { message: String },
}
impl Request {
    pub fn validate(&self) -> io::Result<()> {
        if self.version != VERSION {
            return Err(invalid("Unsupported guest protocol version"));
        }
        if self.id == 0 {
            return Err(invalid("Request ID must not be zero"));
        }
        if let Action::Launch {
            executable,
            arguments,
        } = &self.action
        {
            if executable.is_empty() || executable.len() > 4096 || executable.contains('\0') {
                return Err(invalid("Invalid executable path"));
            }
            if arguments.len() > 128 || arguments.iter().any(|v| v.len() > 4096 || v.contains('\0'))
            {
                return Err(invalid("Invalid argument list"));
            }
        }
        Ok(())
    }
}
pub fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
pub fn read_frame<T: serde::de::DeserializeOwned>(reader: &mut impl BufRead) -> io::Result<T> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take((MAX_FRAME + 1) as u64)
        .read_until(b'\n', &mut bytes)?;
    if bytes.is_empty() {
        return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
    }
    if bytes.len() > MAX_FRAME {
        return Err(invalid("Guest message exceeds 64 KiB"));
    }
    if bytes.last() != Some(&b'\n') {
        return Err(invalid("Truncated guest message"));
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid("Invalid guest message JSON"))
}
pub fn write_frame(writer: &mut impl Write, value: &impl Serialize) -> io::Result<()> {
    let mut bytes =
        serde_json::to_vec(value).map_err(|_| invalid("Cannot encode guest message"))?;
    bytes.push(b'\n');
    if bytes.len() > MAX_FRAME {
        return Err(invalid("Guest message exceeds 64 KiB"));
    }
    writer.write_all(&bytes)?;
    writer.flush()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_arguments_round_trip_without_shell_interpretation() {
        let request = Request {
            version: VERSION,
            id: 7,
            action: Action::Launch {
                executable: "C:\\Windows\\System32\\notepad.exe".into(),
                arguments: vec!["file with spaces.txt".into(), "$(echo secret) & x".into()],
            },
        };
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &request).unwrap();
        let decoded: Request = read_frame(&mut bytes.as_slice()).unwrap();
        assert_eq!(request, decoded);
        decoded.validate().unwrap();
    }
    #[test]
    fn incompatible_or_malformed_requests_are_rejected() {
        let mut request = Request {
            version: 2,
            id: 1,
            action: Action::Ping,
        };
        assert!(request.validate().is_err());
        request.version = VERSION;
        request.id = 0;
        assert!(request.validate().is_err());
        assert!(read_frame::<Request>(&mut b"{}\n".as_slice()).is_err());
        assert!(read_frame::<Request>(
            &mut b"{\"version\":1,\"id\":1,\"action\":{\"type\":\"ping\"},\"unexpected\":1}\n"
                .as_slice()
        )
        .is_err());
    }
    #[test]
    fn oversize_and_unterminated_frames_are_rejected() {
        assert!(read_frame::<Request>(&mut vec![b'x'; MAX_FRAME + 1].as_slice()).is_err());
        assert!(read_frame::<Request>(&mut b"{}".as_slice()).is_err());
        let request = Request {
            version: VERSION,
            id: 1,
            action: Action::Launch {
                executable: "x".repeat(MAX_FRAME),
                arguments: vec![],
            },
        };
        let mut bytes = Vec::new();
        assert!(write_frame(&mut bytes, &request).is_err());
        assert!(bytes.is_empty());
    }
}
