//! Publish the validated file handle, never a mutable staging pathname.
use cap_std::fs::{Dir, File};
use std::io;
#[cfg(target_os = "linux")]
pub fn create(root: &Dir) -> io::Result<File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let fd = unsafe {
        libc::openat(
            root.as_raw_fd(),
            c".".as_ptr(),
            libc::O_TMPFILE | libc::O_RDWR | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(File::from_std(unsafe { std::fs::File::from_raw_fd(fd) }))
}
#[cfg(target_os = "linux")]
pub fn publish(file: &File, root: &Dir, name: &str) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let source = std::ffi::CString::new(format!("/proc/self/fd/{}", file.as_raw_fd())).unwrap();
    let dest = std::ffi::CString::new(name).map_err(|_| io::ErrorKind::InvalidInput)?;
    let result = unsafe {
        libc::linkat(
            libc::AT_FDCWD,
            source.as_ptr(),
            root.as_raw_fd(),
            dest.as_ptr(),
            libc::AT_SYMLINK_FOLLOW,
        )
    };
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn SetFileInformationByHandle(
        handle: *mut std::ffi::c_void,
        class: i32,
        info: *const std::ffi::c_void,
        size: u32,
    ) -> i32;
}
#[cfg(windows)]
pub fn create(root: &Dir) -> io::Result<File> {
    use cap_std::fs::{OpenOptions, OpenOptionsExt};
    let mut random = [0; 16];
    getrandom::fill(&mut random).map_err(|_| io::ErrorKind::Other)?;
    let name = format!(
        ".freelane-stage-{}",
        random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    // No shared read/write/delete access while transferring; deletion is bound to this handle.
    let mut options = OpenOptions::new();
    options
        .create_new(true)
        .write(true)
        .access_mode(0x40000000 | 0x00010000)
        .share_mode(0);
    let file = root.open_with(name, &options)?;
    Ok(file)
}
#[cfg(windows)]
pub fn publish(file: &File, root: &Dir, name: &str) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    #[repr(C)]
    struct Link {
        replace: u8,
        root: *mut std::ffi::c_void,
        length: u32,
        name: [u16; 1],
    }
    let utf16: Vec<u16> = name.encode_utf16().collect();
    let offset = std::mem::offset_of!(Link, name);
    let bytes = offset + utf16.len() * 2;
    let mut buffer = vec![0u64; bytes.div_ceil(8)];
    let link = buffer.as_mut_ptr().cast::<Link>();
    unsafe {
        (*link).replace = 0;
        (*link).root = root.as_raw_handle();
        (*link).length = (utf16.len() * 2) as u32;
        std::ptr::copy_nonoverlapping(
            utf16.as_ptr(),
            buffer.as_mut_ptr().cast::<u8>().add(offset).cast::<u16>(),
            utf16.len(),
        );
    }
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            11,
            buffer.as_ptr().cast(),
            bytes as u32,
        )
    } == 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
#[cfg(not(any(target_os = "linux", windows)))]
pub fn create(_: &Dir) -> io::Result<File> {
    Err(io::ErrorKind::Unsupported.into())
}
#[cfg(not(any(target_os = "linux", windows)))]
pub fn publish(_: &File, _: &Dir, _: &str) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(windows)]
pub fn cleanup(file: &File) {
    use std::os::windows::io::AsRawHandle;
    let delete = 1u8;
    let _ = unsafe {
        SetFileInformationByHandle(file.as_raw_handle(), 4, (&delete as *const u8).cast(), 1)
    };
}
#[cfg(not(windows))]
pub fn cleanup(_: &File) {}
