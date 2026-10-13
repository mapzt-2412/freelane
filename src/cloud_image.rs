//! Standalone qcow2 import. Header checks do not establish guest architecture or authenticity.
use crate::{Error, Result};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
const MAX_SOURCE: u64 = 16 * 1024 * 1024 * 1024;
fn open(source: &Path) -> Result<fs::File> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(source)
        .map_err(|e| Error(e.to_string()))?;
    let m = file.metadata().map_err(|e| Error(e.to_string()))?;
    if !m.is_file() || m.len() > MAX_SOURCE {
        return Err(Error(
            "Cloud image must be a regular qcow2 file of at most 16 GiB".into(),
        ));
    }
    Ok(file)
}
pub fn validate(source: &Path, disk_gib: u64) -> Result<()> {
    let mut file = open(source)?;
    let mut header = [0u8; 104];
    file.read_exact(&mut header[..72])
        .map_err(|_| Error("Cloud image must be a complete qcow2 image".into()))?;
    let u32_at = |at| u32::from_be_bytes(header[at..at + 4].try_into().unwrap());
    let version = u32_at(4);
    let encryption = u32_at(32);
    let cluster = u32_at(20);
    if &header[..4] != b"QFI\xfb"
        || ![2, 3].contains(&version)
        || encryption != 0
        || !(9..=21).contains(&cluster)
    {
        return Err(Error(
            "Only unencrypted standard qcow2 v2/v3 cloud images are supported".into(),
        ));
    }
    if version == 3 {
        file.read_exact(&mut header[72..])
            .map_err(|_| Error("Incomplete qcow2 header".into()))?;
    }
    let u64_at = |at| u64::from_be_bytes(header[at..at + 8].try_into().unwrap());
    if u64_at(8) != 0 || header[16..20] != [0; 4] || (version == 3 && u64_at(72) != 0) {
        return Err(Error(
            "Cloud image must be standalone, clean and without external data or backing files"
                .into(),
        ));
    }
    let capacity = disk_gib
        .checked_mul(1024 * 1024 * 1024)
        .ok_or_else(|| Error("Disk size overflow".into()))?;
    if u64_at(24) == 0 || u64_at(24) > capacity {
        return Err(Error(
            "Destination disk must be at least the cloud image virtual size".into(),
        ));
    }
    Ok(())
}
fn run(tool: &Path, args: &[std::ffi::OsString]) -> Result<()> {
    let mut child = Command::new(tool)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| Error(format!("qemu-img: {e}")))?;
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(Error("qemu-img cloud conversion/resize failed".into()))
                }
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error(
                    "qemu-img cloud operation timed out or could not be observed".into(),
                ));
            }
        }
    }
}
pub fn import(tool: &Path, source: &Path, destination: &Path, disk_gib: u64) -> Result<()> {
    validate(source, disk_gib)?;
    let parent = fs::canonicalize(
        destination
            .parent()
            .ok_or_else(|| Error("Destination directory missing".into()))?,
    )
    .map_err(|e| Error(e.to_string()))?;
    let m = fs::metadata(&parent).map_err(|e| Error(e.to_string()))?;
    if m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
        return Err(Error(
            "Cloud import destination must be a private directory owned by you".into(),
        ));
    }
    let destination = parent.join(
        destination
            .file_name()
            .ok_or_else(|| Error("Destination name missing".into()))?,
    );
    match fs::symlink_metadata(&destination) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        _ => {
            return Err(Error(
                "Existing cloud import destination is preserved".into(),
            ))
        }
    }
    let token = getrandom::u64().map_err(|e| Error(e.to_string()))?;
    let snapshot = parent.join(format!(".cloud-{token:016x}.source"));
    let output = parent.join(format!(".cloud-{token:016x}.qcow2"));
    let mut owned: Vec<PathBuf> = Vec::new();
    let result = (|| {
        let source = open(source)?;
        let before = source.metadata().map_err(|e| Error(e.to_string()))?;
        let mut copy = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&snapshot)
            .map_err(|e| Error(e.to_string()))?;
        owned.push(snapshot.clone());
        let copied = std::io::copy(&mut source.take(before.len() + 1), &mut copy)
            .map_err(|e| Error(e.to_string()))?;
        if copied != before.len() {
            return Err(Error(
                "Cloud source changed length while copying; import not published".into(),
            ));
        }
        copy.flush()
            .and_then(|_| copy.sync_all())
            .map_err(|e| Error(e.to_string()))?;
        // Validate the private snapshot too: QEMU never sees a concurrently replaceable source path.
        validate(&snapshot, disk_gib)?;
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&output)
            .map_err(|e| Error(e.to_string()))?;
        owned.push(output.clone());
        drop(file);
        run(
            tool,
            &[
                "convert".into(),
                "-f".into(),
                "qcow2".into(),
                "-O".into(),
                "qcow2".into(),
                snapshot.as_os_str().into(),
                output.as_os_str().into(),
            ],
        )?;
        run(
            tool,
            &[
                "resize".into(),
                "-f".into(),
                "qcow2".into(),
                output.as_os_str().into(),
                format!("{disk_gib}G").into(),
            ],
        )?;
        validate(&output, disk_gib)?;
        fs::File::open(&output)
            .and_then(|f| f.sync_all())
            .map_err(|e| Error(e.to_string()))?;
        crate::storage::publish(&output, &destination)?;
        Ok(())
    })();
    for path in owned {
        let _ = fs::remove_file(path);
    }
    result
}
