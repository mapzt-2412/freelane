//! Platform file dialogs run in the desktop worker, never the render thread.
use std::path::PathBuf;

pub fn choose(folder: bool) -> Result<Option<String>, String> {
    use std::process::Command;
    let mut command = if cfg!(target_os = "macos") {
        let mut c = Command::new("/usr/bin/osascript");
        // Fixed scripts: user paths are never interpolated into AppleScript or a shell.
        c.arg("-e").arg(if folder {
            "try\nreturn POSIX path of (choose folder)\non error number -128\nreturn \"\"\nend try"
        } else {
            "try\nreturn POSIX path of (choose file)\non error number -128\nreturn \"\"\nend try"
        });
        c
    } else {
        let mut c = Command::new("zenity");
        c.arg("--file-selection");
        if folder {
            c.arg("--directory");
        }
        c
    };
    let output = command.output().map_err(|_| {
        "File dialog unavailable. Enter the path manually; on Linux install zenity.".to_string()
    })?;
    if !output.status.success() {
        if !cfg!(target_os = "macos") && output.status.code() == Some(1) {
            return Ok(None);
        }
        return Err("Unable to open the file dialog. Enter the path manually.".into());
    }
    decode_selection(&output.stdout)?
        .map(|p| {
            p.into_os_string()
                .into_string()
                .map_err(|_| "Selected path is not UTF-8".into())
        })
        .transpose()
}
fn decode_selection(bytes: &[u8]) -> Result<Option<PathBuf>, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "Selected path is not UTF-8")?;
    let text = text.strip_suffix('\n').unwrap_or(text);
    if text.is_empty() {
        return Ok(None);
    }
    if text.contains(['\n', '\r', '\0']) || text.len() > 16_384 {
        return Err("Selected path cannot be represented by the form".into());
    }
    let path = PathBuf::from(text);
    if !path.is_absolute() {
        return Err("Select an absolute path".into());
    }
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_and_spaces_are_preserved() {
        assert_eq!(
            decode_selection("/tmp/Dữ liệu ARM.iso\n".as_bytes()).unwrap(),
            Some(PathBuf::from("/tmp/Dữ liệu ARM.iso"))
        );
    }
    #[test]
    fn cancel_is_empty_but_ambiguous_or_relative_paths_are_errors() {
        assert_eq!(decode_selection(b"").unwrap(), None);
        for output in [
            &b"relative.iso\n"[..],
            &b"/tmp/a\n/tmp/b\n"[..],
            &b"/tmp/a\0b\n"[..],
        ] {
            assert!(decode_selection(output).is_err());
        }
    }
}
