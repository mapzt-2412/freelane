//! Open only explicitly requested, unchanged transferred documents through literal paths.
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};
fn refused() -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, "Document open refused")
}
pub fn verified_path(mut file: File, name: &str, size: u64, expected: &str) -> io::Result<PathBuf> {
    let extension = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !["txt", "csv"].contains(&extension.as_str())
        || size > 32 * 1024 * 1024
        || !file.metadata()?.is_file()
        || file.metadata()?.len() != size
    {
        return Err(refused());
    }
    let mut hash = Sha256::new();
    let mut buf = [0; 16384];
    let mut count = 0;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > size {
            return Err(refused());
        }
        hash.update(&buf[..n]);
    }
    let actual: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if count != size || actual != expected {
        return Err(refused());
    }
    let path = file_path(&file)?;
    let actual = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !["txt", "csv"].contains(&actual.as_str()) {
        return Err(refused());
    }
    Ok(path)
}
#[cfg(target_os = "linux")]
fn file_path(file: &File) -> io::Result<PathBuf> {
    use std::os::fd::AsRawFd;
    let path = std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))?;
    if !path.is_absolute() || path.to_string_lossy().ends_with(" (deleted)") {
        return Err(refused());
    }
    Ok(path)
}
#[cfg(windows)]
fn file_path(file: &File) -> io::Result<PathBuf> {
    use std::os::windows::{ffi::OsStringExt, io::AsRawHandle};
    #[link(name = "kernel32")]
    extern "system" {
        fn GetFinalPathNameByHandleW(
            handle: *mut std::ffi::c_void,
            path: *mut u16,
            len: u32,
            flags: u32,
        ) -> u32;
    }
    let mut buffer = vec![0u16; 32768];
    let n = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle(),
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            0,
        )
    };
    if n == 0 || n as usize >= buffer.len() {
        return Err(refused());
    }
    Ok(std::ffi::OsString::from_wide(&buffer[..n as usize]).into())
}
#[cfg(not(any(windows, target_os = "linux")))]
fn file_path(_: &File) -> io::Result<PathBuf> {
    Err(refused())
}
#[cfg(target_os = "linux")]
pub fn open(path: &Path) -> io::Result<()> {
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return Err(refused());
    }
    let editor = ["/usr/bin/gnome-text-editor", "/usr/bin/gedit"]
        .into_iter()
        .find(|p| Path::new(p).is_file())
        .ok_or_else(refused)?;
    let mut child = std::process::Command::new(editor)
        .arg("--")
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    // Reap asynchronously; success means association launch requested, not rendered.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
#[cfg(windows)]
pub fn open(path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStringExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetSystemDirectoryW(buffer: *mut u16, size: u32) -> u32;
    }
    let mut buffer = vec![0u16; 32768];
    let n = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
    if n == 0 || n as usize >= buffer.len() {
        return Err(refused());
    }
    let editor =
        PathBuf::from(std::ffi::OsString::from_wide(&buffer[..n as usize])).join("notepad.exe");
    let mut child = std::process::Command::new(editor)
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
#[cfg(not(any(windows, target_os = "linux")))]
pub fn open(_: &Path) -> io::Result<()> {
    Err(refused())
}
#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn renamed_document_cannot_become_executable_dispatch_path() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("a.txt");
        std::fs::write(&path, b"abc").unwrap();
        let file = std::fs::File::open(&path).unwrap();
        std::fs::rename(&path, root.path().join("a.exe")).unwrap();
        assert!(super::verified_path(
            file,
            "a.txt",
            3,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        )
        .is_err());
    }
}
