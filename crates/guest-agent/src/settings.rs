//! Explicit per-user installer selections. No inferred or automatically created folders.
use serde::Deserialize;
use std::{
    fs::OpenOptions,
    io::{self, Read},
    path::{Path, PathBuf},
};
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub inbox: Option<PathBuf>,
    pub outbox: Option<PathBuf>,
}
#[derive(Deserialize)]
struct Manifest {
    schema: u16,
    #[serde(default)]
    transfer: Settings,
}
impl Settings {
    pub fn load(path: &Path) -> io::Result<Self> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x00200000);
        }
        let file = match options.open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e),
        };
        let metadata = file.metadata()?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(io::ErrorKind::InvalidInput.into());
            }
        }
        if !metadata.is_file() || metadata.len() > 4096 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let mut bytes = Vec::new();
        file.take(4097).read_to_end(&mut bytes)?;
        if bytes.len() > 4096 {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let manifest: Manifest =
            serde_json::from_slice(&bytes).map_err(|_| io::ErrorKind::InvalidData)?;
        if manifest.schema != 1 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        for path in [&manifest.transfer.inbox, &manifest.transfer.outbox]
            .into_iter()
            .flatten()
        {
            let text = path.to_str().ok_or(io::ErrorKind::InvalidData)?;
            if !path.is_absolute() || text.len() > 2048 || text.chars().any(char::is_control) {
                return Err(io::ErrorKind::InvalidData.into());
            }
        }
        Ok(manifest.transfer)
    }
}
