//! Bounded, read-only ISO9660 inspection. A verified UEFI loader is not OS-installation proof.
use crate::{Error, Result};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
const SECTOR: u64 = 2048;
fn fail() -> Error {
    Error(
        "Cannot verify ARM64 installer media. Choose an ISO containing EFI/BOOT/BOOTAA64.EFI."
            .into(),
    )
}
struct Entry {
    sector: u32,
    size: u32,
    directory: bool,
}
fn entry(record: &[u8]) -> Result<Entry> {
    if record.len() < 34
        || record[0] as usize != record.len()
        || record[32] as usize + 33 > record.len()
        || record[26] != 0
        || record[27] != 0
        || record[25] & 0x80 != 0
    {
        return Err(fail());
    }
    let sector = u32::from_le_bytes(record[2..6].try_into().unwrap());
    let size = u32::from_le_bytes(record[10..14].try_into().unwrap());
    if sector != u32::from_be_bytes(record[6..10].try_into().unwrap())
        || size != u32::from_be_bytes(record[14..18].try_into().unwrap())
    {
        return Err(fail());
    }
    Ok(Entry {
        sector,
        size,
        directory: record[25] & 2 != 0,
    })
}
fn read_at(file: &mut File, length: u64, offset: u64, count: usize) -> Result<Vec<u8>> {
    if count > 1024 * 1024
        || offset
            .checked_add(count as u64)
            .is_none_or(|end| end > length)
    {
        return Err(fail());
    }
    let mut bytes = vec![0; count];
    file.seek(SeekFrom::Start(offset))
        .and_then(|_| file.read_exact(&mut bytes))
        .map_err(|_| fail())?;
    Ok(bytes)
}
fn child(file: &mut File, length: u64, parent: &Entry, name: &[u8]) -> Result<Entry> {
    if !parent.directory || parent.size == 0 {
        return Err(fail());
    }
    let bytes = read_at(
        file,
        length,
        parent.sector as u64 * SECTOR,
        parent.size as usize,
    )?;
    let mut cursor = 0;
    let mut found = None;
    while cursor < bytes.len() {
        let n = bytes[cursor] as usize;
        if n == 0 {
            cursor = (cursor / 2048 + 1) * 2048;
            continue;
        }
        if n < 34 || cursor + n > bytes.len() || cursor % 2048 + n > 2048 {
            return Err(fail());
        }
        let record = &bytes[cursor..cursor + n];
        if 33 + record[32] as usize > record.len() {
            return Err(fail());
        }
        let id = &record[33..33 + record[32] as usize];
        let id = id.split(|c| *c == b';').next().unwrap();
        if id.eq_ignore_ascii_case(name) {
            if found.is_some() {
                return Err(fail());
            }
            found = Some(entry(record)?);
        }
        cursor += n;
    }
    found.ok_or_else(fail)
}
fn verify_iso9660(path: &Path) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    // Nonblocking open prevents a replaced FIFO from hanging the preflight.
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| Error(format!("Installer: {e}")))?;
    let metadata = file.metadata().map_err(|_| fail())?;
    if !metadata.is_file() {
        return Err(fail());
    }
    let length = metadata.len();
    let mut root = None;
    for sector in 16..80 {
        let descriptor = read_at(&mut file, length, sector * SECTOR, 2048)?;
        if &descriptor[1..6] != b"CD001" || descriptor[6] != 1 {
            return Err(fail());
        }
        if descriptor[0] == 255 {
            break;
        }
        if descriptor[0] == 1 {
            let n = descriptor[156] as usize;
            if n < 34 || 156 + n > 2048 {
                return Err(fail());
            }
            root = Some(entry(&descriptor[156..156 + n])?);
            break;
        }
    }
    let root = root.ok_or_else(fail)?;
    let efi = child(&mut file, length, &root, b"EFI")?;
    let boot = child(&mut file, length, &efi, b"BOOT")?;
    let loader = child(&mut file, length, &boot, b"BOOTAA64.EFI")?;
    if loader.directory || loader.size < 64 {
        return Err(fail());
    }
    let offset = loader.sector as u64 * SECTOR;
    // Bound the entire extent, even though only its header is read.
    if offset
        .checked_add(loader.size as u64)
        .is_none_or(|end| end > length)
    {
        return Err(fail());
    }
    let dos = read_at(&mut file, length, offset, 64)?;
    if &dos[..2] != b"MZ" {
        return Err(fail());
    }
    let pe = u32::from_le_bytes(dos[60..64].try_into().unwrap()) as u64;
    if pe < 64 || pe + 6 > loader.size as u64 {
        return Err(fail());
    }
    let header = read_at(&mut file, length, offset + pe, 6)?;
    if &header[..4] != b"PE\0\0" || u16::from_le_bytes(header[4..6].try_into().unwrap()) != 0xaa64 {
        return Err(fail());
    }
    Ok(())
}

/// ISO9660 is inspected directly; UDF/bridge layouts require a local 7zz or 7z.
/// Only the architecture of the selected UEFI bootloader is established.
pub fn verify_arm64_iso(path: &Path) -> Result<()> {
    if verify_iso9660(path).is_ok() {
        return Ok(());
    }
    verify_with_7zip(path)
}
fn verify_with_7zip(path: &Path) -> Result<()> {
    use std::{
        process::{Command, Stdio},
        sync::mpsc,
        time::{Duration, Instant},
    };
    let path = std::fs::canonicalize(path).map_err(|e| Error(format!("Installer: {e}")))?;
    if !std::fs::metadata(&path).map_err(|_| fail())?.is_file() {
        return Err(fail());
    }
    let start = Instant::now();
    let mut spawned = None;
    for tool in ["7zz", "7z"] {
        match Command::new(crate::host_tools::binary(tool))
            .args(["e", "-tudf", "-so", "-bd", "-y", "-ssc-"])
            .arg(&path)
            .arg("EFI/BOOT/BOOTAA64.EFI")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                spawned = Some(child);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                return Err(Error(
                    "Unable to execute 7-Zip for installer inspection".into(),
                ))
            }
        }
    }
    let mut child=spawned.ok_or_else(|| Error("Installer needs UDF inspection. Install 7-Zip (macOS: brew install sevenzip), then retry.".into()))?;
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = tx.send(result);
    });
    let bytes = match rx.recv_timeout(Duration::from_secs(30)) {
        Ok(Ok(bytes)) if bytes.len() <= 8 * 1024 * 1024 => bytes,
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error(
                "Installer inspection exceeded its size/time limit".into(),
            ));
        }
    };
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < Duration::from_secs(30) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error("Installer inspection did not finish".into()));
            }
        }
    };
    if !status.success() || bytes.len() < 64 || &bytes[..2] != b"MZ" {
        return Err(fail());
    }
    let pe = u32::from_le_bytes(bytes[60..64].try_into().unwrap()) as usize;
    if pe < 64
        || pe.checked_add(6).is_none_or(|end| end > bytes.len())
        || &bytes[pe..pe + 4] != b"PE\0\0"
        || u16::from_le_bytes(bytes[pe + 4..pe + 6].try_into().unwrap()) != 0xaa64
    {
        return Err(fail());
    }
    Ok(())
}
