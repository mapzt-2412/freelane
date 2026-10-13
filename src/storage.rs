//! Read-only filesystem capacity checks; these do not reserve space.
use crate::{Error, Result};
use std::{ffi::CString, os::unix::ffi::OsStrExt, path::Path};
#[derive(Debug, serde::Serialize)]
pub struct Destination {
    pub filesystem_path: std::path::PathBuf,
    pub available_bytes: u64,
}
/// Query the filesystem containing the chosen destination, without creating it.
pub fn inspect_destination(destination: &Path) -> Result<Destination> {
    if destination.as_os_str().is_empty() {
        return Err(Error("Choose a VM destination folder".into()));
    }
    #[cfg(target_os = "macos")]
    check_mounted_volume(destination, Path::new("/Volumes"))?;
    let mut ancestor = destination;
    loop {
        if ancestor.as_os_str().is_empty() {
            ancestor = Path::new(".");
        }
        match std::fs::metadata(ancestor) {
            Ok(m) if m.is_dir() => break,
            Ok(_) => return Err(Error("VM destination ancestor is not a directory".into())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| Error("No existing destination ancestor".into()))?
            }
            Err(e) => return Err(Error(e.to_string())),
        }
    }
    let filesystem_path = std::fs::canonicalize(ancestor).map_err(|e| Error(e.to_string()))?;
    Ok(Destination {
        available_bytes: free_space(&filesystem_path)?,
        filesystem_path,
    })
}
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

/// Prepare a private VM container when a chosen drive root is shared or owned by another user.
/// Called only after an explicit folder selection, never by a capacity query.
pub fn prepare_vm_parent(parent: &Path) -> Result<std::path::PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let selected = std::fs::canonicalize(parent).map_err(|e| Error(e.to_string()))?;
    private_ancestry(&selected)?;
    let metadata = std::fs::metadata(&selected).map_err(|e| Error(e.to_string()))?;
    if !metadata.is_dir() {
        return Err(Error("Choose a VM destination folder".into()));
    }
    if metadata.uid() == unsafe { libc::geteuid() } && metadata.mode() & 0o022 == 0 {
        return Ok(selected);
    }
    let anchor = ParentAnchor::open(&selected.join("new-vm"))?;
    for index in 0..10000 {
        let candidate = selected.join(if index == 0 {
            "FreeLane VMs".into()
        } else {
            format!("FreeLane VMs-{index}")
        });
        match std::fs::symlink_metadata(&candidate) {
            Ok(m)
                if m.is_dir() && m.uid() == unsafe { libc::geteuid() } && m.mode() & 0o022 == 0 =>
            {
                anchor.verify(&candidate)?;
                return Ok(candidate);
            }
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                anchor.mkdir(&candidate)?;
                let m = std::fs::metadata(&candidate).map_err(|e| Error(e.to_string()))?;
                if m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o022 != 0 {
                    return Err(Error("Selected storage cannot provide a private VM folder; choose another folder or volume".into()));
                }
                return Ok(candidate);
            }
            Err(e) => return Err(Error(e.to_string())),
        }
    }
    Err(Error(
        "Choose a folder with fewer conflicting VM names".into(),
    ))
}
fn private_ancestry(path: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let user = unsafe { libc::geteuid() };
    let authority = std::fs::metadata("/")
        .map_err(|e| Error(e.to_string()))?
        .uid();
    for ancestor in path.ancestors() {
        let m = std::fs::metadata(ancestor).map_err(|e| Error(e.to_string()))?;
        let trusted = m.uid() == user || m.uid() == authority;
        if !trusted || (m.mode() & 0o022 != 0 && m.mode() & 0o1000 == 0) {
            return Err(Error(
                "Selected folder can be changed by other users; choose a private storage location"
                    .into(),
            ));
        }
    }
    Ok(())
}

