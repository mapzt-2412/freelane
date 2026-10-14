//! Opt-in, bounded storage transactions. Publication never replaces an existing entry.
use cap_std::fs::{Dir, File};
#[path = "transfer_storage.rs"]
mod storage;
use sha2::{Digest, Sha256};
use std::{
    io::{self, Write},
    path::Path,
};
pub const MAX_CHUNK: usize = 16384;
pub const MAX_SIZE: u64 = 64 * 1024 * 1024 * 1024;
pub struct Transfer {
    root: Dir,
    name: String,
    file: Option<File>,
    failed: bool,
    size: u64,
    received: u64,
    expected: String,
    hash: Sha256,
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "File transfer metadata or state refused",
    )
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
impl Transfer {
    pub fn begin(root: &Path, name: &str, size: u64, hash: &str) -> io::Result<Self> {
        if !valid_name(name)
            || size > MAX_SIZE
            || hash.len() != 64
            || !hash.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(invalid());
        }
        let root = Dir::open_ambient_dir(root, cap_std::ambient_authority())?;
        Self::begin_dir(root, name, size, hash)
    }
    pub fn begin_dir(root: Dir, name: &str, size: u64, hash: &str) -> io::Result<Self> {
        if !valid_name(name)
            || size > MAX_SIZE
            || hash.len() != 64
            || !hash.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(invalid());
        }
        let file = storage::create(&root)?;
        Ok(Self {
            root,
            name: name.into(),
            file: Some(file),
            failed: false,
            size,
            received: 0,
            expected: hash.to_ascii_lowercase(),
            hash: Sha256::new(),
        })
    }
    pub fn write(&mut self, offset: u64, data: &[u8]) -> io::Result<()> {
        if self.failed
            || offset != self.received
            || data.is_empty()
            || data.len() > MAX_CHUNK
            || data.len() as u64 > self.size - self.received
        {
            return Err(invalid());
        }
        // A partial I/O failure invalidates the session: caller must discard it.
        if let Err(error) = self.file.as_mut().unwrap().write_all(data) {
            self.failed = true;
            return Err(error);
        }
        self.hash.update(data);
        self.received += data.len() as u64;
        Ok(())
    }
    pub fn receipt(&self) -> (String, u64, String) {
        (self.name.clone(), self.size, self.expected.clone())
    }
    pub fn received(&self) -> u64 {
        self.received
    }
    pub fn finish(self) -> io::Result<()> {
        let actual: String = self
            .hash
            .clone()
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if self.failed || self.received != self.size || actual != self.expected {
            return Err(invalid());
        }
        self.file.as_ref().unwrap().sync_all()?;
        storage::publish(self.file.as_ref().unwrap(), &self.root, &self.name)?;
        Ok(())
    }
}
impl Drop for Transfer {
    fn drop(&mut self) {
        if let Some(file) = &self.file {
            storage::cleanup(file);
        }
        self.file.take();
    }
}
