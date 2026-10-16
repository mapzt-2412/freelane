use crate::{Error, Result};
use freelane_agent_protocol::{
    export::{self, Operation as E},
    Outcome,
};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
fn refused() -> Error {
    Error("File receive failed or canceled; existing host files preserved".into())
}
pub fn receive(
    config: &Path,
    name: &str,
    destination: &Path,
    cancelled: &impl Fn() -> bool,
    progress: &mut impl FnMut(u64, u64),
) -> Result<PathBuf> {
    if crate::agent_capabilities::inspect(config)?.supports("file_export", export::VERSION)
        != Some(true)
    {
        return Err(Error(
            "Enable an outbox with --export-root in the updated guest agent".into(),
        ));
    }
    receive_with(name, destination, cancelled, progress, |operation| {
        let cleanup = matches!(operation, E::Close { .. });
        crate::agent::export_request(config, operation, &|| !cleanup && cancelled())
            .map(|r| r.outcome)
    })
}
fn receive_with(
    name: &str,
    destination: &Path,
    cancelled: &impl Fn() -> bool,
    progress: &mut impl FnMut(u64, u64),
    mut rpc: impl FnMut(E) -> Result<Outcome>,
) -> Result<PathBuf> {
    if !export::valid_name(name) || cancelled() {
        return Err(refused());
    }
    // Canonicalize the user-selected host folder once. Local staging is private (0600).
    let destination = std::fs::canonicalize(destination).map_err(|_| refused())?;
    if !destination.is_dir() {
        return Err(refused());
    }
    let output = destination.join(name);
    if std::fs::symlink_metadata(&output).is_ok() {
        return Err(Error("Destination file already exists".into()));
    }
    let (token, size) = match rpc(E::Begin { name: name.into() })? {
        Outcome::ExportReady { token, size }
            if export::hex(&token, 32) && size <= export::MAX_SIZE =>
        {
            (token, size)
        }
        _ => return Err(refused()),
    };
    let result = (|| {
        let mut stage = tempfile::NamedTempFile::new_in(&destination).map_err(|_| refused())?;
        let mut offset = 0u64;
        let mut hash = Sha256::new();
        progress(0, size);
        while offset < size {
            if cancelled() {
                return Err(refused());
            }
            let data_hex = match rpc(E::Read {
                token: token.clone(),
                offset,
            })? {
                Outcome::ExportChunk {
                    offset: actual,
                    data_hex,
                } if actual == offset
                    && !data_hex.is_empty()
                    && data_hex.len() <= export::MAX_CHUNK * 2
                    && data_hex.len() % 2 == 0
                    && export::hex(&data_hex, data_hex.len()) =>
                {
                    data_hex
                }
                _ => return Err(refused()),
            };
            let data: Vec<u8> = data_hex
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                .collect();
            if data.len() as u64 > size - offset {
                return Err(refused());
            }
            stage.write_all(&data).map_err(|_| refused())?;
            hash.update(&data);
            offset += data.len() as u64;
            progress(offset, size);
        }
        if cancelled() {
            return Err(refused());
        }
        let expected = match rpc(E::Finish {
            token: token.clone(),
        })? {
            Outcome::ExportFinished { sha256 } if export::hex(&sha256, 64) => {
                sha256.to_ascii_lowercase()
            }
            _ => return Err(refused()),
        };
        if cancelled()
            || hash
                .finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
                != expected
        {
            return Err(refused());
        }
        stage.as_file().sync_all().map_err(|_| refused())?;
        stage.persist_noclobber(&output).map_err(|_| refused())?;
        Ok(output)
    })();
    // Best-effort release even after cancellation. Never retry reads or publication.
    let _ = rpc(E::Close { token });
    result
}
/// Explicit read-only listing of a separately opted-in guest outbox.
pub fn catalog(config: &Path) -> Result<freelane_agent_protocol::export_catalog::Page> {
    if crate::agent_capabilities::inspect(config)?.supports(
        "file_export_catalog",
        freelane_agent_protocol::export_catalog::VERSION,
    ) != Some(true)
    {
        return Err(Error(
            "Update guest agent and enable an outbox to choose a file".into(),
        ));
    }
    match crate::agent::export_catalog(config)?.outcome {
        Outcome::ExportCatalog { page } => {
            page.validate()
                .map_err(|_| Error("Invalid guest outbox listing".into()))?;
            Ok(page)
        }
        _ => Err(Error("Invalid guest outbox listing".into())),
    }
}

/// Read-only check or explicitly requested reopening of previously unavailable configured roots.
pub fn folders(
    config: &Path,
    reconnect: bool,
) -> Result<freelane_agent_protocol::transfer_folders::Report> {
    if crate::agent_capabilities::inspect(config)?.supports(
        "file_folders",
        freelane_agent_protocol::transfer_folders::VERSION,
    ) != Some(true)
    {
        return Err(Error("Update guest agent to check guest folders".into()));
    }
    match crate::agent::transfer_folders(config, reconnect)?.outcome {
        Outcome::TransferFolders { report } => Ok(report),
        _ => Err(Error("Invalid guest folder response".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reply(op: E, bytes: &[u8], bad_hash: bool) -> Result<Outcome> {
        Ok(match op {
            E::Begin { .. } => Outcome::ExportReady {
                token: "a".repeat(32),
                size: bytes.len() as u64,
            },
            E::Finish { .. } => Outcome::ExportFinished {
                sha256: if bad_hash {
                    "0".repeat(64)
                } else {
                    Sha256::digest(bytes)
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                },
            },
            E::Read { offset, .. } => Outcome::ExportChunk {
                offset,
                data_hex: bytes[offset as usize..]
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect(),
            },
            E::Close { .. } => Outcome::ExportClosed,
        })
    }
    #[test]
    fn receive_verifies_bytes_and_never_replaces_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let output = receive_with(
            "Tài liệu.txt",
            dir.path(),
            &|| false,
            &mut |_, _| {},
            |op| reply(op, b"guest bytes", false),
        )
        .unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), b"guest bytes");
        assert!(receive_with(
            "Tài liệu.txt",
            dir.path(),
            &|| false,
            &mut |_, _| {},
            |op| reply(op, b"new bytes", false)
        )
        .is_err());
        assert_eq!(std::fs::read(output).unwrap(), b"guest bytes");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn bad_hash_offset_and_cancellation_leave_no_output_and_close_the_session() {
        let dir = tempfile::tempdir().unwrap();
        let mut closed = false;
        assert!(
            receive_with("bad.txt", dir.path(), &|| false, &mut |_, _| {}, |op| {
                if matches!(op, E::Close { .. }) {
                    closed = true;
                }
                reply(op, b"bytes", true)
            })
            .is_err()
        );
        assert!(closed);
        let cancelled = std::cell::Cell::new(false);
        assert!(receive_with(
            "cancel.txt",
            dir.path(),
            &|| cancelled.get(),
            &mut |_, _| {},
            |op| {
                let begin = matches!(op, E::Begin { .. });
                let r = reply(op, b"bytes", false);
                if begin {
                    cancelled.set(true);
                }
                r
            }
        )
        .is_err());
        assert!(
            receive_with("offset.txt", dir.path(), &|| false, &mut |_, _| {}, |op| {
                if matches!(op, E::Read { .. }) {
                    Ok(Outcome::ExportChunk {
                        offset: 1,
                        data_hex: "11".into(),
                    })
                } else {
                    reply(op, b"bytes", false)
                }
            })
            .is_err()
        );
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
