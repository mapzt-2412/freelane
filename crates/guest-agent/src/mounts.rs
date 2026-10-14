//! Bounded, read-only observation of the fixed FreeLane9p mount tag.
use freelane_agent_protocol::{invalid, SharedMount};
use std::io;
pub fn parse(text: &str) -> io::Result<Vec<SharedMount>> {
    if text.len() > 1024 * 1024 {
        return Err(invalid("Mount metadata exceeds limit"));
    }
    let mut mounts = Vec::new();
    for line in text.lines() {
        let Some((before, after)) = line.split_once(" - ") else {
            continue;
        };
        let tail: Vec<_> = after.split_whitespace().collect();
        if tail.len() < 2 || tail[0] != "9p" || tail[1] != "freelane" {
            continue;
        }
        let fields: Vec<_> = before.split_whitespace().collect();
        if fields.len() < 6 {
            return Err(invalid("Invalid share mount metadata"));
        }
        let raw = fields[4].as_bytes();
        let mut bytes = Vec::new();
        let mut index = 0;
        while index < raw.len() {
            if raw[index] == b'\\' {
                if index + 3 >= raw.len()
                    || !raw[index + 1..index + 4]
                        .iter()
                        .all(|b| (b'0'..=b'7').contains(b))
                {
                    return Err(invalid("Invalid mount escape"));
                }
                let value = ((raw[index + 1] - b'0') as u16) * 64
                    + ((raw[index + 2] - b'0') as u16) * 8
                    + (raw[index + 3] - b'0') as u16;
                if value > 255 {
                    return Err(invalid("Invalid mount escape"));
                }
                bytes.push(value as u8);
                index += 4;
            } else {
                bytes.push(raw[index]);
                index += 1;
            }
        }
        let path =
            String::from_utf8(bytes).map_err(|_| invalid("Unsupported mount path encoding"))?;
        let options: Vec<_> = fields[5].split(',').collect();
        if !options.contains(&"ro") && !options.contains(&"rw") {
            return Err(invalid("Invalid mount access mode"));
        }
        let mount = SharedMount {
            path,
            readonly: options.contains(&"ro"),
        };
        mount.validate()?;
        mounts.push(mount);
        if mounts.len() > 16 {
            return Err(invalid("Too many share mounts"));
        }
    }
    Ok(mounts)
}
#[cfg(target_os = "linux")]
pub fn current() -> io::Result<Vec<SharedMount>> {
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open("/proc/self/mountinfo")?
        .take(1024 * 1024 + 1)
        .read_to_string(&mut text)?;
    parse(&text)
}