/// Hold the chosen parent directory open across media inspection and staging.
#[derive(Debug)]
pub(crate) struct ParentAnchor {
    file: std::fs::File,
    path: std::path::PathBuf,
}
impl ParentAnchor {
    pub(crate) fn open(destination: &Path) -> Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;
        inspect_destination(destination)?;
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let path = std::fs::canonicalize(parent).map_err(|e| Error(e.to_string()))?;
        private_ancestry(&path)?;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
            .map_err(|e| Error(e.to_string()))?;
        let anchor = Self { file, path };
        anchor.verify(destination)?;
        Ok(anchor)
    }
    pub(crate) fn verify(&self, destination: &Path) -> Result<()> {
        use std::os::unix::fs::MetadataExt;
        #[cfg(target_os = "macos")]
        check_mounted_volume(destination, Path::new("/Volumes"))?;
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let current = std::fs::canonicalize(parent).map_err(|_| {
            Error("Destination changed or disconnected; select the storage again".into())
        })?;
        private_ancestry(&current)?;
        let actual = std::fs::metadata(&current).map_err(|e| Error(e.to_string()))?;
        let pinned = self.file.metadata().map_err(|e| Error(e.to_string()))?;
        if current != self.path || (actual.dev(), actual.ino()) != (pinned.dev(), pinned.ino()) {
            return Err(Error(
                "Destination changed or disconnected; select the storage again".into(),
            ));
        }
        Ok(())
    }
    fn name(&self, path: &Path) -> Result<CString> {
        if path.parent() != Some(self.path.as_path()) {
            return Err(Error("Destination parent mismatch".into()));
        }
        CString::new(
            path.file_name()
                .ok_or_else(|| Error("Destination name missing".into()))?
                .as_bytes(),
        )
        .map_err(|e| Error(e.to_string()))
    }
    fn verify_stage(&self, directory: &Path, stage: &std::fs::File) -> Result<()> {
        use std::os::unix::fs::MetadataExt;
        let actual = std::fs::metadata(directory).map_err(|e| Error(e.to_string()))?;
        let original = stage.metadata().map_err(|e| Error(e.to_string()))?;
        if (actual.dev(), actual.ino()) != (original.dev(), original.ino()) {
            return Err(Error(
                "Destination changed or disconnected; select the storage again".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn mkdir(&self, directory: &Path) -> Result<std::fs::File> {
        use std::os::fd::AsRawFd;
        self.verify(directory)?;
        let name = self.name(directory)?;
        if unsafe { libc::mkdirat(self.file.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
            return Err(Error(std::io::Error::last_os_error().to_string()));
        }
        use std::os::fd::FromRawFd;
        let fd = unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                name.as_ptr(),
                libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(Error(std::io::Error::last_os_error().to_string()));
        }
        let stage = unsafe { std::fs::File::from_raw_fd(fd) };
        self.verify_stage(directory, &stage)?;
        Ok(stage)
    }
    pub(crate) fn publish(&self, source: &Path, target: &Path) -> Result<()> {
        use std::os::fd::AsRawFd;
        self.verify(target)?;
        let source = self.name(source)?;
        let target = self.name(target)?;
        let fd = self.file.as_raw_fd();
        #[cfg(target_os = "linux")]
        let status = unsafe {
            libc::renameat2(
                fd,
                source.as_ptr(),
                fd,
                target.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        #[cfg(target_os = "macos")]
        let status = unsafe {
            libc::renameatx_np(fd, source.as_ptr(), fd, target.as_ptr(), libc::RENAME_EXCL)
        };
        if status != 0 {
            return Err(Error(std::io::Error::last_os_error().to_string()));
        }
        Ok(())
    }
    pub(crate) fn remove_marker(&self, directory: &Path) -> Result<()> {
        use std::os::fd::{AsRawFd, FromRawFd};
        let name = self.name(directory)?;
        let fd = unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                name.as_ptr(),
                libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(Error(std::io::Error::last_os_error().to_string()));
        }
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        let marker = CString::new(crate::provision::MARKER).unwrap();
        if unsafe { libc::unlinkat(file.as_raw_fd(), marker.as_ptr(), 0) } != 0 {
            return Err(Error(std::io::Error::last_os_error().to_string()));
        }
        Ok(())
    }
}

#[cfg(any(target_os = "macos", test))]
fn check_mounted_volume(destination: &Path, volumes: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let Ok(relative) = destination.strip_prefix(volumes) else {
        return Ok(());
    };
    let error = || {
        Error("Selected volume is unavailable. Reconnect it and choose the folder again.".into())
    };
    let name = relative.components().next().ok_or_else(error)?;
    if !matches!(name, std::path::Component::Normal(_)) {
        return Err(error());
    }
    let volume = volumes.join(name.as_os_str());
    let m = std::fs::metadata(&volume).map_err(|_| error())?;
    let parent = std::fs::metadata(volumes).map_err(|_| error())?;
    if !m.is_dir() || m.dev() == parent.dev() {
        return Err(error());
    }
    Ok(())
}
#[cfg(test)]
mod destination_tests {
    use super::*;
    #[test]
    fn disconnected_external_volume_is_not_recreated_on_internal_storage() {
        let root = std::env::temp_dir().join(format!("fl-volume-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let volumes = root.join("Volumes");
        std::fs::create_dir(&volumes).unwrap();
        let destination = volumes.join("Disconnected/VM");
        assert!(check_mounted_volume(&destination, &volumes).is_err());
        assert!(!destination.exists());
        std::fs::create_dir(volumes.join("Disconnected")).unwrap();
        assert!(
            check_mounted_volume(&destination, &volumes).is_err(),
            "ordinary local directory is not a mounted volume"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn replaced_destination_parent_refuses_staging_and_publication() {
        let root = std::env::temp_dir().join(format!("fl-anchor-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let parent = root.join("selected");
        std::fs::create_dir(&parent).unwrap();
        let destination = parent.join("vm");
        let anchor = ParentAnchor::open(&destination).unwrap();
        let retained = root.join("original-filesystem");
        std::fs::rename(&parent, &retained).unwrap();
        std::fs::create_dir(&parent).unwrap();
        assert!(anchor.mkdir(&parent.join("pending")).is_err());
        assert!(anchor
            .publish(&parent.join("pending"), &destination)
            .is_err());
        assert_eq!(std::fs::read_dir(&parent).unwrap().count(), 0);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn explicit_selection_prepares_private_container_in_shared_storage() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let root = std::env::temp_dir().join(format!("fl-private-parent-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(prepare_vm_parent(&root).is_err());
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o1777)).unwrap();
        let parent = prepare_vm_parent(&root).unwrap();
        assert_eq!(
            parent,
            std::fs::canonicalize(&root).unwrap().join("FreeLane VMs")
        );
        assert_eq!(std::fs::metadata(&parent).unwrap().mode() & 0o777, 0o700);
        assert_eq!(prepare_vm_parent(&root).unwrap(), parent);
        assert_eq!(std::fs::read_dir(&parent).unwrap().count(), 0);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn stage_replacement_on_same_filesystem_is_rejected_before_payload_writes() {
        let root = std::env::temp_dir().join(format!("fl-stage-identity-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let anchor = ParentAnchor::open(&root.join("vm")).unwrap();
        let stage = root.join("stage");
        std::fs::create_dir(&stage).unwrap();
        let original = std::fs::File::open(&stage).unwrap();
        std::fs::rename(&stage, root.join("original")).unwrap();
        std::fs::create_dir(&stage).unwrap();
        assert!(anchor.verify_stage(&stage, &original).is_err());
        assert_eq!(std::fs::read_dir(&stage).unwrap().count(), 0);
        std::fs::remove_dir_all(root).unwrap();
    }
}
