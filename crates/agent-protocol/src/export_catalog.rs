//! Explicit, bounded inventory of the configured guest outbox.
use serde::{Deserialize, Serialize};
use std::io;
pub const VERSION: u16 = 12;
pub const MAX_SCAN: usize = 512;
pub const MAX_FILES: usize = 128;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub name: String,
    pub size: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub files: Vec<Entry>,
    pub limited: bool,
}
impl Page {
    pub fn validate(&self) -> io::Result<()> {
        if self.files.len() > MAX_FILES
            || self
                .files
                .iter()
                .any(|f| !crate::export::valid_name(&f.name) || f.size > crate::export::MAX_SIZE)
            || self.files.windows(2).any(|w| w[0].name >= w[1].name)
        {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(())
    }
}
