//! Read-only filesystem capacity checks; these do not reserve space.
use crate::{Error, Result};
use std::{ffi::CString, os::unix::ffi::OsStrExt, path::Path};
pub(crate) fn free_space(path: &Path) -> Result<u64> {
    let name = CString::new(path.as_os_str().as_bytes()).map_err(|e| Error(e.to_string()))?;
    let mut value = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // The C API initializes this structure on success; the path stays alive throughout.
    if unsafe { libc::statvfs(name.as_ptr(), value.as_mut_ptr()) } != 0 {
        return Err(Error(std::io::Error::last_os_error().to_string()));
    }
    let value = unsafe { value.assume_init() };
    Ok((value.f_bavail as u128 * value.f_frsize as u128).min(u64::MAX as u128) as u64)
}

pub(crate) fn publish(source: &Path, target: &Path) -> Result<()> {
    let source = CString::new(source.as_os_str().as_bytes()).map_err(|e| Error(e.to_string()))?;
    let target = CString::new(target.as_os_str().as_bytes()).map_err(|e| Error(e.to_string()))?;
    // Both live C strings are borrowed only for this atomic no-replace rename.
    #[cfg(target_os = "linux")]
    let status = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    #[cfg(target_os = "macos")]
    let status = unsafe {
        libc::renameatx_np(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    return Err(Error(
        "Atomic no-replace checkpoint publication is unavailable on this platform".into(),
    ));
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if status != 0 {
        return Err(Error(std::io::Error::last_os_error().to_string()));
    }
    Ok(())
}
