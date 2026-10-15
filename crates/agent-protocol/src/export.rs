//! Explicit guest outbox reads; no arbitrary paths or directory enumeration.
use serde::{Deserialize, Serialize};
use std::io;
pub const VERSION: u16 = 11;
pub const MAX_SIZE: u64 = 64 * 1024 * 1024 * 1024;
pub const MAX_CHUNK: usize = 16384;
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    Begin { name: String },
    Read { token: String, offset: u64 },
    Finish { token: String },
    Close { token: String },
}
pub fn hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit())
}
pub fn valid_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    !name.is_empty()
        && name.len() <= 240
        && !name.starts_with('.')
        && !name.ends_with([' ', '.'])
        && !name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        && !["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&stem.as_str())
        && !((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem[3..].chars().count() == 1
            && stem[3..]
                .chars()
                .next()
                .is_some_and(|c| "123456789¹²³".contains(c)))
}
impl Operation {
    pub fn validate(&self) -> io::Result<()> {
        let valid = match self {
            Self::Begin { name } => valid_name(name),
            Self::Read { token, offset } => hex(token, 32) && *offset <= MAX_SIZE,
            Self::Finish { token } | Self::Close { token } => hex(token, 32),
        };
        if valid {
            Ok(())
        } else {
            Err(io::ErrorKind::InvalidInput.into())
        }
    }
}
