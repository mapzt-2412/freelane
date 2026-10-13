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
