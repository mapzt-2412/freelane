//! Explicit copies to an agent's opt-in inbox. Never retry an uncertain mutation.
use crate::{Error, Result};
use freelane_agent_protocol::{Outcome, TransferAction as T, TRANSFER_VERSION};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
fn error() -> Error {
    Error("File transfer failed or canceled; existing guest files preserved".into())
}
pub fn send(
    config: &Path,
    source: &Path,
    cancelled: &impl Fn() -> bool,
    progress: &mut impl FnMut(u64, u64),
) -> Result<()> {
    send_options(config, source, false, cancelled, progress)
}
pub fn send_options(
    config: &Path,
    source: &Path,
    open: bool,
    cancelled: &impl Fn() -> bool,
    progress: &mut impl FnMut(u64, u64),
) -> Result<()> {
    let info = crate::agent_capabilities::inspect(config)?;
    if info.supports("file_transfer", TRANSFER_VERSION) != Some(true) {
        return Err(Error(
            "Enable an inbox with --transfer-root in the updated guest agent".into(),
        ));
    }
    send_with_options(
        source,
        open,
        cancelled,
        &mut |operation| {
            let cleanup = matches!(operation, T::Cancel { .. });
            crate::agent::transfer_request(config, operation, &|| !cleanup && cancelled())
                .map(|r| r.outcome)
        },
        progress,
    )
}
pub fn send_with(
    source: &Path,
    cancelled: &impl Fn() -> bool,
    rpc: &mut impl FnMut(T) -> Result<Outcome>,
    progress: &mut impl FnMut(u64, u64),
) -> Result<()> {
    send_with_options(source, false, cancelled, rpc, progress)
}
pub fn send_with_options(
    source: &Path,
    open: bool,
    cancelled: &impl Fn() -> bool,
    rpc: &mut impl FnMut(T) -> Result<Outcome>,
    progress: &mut impl FnMut(u64, u64),
) -> Result<()> {
    let mut file = File::open(source).map_err(|_| error())?;
    let before = file.metadata().map_err(|_| error())?;
    if !before.is_file() || before.len() > 64 * 1024 * 1024 * 1024 {
        return Err(error());
    }
    let name = source
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(error)?
        .to_string();
    let mut hash = Sha256::new();
    let mut buf = [0; 16384];
    let mut counted = 0u64;
    loop {
        if cancelled() {
            return Err(error());
        }
        let n = file.read(&mut buf).map_err(|_| error())?;
        if n == 0 {
            break;
        }
        counted += n as u64;
        if counted > before.len() {
            return Err(error());
        }
        hash.update(&buf[..n]);
    }
    if counted != before.len() {
        return Err(error());
    }
    let sha256 = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    file.seek(SeekFrom::Start(0)).map_err(|_| error())?;
    if cancelled() {
        return Err(error());
    }
    let token = match rpc(T::Begin {
        name,
        size: before.len(),
        sha256,
    })? {
        Outcome::TransferReady { token }
            if token.len() == 32 && token.bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            token
        }
        _ => return Err(error()),
    };
    let result = (|| {
        let current = file.metadata().map_err(|_| error())?;
        if current.len() != before.len() || current.modified().ok() != before.modified().ok() {
            return Err(error());
        }
        let mut offset = 0;
        loop {
            if cancelled() {
                return Err(error());
            }
            let n = file.read(&mut buf).map_err(|_| error())?;
            if n == 0 {
                break;
            }
            if offset + n as u64 > before.len() {
                return Err(error());
            }
            let data_hex = buf[..n].iter().map(|b| format!("{b:02x}")).collect();
            let reply = rpc(T::Chunk {
                token: token.clone(),
                offset,
                data_hex,
            })?;
            offset += n as u64;
            if reply != (Outcome::TransferProgress { received: offset }) {
                return Err(error());
            }
            progress(offset, before.len());
        }
        if offset != before.len() || cancelled() {
            return Err(error());
        }
        if rpc(T::Finish {
            token: token.clone(),
        })? != Outcome::TransferComplete
        {
            return Err(error());
        }
        if open {
            if cancelled() {
                return Err(Error("File transferred; document opening canceled".into()));
            }
            match rpc(T::Open {
                token: token.clone(),
            }) {
                Ok(Outcome::DocumentOpened) => {}
                _ => {
                    return Err(Error(
                        "File transferred; document opening unavailable or refused".into(),
                    ))
                }
            }
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = rpc(T::Cancel { token });
    }
    result
}
