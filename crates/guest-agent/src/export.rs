//! One read-only outbox session, rooted in a pinned directory capability.
use cap_std::fs::{Dir, File, OpenOptions};
use freelane_agent_protocol::{
    export::{self, Operation as E},
    Outcome,
};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read},
    path::Path,
    time::{Duration, Instant},
};
struct Session {
    token: String,
    file: File,
    size: u64,
    offset: u64,
    touched: Instant,
    modified: cap_std::time::SystemTime,
    hash: Sha256,
}
#[derive(Default)]
pub struct Outbox {
    root: Option<Dir>,
    active: Option<Session>,
}
impl Outbox {
    pub fn new(root: Option<&Path>) -> io::Result<Self> {
        Ok(Self {
            root: root
                .map(|p| Dir::open_ambient_dir(p, cap_std::ambient_authority()))
                .transpose()?,
            active: None,
        })
    }
    pub fn enabled(&self) -> bool {
        self.root.is_some()
    }
    pub fn disconnect(&mut self) {
        self.active = None;
    }
    pub fn catalog(&self) -> io::Result<freelane_agent_protocol::export_catalog::Page> {
        use freelane_agent_protocol::export_catalog::{Entry, Page, MAX_FILES, MAX_SCAN};
        let root = self.root.as_ref().ok_or(io::ErrorKind::Unsupported)?;
        let mut files = std::collections::BTreeMap::new();
        let mut limited = false;
        for (index, entry) in root.entries()?.enumerate() {
            if index >= MAX_SCAN {
                limited = true;
                break;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    limited = true;
                    continue;
                }
            };
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            if !export::valid_name(&name) {
                continue;
            }
            let metadata = match root.symlink_metadata(&name) {
                Ok(m) => m,
                Err(_) => {
                    limited = true;
                    continue;
                }
            };
            if !metadata.is_file() || metadata.len() > export::MAX_SIZE {
                continue;
            }
            if files.len() >= MAX_FILES {
                limited = true;
                continue;
            }
            files.insert(name, metadata.len());
        }
        Ok(Page {
            files: files
                .into_iter()
                .map(|(name, size)| Entry { name, size })
                .collect(),
            limited,
        })
    }
    pub(crate) fn busy(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|s| s.touched.elapsed() <= Duration::from_secs(300))
    }
    pub(crate) fn expire(&mut self) {
        if self
            .active
            .as_ref()
            .is_some_and(|s| s.touched.elapsed() > Duration::from_secs(300))
        {
            self.active = None;
        }
    }
    pub fn action(&mut self, operation: &E) -> io::Result<Outcome> {
        operation.validate()?;
        self.expire();
        let root = self.root.as_ref().ok_or(io::ErrorKind::Unsupported)?;
        match operation {
            E::Begin { name } => {
                if self.active.is_some() {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                let mut options = OpenOptions::new();
                options.read(true);
                #[cfg(unix)]
                {
                    use cap_std::fs::OpenOptionsExt;
                    options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
                }
                let file = root.open_with(name, &options)?;
                let before = file.metadata()?;
                if !before.is_file() || before.len() > export::MAX_SIZE {
                    return Err(io::ErrorKind::InvalidInput.into());
                }
                let mut random = [0; 16];
                getrandom::fill(&mut random).map_err(|_| io::ErrorKind::Other)?;
                let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
                self.active = Some(Session {
                    token: token.clone(),
                    file,
                    size: before.len(),
                    offset: 0,
                    touched: Instant::now(),
                    modified: before.modified()?,
                    hash: Sha256::new(),
                });
                Ok(Outcome::ExportReady {
                    token,
                    size: before.len(),
                })
            }
            E::Read { token, offset } => {
                let s = self
                    .active
                    .as_mut()
                    .filter(|s| &s.token == token)
                    .ok_or(io::ErrorKind::NotFound)?;
                if *offset != s.offset || *offset >= s.size {
                    return Err(io::ErrorKind::InvalidInput.into());
                }
                let mut buffer = [0; export::MAX_CHUNK];
                let length = (s.size - s.offset).min(export::MAX_CHUNK as u64) as usize;
                if let Err(e) = s.file.read_exact(&mut buffer[..length]) {
                    self.active = None;
                    return Err(e);
                }
                let data_hex = buffer[..length]
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();
                s.hash.update(&buffer[..length]);
                s.offset += length as u64;
                s.touched = Instant::now();
                Ok(Outcome::ExportChunk {
                    offset: *offset,
                    data_hex,
                })
            }
            E::Finish { token } => {
                if !self.active.as_ref().is_some_and(|s| &s.token == token) {
                    return Err(io::ErrorKind::NotFound.into());
                }
                let s = self.active.take().unwrap();
                let metadata = s.file.metadata()?;
                if s.offset != s.size
                    || metadata.len() != s.size
                    || metadata.modified()? != s.modified
                {
                    return Err(io::ErrorKind::InvalidData.into());
                }
                let sha256 = s
                    .hash
                    .finalize()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();
                Ok(Outcome::ExportFinished { sha256 })
            }
            E::Close { token } => {
                if !self.active.as_ref().is_some_and(|s| &s.token == token) {
                    return Err(io::ErrorKind::NotFound.into());
                }
                self.active = None;
                Ok(Outcome::ExportClosed)
            }
        }
    }
}
