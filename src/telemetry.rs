//! Observed host process counters; configured guest memory is not resident memory.
use crate::{Error, Result};
use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct ProcessResources {
    pub resident_bytes: u64,
    pub cpu_nanoseconds: u64,
}
#[cfg(target_os = "linux")]
pub fn parse_linux_stat(
    text: &str,
    ticks_per_second: u64,
    page_bytes: u64,
) -> Result<ProcessResources> {
    let invalid = || Error("Invalid process resource counters".into());
    if ticks_per_second == 0 || page_bytes == 0 || text.len() > 65536 {
        return Err(invalid());
    }
    let end = text.rfind(')').ok_or_else(invalid)?;
    if !text[..end].contains('(') {
        return Err(invalid());
    }
    let fields: Vec<_> = text[end + 1..].split_whitespace().collect();
    let number = |index: usize| {
        fields
            .get(index)
            .ok_or_else(invalid)?
            .parse::<u64>()
            .map_err(|_| invalid())
    };
    let ticks = number(11)?.checked_add(number(12)?).ok_or_else(invalid)?;
    let nanos = (u128::from(ticks) * 1_000_000_000 / u128::from(ticks_per_second))
        .try_into()
        .map_err(|_| invalid())?;
    Ok(ProcessResources {
        resident_bytes: number(21)?.checked_mul(page_bytes).ok_or_else(invalid)?,
        cpu_nanoseconds: nanos,
    })
}
#[cfg(target_os = "linux")]
pub fn process(pid: u32) -> Result<ProcessResources> {
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open(format!("/proc/{pid}/stat"))
        .and_then(|f| f.take(65537).read_to_string(&mut text))
        .map_err(|e| Error(format!("Process counters unavailable: {e}")))?;
    // sysconf has no pointer arguments and these constants request immutable system limits.
    let (ticks, page) = unsafe {
        (
            libc::sysconf(libc::_SC_CLK_TCK),
            libc::sysconf(libc::_SC_PAGESIZE),
        )
    };
    if ticks <= 0 || page <= 0 {
        return Err(Error("Process counter units unavailable".into()));
    }
    parse_linux_stat(&text, ticks as u64, page as u64)
}
#[cfg(target_os = "macos")]
pub fn process(pid: u32) -> Result<ProcessResources> {
    let pid = i32::try_from(pid).map_err(|_| Error("Invalid process identifier".into()))?;
    // libc supplies the ABI layout; the kernel writes the requested v2 structure.
    let mut usage: libc::rusage_info_v2 = unsafe { std::mem::zeroed() };
    let result = unsafe {
        libc::proc_pid_rusage(
            pid,
            libc::RUSAGE_INFO_V2,
            (&mut usage as *mut libc::rusage_info_v2).cast(),
        )
    };
    if result != 0 {
        return Err(Error("Process counters unavailable".into()));
    }
    Ok(ProcessResources {
        resident_bytes: usage.ri_resident_size,
        cpu_nanoseconds: usage
            .ri_user_time
            .checked_add(usage.ri_system_time)
            .ok_or_else(|| Error("CPU counter overflow".into()))?,
    })
}
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn process(_: u32) -> Result<ProcessResources> {
    Err(Error(
        "Host process telemetry is unsupported on this platform".into(),
    ))
}

/// Sample the process attached to the VM QMP socket. Both status queries must succeed.
pub fn vm(config: &std::path::Path) -> Result<serde_json::Value> {
    let mut qmp = crate::qmp::Qmp::connect(&crate::qemu::runtime_dir(config)?.join("qmp.sock"))?;
    qmp.execute("query-status", None)?;
    let pid = qmp.peer_pid()?;
    let resources = process(pid)?;
    let status = qmp.execute("query-status", None)?;
    Ok(
        serde_json::json!({"process_id":pid,"resources":resources,"status":status,"cpu_unit":"cumulative_nanoseconds","memory_unit":"resident_bytes"}),
    )
}
