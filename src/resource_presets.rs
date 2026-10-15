//! Allocation suggestions, not measured speed/battery policies or live limits.
use crate::{Error, GuestOs, Result};
#[derive(Debug, Clone, Copy)]
pub struct HostCapacity {
    pub cpus: u16,
    pub memory_mib: u64,
}
#[derive(Debug, Clone, Copy)]
pub enum Preset {
    Battery,
    Balanced,
    Performance,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allocation {
    pub cpus: u16,
    pub memory_mib: u64,
}
pub fn suggest(host: HostCapacity, guest: GuestOs, preset: Preset) -> Result<Allocation> {
    let (min_cpus, min_memory) = match guest {
        GuestOs::Windows => (2, 4096),
        GuestOs::Ubuntu => (1, 2048),
    };
    let max_cpus = host.cpus.saturating_sub(1).min(64);
    let max_memory = host.memory_mib.saturating_sub(2048).min(1_048_576);
    if max_cpus < min_cpus || max_memory < min_memory {
        return Err(Error(
            "Host capacity is too small for a resource suggestion".into(),
        ));
    }
    let (cpus, memory) = match preset {
        Preset::Battery => (u32::from(host.cpus) / 4, host.memory_mib / 8),
        Preset::Balanced => (u32::from(host.cpus) / 2, host.memory_mib / 4),
        Preset::Performance => (u32::from(host.cpus) * 3 / 4, host.memory_mib / 2),
    };
    Ok(Allocation {
        cpus: cpus.clamp(u32::from(min_cpus), u32::from(max_cpus)) as u16,
        memory_mib: memory.clamp(min_memory, max_memory) / 256 * 256,
    })
}
impl HostCapacity {
    /// Physical capacity, capped by known Linux memory cgroup limits; not free RAM.
    pub fn detect() -> Option<Self> {
        let cpus =
            u16::try_from(std::thread::available_parallelism().ok()?.get()).unwrap_or(u16::MAX);
        let bytes = physical_memory()?;
        #[cfg(target_os = "linux")]
        let bytes = {
            // Only advertise suggestions when the process belongs to a visible
            // unified root. Nested/remapped/v1 layouts need ancestor resolution;
            // treating their mount-root value as effective would overallocate.
            let membership = std::fs::read_to_string("/proc/self/cgroup").ok()?;
            let mounts = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
            let limit = match std::fs::read_to_string("/sys/fs/cgroup/memory.max") {
                Ok(text) => text,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => "max".into(),
                Err(_) => return None,
            };
            bytes.min(root_memory_limit(&membership, &mounts, &limit)?)
        };
        Some(Self {
            cpus,
            memory_mib: bytes / (1024 * 1024),
        })
    }
}
#[cfg(target_os = "linux")]
fn memory_limit(text: &str) -> Option<u64> {
    let text = text.trim();
    if text == "max" {
        Some(u64::MAX)
    } else {
        text.parse::<u64>().ok().filter(|n| *n > 0)
    }
}
#[cfg(target_os = "linux")]
fn root_memory_limit(membership: &str, mounts: &str, limit: &str) -> Option<u64> {
    if membership.trim() != "0::/" {
        return None;
    }
    let mut unified = mounts.lines().filter_map(|line| {
        let (before, after) = line.split_once(" - ")?;
        if after.split_whitespace().next() != Some("cgroup2") {
            return None;
        }
        Some(before.split_whitespace().collect::<Vec<_>>())
    });
    let mount = unified.next()?;
    if unified.next().is_some()
        || mount.get(3) != Some(&"/")
        || mount.get(4) != Some(&"/sys/fs/cgroup")
    {
        return None;
    }
    memory_limit(limit)
}
#[cfg(target_os = "linux")]
fn physical_memory() -> Option<u64> {
    let pages = u64::try_from(unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) }).ok()?;
    let size = u64::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }).ok()?;
    pages.checked_mul(size).filter(|n| *n > 0)
}
#[cfg(target_os = "macos")]
fn physical_memory() -> Option<u64> {
    let mut bytes = 0u64;
    let mut len = std::mem::size_of::<u64>();
    let status = unsafe {
        libc::sysctlbyname(
            c"hw.memsize".as_ptr(),
            (&mut bytes as *mut u64).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    (status == 0 && len == std::mem::size_of::<u64>() && bytes > 0).then_some(bytes)
}
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn physical_memory() -> Option<u64> {
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    #[test]
    fn unknown_nested_or_remapped_cgroup_ceiling_disables_suggestions() {
        let root = "10 9 0:1 / /sys/fs/cgroup ro - cgroup2 cgroup rw";
        assert_eq!(
            root_memory_limit("0::/\n", root, "6442450944"),
            Some(6442450944)
        );
        assert_eq!(
            root_memory_limit("0::/user.slice/session.scope\n", root, "max"),
            None
        );
        assert_eq!(
            root_memory_limit(
                "0::/\n",
                "10 9 0:1 /limited /sys/fs/cgroup ro - cgroup2 cgroup rw",
                "max"
            ),
            None
        );
        assert_eq!(root_memory_limit("5:memory:/limited\n", root, "max"), None);
    }
    #[test]
    fn resource_suggestions_reserve_host_capacity_and_keep_windows_minimums() {
        let host = HostCapacity {
            cpus: 8,
            memory_mib: 16384,
        };
        assert_eq!(
            suggest(host, GuestOs::Windows, Preset::Battery).unwrap(),
            Allocation {
                cpus: 2,
                memory_mib: 4096
            }
        );
        assert_eq!(
            suggest(host, GuestOs::Windows, Preset::Balanced).unwrap(),
            Allocation {
                cpus: 4,
                memory_mib: 4096
            }
        );
        assert_eq!(
            suggest(host, GuestOs::Windows, Preset::Performance).unwrap(),
            Allocation {
                cpus: 6,
                memory_mib: 8192
            }
        );
        assert_eq!(
            suggest(host, GuestOs::Ubuntu, Preset::Battery).unwrap(),
            Allocation {
                cpus: 2,
                memory_mib: 2048
            }
        );
    }
    #[test]
    fn small_or_invalid_hosts_are_refused_instead_of_suggesting_overallocation() {
        for host in [
            HostCapacity {
                cpus: 0,
                memory_mib: 16384,
            },
            HostCapacity {
                cpus: 1,
                memory_mib: 16384,
            },
            HostCapacity {
                cpus: 8,
                memory_mib: 4096,
            },
        ] {
            assert!(suggest(host, GuestOs::Windows, Preset::Performance).is_err());
        }
    }
    #[test]
    fn all_presets_stay_bounded_and_monotonic_over_supported_host_sizes() {
        for cpus in [4, 6, 8, 12, 64, u16::MAX] {
            for memory_mib in [6144, 8192, 16384, 32768, u64::MAX] {
                let host = HostCapacity { cpus, memory_mib };
                for guest in [GuestOs::Windows, GuestOs::Ubuntu] {
                    let allocations = [Preset::Battery, Preset::Balanced, Preset::Performance]
                        .map(|p| suggest(host, guest, p).unwrap());
                    for a in allocations {
                        assert!(a.cpus < cpus && a.cpus <= 64);
                        assert!(a.memory_mib <= 1_048_576 && a.memory_mib <= memory_mib - 2048);
                        assert!(a.memory_mib >= 2048);
                    }
                    assert!(allocations
                        .windows(2)
                        .all(|w| w[0].cpus <= w[1].cpus && w[0].memory_mib <= w[1].memory_mib));
                }
            }
        }
    }
}
