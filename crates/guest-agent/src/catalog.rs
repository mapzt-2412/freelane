//! Local registered applications, read without executing registry or desktop-file commands.
use freelane_agent_protocol::{invalid, AppEntry, AppPage};
use std::io;
pub type Entry = AppEntry;
pub fn page(mut entries: Vec<Entry>, cursor: u16) -> io::Result<AppPage> {
    if entries.len() > 512 {
        return Err(invalid("Catalog exceeds limit"));
    }
    for entry in &entries {
        entry.validate()?;
    }
    entries.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.executable.cmp(&b.executable))
    });
    let mut seen = std::collections::BTreeSet::new();
    entries.retain(|entry| seen.insert(entry.executable.clone()));
    let start = cursor as usize;
    if start > entries.len() {
        return Err(invalid("Catalog changed; refresh"));
    }
    let end = (start + 12).min(entries.len());
    Ok(AppPage {
        apps: entries[start..end].to_vec(),
        next: (end < entries.len()).then_some(end as u16),
    })
}
#[cfg(target_os = "linux")]
pub fn entries() -> io::Result<Vec<Entry>> {
    let mut result = Vec::new();
    for (name, path) in [
        ("Terminal", "/usr/bin/gnome-terminal"),
        ("Text Editor", "/usr/bin/gnome-text-editor"),
        ("Gedit", "/usr/bin/gedit"),
        ("Firefox", "/usr/bin/firefox"),
        ("Chromium", "/usr/bin/chromium"),
        ("XTerm", "/usr/bin/xterm"),
    ] {
        if std::path::Path::new(path).is_file() {
            result.push(Entry {
                name: name.into(),
                executable: path.into(),
            });
        }
    }
    Ok(result)
}
#[cfg(windows)]
pub fn entries() -> io::Result<Vec<Entry>> {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    use std::time::Duration;
    static BUSY: AtomicBool = AtomicBool::new(false);
    if BUSY.swap(true, Ordering::AcqRel) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "Catalog enumeration busy",
        ));
    }
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            BUSY.store(false, Ordering::Release);
        }
    }
    let (sender, receiver) = mpsc::sync_channel(1);
    if let Err(error) = std::thread::Builder::new()
        .name("app-catalog".into())
        .spawn(move || {
            let _reset = Reset;
            let _ = sender.send(windows::entries());
        })
    {
        BUSY.store(false, Ordering::Release);
        return Err(error);
    }
    receiver
        .recv_timeout(Duration::from_millis(1500))
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Catalog enumeration unavailable"))?
}
#[cfg(windows)]
mod windows {
    use super::*;
    use std::{ffi::c_void, path::Path};
    type Key = *mut c_void;
    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(
            key: Key,
            path: *const u16,
            options: u32,
            access: u32,
            out: *mut Key,
        ) -> i32;
        fn RegEnumKeyExW(
            key: Key,
            index: u32,
            name: *mut u16,
            len: *mut u32,
            reserved: *mut u32,
            class: *mut u16,
            class_len: *mut u32,
            time: *mut c_void,
        ) -> i32;
        fn RegQueryValueExW(
            key: Key,
            name: *const u16,
            reserved: *mut u32,
            kind: *mut u32,
            data: *mut u8,
            size: *mut u32,
        ) -> i32;
        fn RegCloseKey(key: Key) -> i32;
    }
    struct Owned(Key);
    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }
    fn open(key: Key, path: &str, access: u32) -> Option<Owned> {
        let path: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
        let mut out = std::ptr::null_mut();
        if unsafe { RegOpenKeyExW(key, path.as_ptr(), 0, access, &mut out) } == 0 {
            Some(Owned(out))
        } else {
            None
        }
    }
    fn add(result: &mut std::collections::BTreeMap<String, Entry>, name: String, path: String) {
        let path = path.trim().trim_matches('"').to_string();
        let p = Path::new(&path);
        if path.starts_with("\\\\")
            || !p.is_absolute()
            || !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe"))
            || !p.is_file()
        {
            return;
        }
        let entry = Entry {
            name,
            executable: path,
        };
        if entry.validate().is_ok() {
            result.insert(entry.executable.to_lowercase(), entry);
        }
    }
    pub fn entries() -> io::Result<Vec<Entry>> {
        use std::os::windows::ffi::OsStringExt;
        let mut result = std::collections::BTreeMap::new();
        for hive in [0x80000001u32, 0x80000002u32] {
            for view in [0x100, 0x200] {
                let Some(root) = open(
                    hive as i32 as isize as Key,
                    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths",
                    0x20019 | view,
                ) else {
                    continue;
                };
                for index in 0..128 {
                    let mut name = [0u16; 256];
                    let mut len = 255;
                    let code = unsafe {
                        RegEnumKeyExW(
                            root.0,
                            index,
                            name.as_mut_ptr(),
                            &mut len,
                            std::ptr::null_mut(),
                            std::ptr::null_mut(),
                            std::ptr::null_mut(),
                            std::ptr::null_mut(),
                        )
                    };
                    if code == 259 {
                        break;
                    }
                    if code != 0 {
                        continue;
                    }
                    let key = String::from_utf16(&name[..len as usize])
                        .map_err(|_| invalid("Invalid registry metadata"))?;
                    let Some(app) = open(root.0, &key, 0x20019 | view) else {
                        continue;
                    };
                    let mut data = [0u16; 4096];
                    let mut size = 8192;
                    let mut kind = 0;
                    if unsafe {
                        RegQueryValueExW(
                            app.0,
                            std::ptr::null(),
                            std::ptr::null_mut(),
                            &mut kind,
                            data.as_mut_ptr().cast(),
                            &mut size,
                        )
                    } != 0
                        || kind != 1
                        || size > 8192
                        || size % 2 != 0
                    {
                        continue;
                    }
                    let units = &data[..size as usize / 2];
                    let units = &units[..units.iter().position(|c| *c == 0).unwrap_or(units.len())];
                    let Ok(path) = String::from_utf16(units) else {
                        continue;
                    };
                    let name = key.strip_suffix(".exe").unwrap_or(&key).to_string();
                    add(&mut result, name, path);
                }
            }
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetSystemDirectoryW(path: *mut u16, len: u32) -> u32;
        }
        let mut system = [0u16; 4096];
        let len = unsafe { GetSystemDirectoryW(system.as_mut_ptr(), 4096) };
        if len > 0 && len < 4096 {
            let base =
                std::path::PathBuf::from(std::ffi::OsString::from_wide(&system[..len as usize]));
            for (name, file) in [
                ("Notepad", "notepad.exe"),
                ("Paint", "mspaint.exe"),
                ("Calculator", "calc.exe"),
            ] {
                add(
                    &mut result,
                    name.into(),
                    base.join(file).to_string_lossy().into(),
                );
            }
        }
        Ok(result.into_values().take(512).collect())
    }
}
