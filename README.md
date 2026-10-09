# FreeLane

### Windows on Mac. Free & Open Source.

**A Rust-based alternative to Parallels Desktop for Apple Silicon — built around an ambitious goal: smooth Windows gaming with less overhead.**

No subscription. Open code. Performance claims backed by measurements.

> **Project status: early development.** FreeLane is not a usable Windows virtual machine yet. Windows boot, accelerated graphics, game compatibility, and seamless app windows are milestones to build and verify. No performance advantage over Parallels has been demonstrated.

## Why FreeLane?

Running Windows on a Mac should have a free, open-source option designed with Apple Silicon in mind. FreeLane aims to make that option practical: a small host application, a Rust virtualization core, and a clear path from a working Windows ARM guest to playable games.

The first target is **Apple M1**. Gaming comes first; a lightweight interface and convenient desktop integration follow a proven engine.

## What we are building

| Goal | What success looks like |
| --- | --- |
| Windows on Apple Silicon | Install and boot a user-supplied Windows 11 ARM image |
| Smooth gaming | Stable frame pacing, useful 1% lows, and responsive input in tested games |
| Low overhead | Measure host and guest memory, idle CPU, swap, and energy use |
| Free, open-source host software | No FreeLane subscription; inspectable and buildable source |
| Native desktop experience | Eventually launch Windows apps from macOS and show individual app windows |
| Reproducible comparisons | Publish configurations, workloads, results, and known failures |

Being lighter and more compatible than Parallels is a **target**, not a present capability. Rust alone does not make a VM faster or reduce guest memory requirements.

## Architecture direction

The design is provisional. Feasibility experiments should decide the architecture before a large user interface is built.

- **Host:** macOS on Apple Silicon, starting with M1.
- **Core:** Rust VM lifecycle, configuration, memory ownership, and device boundaries.
- **CPU virtualization:** investigate Apple's Hypervisor.framework with a small native backend.
- **Guest:** Windows 11 ARM, with firmware and virtual devices selected after compatibility and license review.
- **Graphics:** an early research track for a Windows ARM virtual GPU driver and a macOS rendering backend; DirectX-to-Metal translation is a possibility to investigate.
- **Integration:** guest tools for application launch, clipboard, shared folders, and later individual-window presentation.

Windows support requires much more than starting a virtual CPU: firmware, interrupts, timers, storage, input, networking, and compatible guest drivers all need implementation or suitable existing components. The engine and dependencies are not selected yet.

**Graphics is the highest-risk milestone.** Apple GPU passthrough is not assumed. A graphics prototype must demonstrate a viable driver and rendering path before gaming promises become product claims.

## Roadmap

| Milestone | Deliverable | Exit condition |
| --- | --- | --- |
| 0 — Foundation | Rust workspace, validated VM configuration, lifecycle boundaries, and host diagnostics | Portable tests pass; unsupported hosts fail clearly |
| 1 — CPU proof | Small Hypervisor.framework experiment on an Apple Silicon Mac | Guest code executes and exits correctly on real hardware |
| 2 — Graphics feasibility | Documented Windows ARM driver and rendering experiment | Demonstrate a supported graphics workload and record limitations |
| 3 — Windows boot | Firmware, disk, display, input, and networking | A reproducible Windows ARM install boots reliably |
| 4 — Gaming baseline | A small published game compatibility suite | Repeatable frametime and resource measurements on the same Mac |
| 5 — Efficiency | Tune measured bottlenecks; explore save and restore | Demonstrated improvements without compatibility regressions |
| 6 — Desktop integration | Guest tools and individual Windows app windows | Applications launch and interact reliably from macOS |

These are deliverables, not release-date promises. CPU and graphics experiments may proceed independently; a working GPU path is a gate for the gaming goal.

## Build and try

There is no installer, runnable VM, or supported build procedure yet. Build commands and host requirements will be added alongside the first verified implementation.

Until then, this repository documents the intended scope. Do not expect Windows boot or playable games from the initial foundation.

## How we will measure performance

Compare FreeLane and Parallels on the **same Mac, macOS, Windows build, vCPU count, guest RAM, game version, resolution, and graphics settings**. Record warm-up, workload duration, power conditions, and repeated runs.

Publish:

- Average FPS, 1% low FPS, frametime distribution, and stutter counts.
- Input latency and rendering or shader-related stalls where measurable.
- Host and guest memory, whole-system memory pressure, and swap.
- Idle CPU, boot and restore duration, and energy use where measurable.
- Crashes, visual defects, and per-game compatibility results.

Report failures alongside successes. No invented FPS numbers or projected memory savings.

## Compatibility boundaries

- Windows ARM's x86/x64 translation affects game compatibility and performance independently of the VM core.
- Kernel drivers and anti-cheat may prevent games from running. FreeLane will not promise universal game support or anti-cheat bypasses.
- Pausing a VM does not automatically return its allocated RAM. Releasing memory through save/restore requires explicit engine support and validation.
- Seamless app windows still depend on a running or restored Windows guest; they do not eliminate the guest operating system.

## Contributing

Early contributions are most useful when they reduce an unknown:

1. Small, reproducible Apple Silicon virtualization experiments.
2. Windows ARM firmware, device, or driver compatibility evidence.
3. Virtual GPU and rendering prototypes with explicit constraints.
4. Benchmark tooling and documented game failures.
5. Focused Rust changes with meaningful tests and clear error handling.

Before adding a dependency, record its license, maintenance status, supported platforms, and Windows guest limitations. Keep proposals small enough to review and reproduce.

## License and Windows

An open-source license for FreeLane will be selected and included before code is released for reuse. This README does not itself grant a software license.

FreeLane's intended free pricing applies to the host application. Users must obtain Windows and any games separately and comply with their licenses. Windows images, proprietary drivers, and game files will not be bundled without appropriate redistribution rights.

## Name and affiliation

FreeLane is an independent project. It is not affiliated with Apple, Microsoft, or Parallels. Product names belong to their respective owners. The working project name has not undergone trademark clearance.

---

**Build in the open. Measure on real hardware. Make Windows on Mac more accessible.**
