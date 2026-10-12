# Implementation status

Goal: a free, open-source alternative to Parallels for Windows applications and games on Apple Silicon, with improvements demonstrated by comparable measurements. Universal Windows application compatibility cannot be assumed, especially for kernel drivers and anti-cheat.

## Implemented and exercised on Linux

- Optional standard-library Python stdio MCP server for fixed build/test jobs and named VM controls, real PNG captures and guest-agent requests. Real stdio/UEFI startup, pause/resume, input requests and disconnect/adoption passed; the official MCP SDK also negotiated and exercised tools successfully. Configuration is provided for Codex, Cursor and Claude; their actual UI connections, Windows operations through MCP and native Mac remain separate checks. See [MCP setup](mcp.md).
- Native egui desktop manager: open existing configurations, start/status/pause/resume/power controls and bounded display snapshots, guest connection checks and literal app launch requests. Optional five-second display refresh and snapshot mouse clicks are implemented; actual installed Windows Terminal minimize/restore was observed. Linux GUI lifecycle checks start a real VM, pause/resume it, and verify that closing the manager leaves it running. Continuous video, keyboard forwarding and seamless app windows remain unimplemented.
- Rust CLI, strict TOML VM configuration, CPU/memory bounds and lifecycle boundaries.
- External QEMU backend with explicit HVF (Apple Silicon) or TCG (software) acceleration. HVF never silently falls back to TCG. Optional single/multi TCG threading supports concurrency diagnostics without changing guest CPU/RAM; single-threaded UEFI boot passed, Windows stability remains under investigation.
- Fresh sparse qcow2 disk creation, independent firmware/NVRAM copies, NVMe storage, USB keyboard/tablet and a firmware framebuffer (`ramfb`). Optional user-mode virtio or USB networking, disabled by default.
- TPM 2.0 through a managed swtpm process, private persistent state and a separate state lock.
- Private local QMP sockets, capability negotiation, bounded messages/deadlines, event filtering, response IDs and command errors.
- Foreground startup, status/pause/resume, guest powerdown requests, force stop, signal cleanup and process reaping. No Chromium or always-on daemon.
- Configuration locks prevent concurrent launches; QEMU retains disk-image locks. Disk, firmware, NVRAM and installer aliases (including hard links) are rejected.
- Virtio PCI devices use modern memory-mapped interfaces, with legacy port I/O disabled. Actual QMP enumeration in UEFI verifies serial device ID 0x1043 and absence of I/O regions.
- Opt-in virtio-serial guest channel, versioned/bounded RPC and host ping/launch CLI. A cross-linked Windows ARM64 agent ran in WinPE and the installed Audit session through real virtio serial with normal signed driver loading. Installed host ping returned Windows/aarch64; literal CLI launch opened 7-Zip with the intended archive, and the native FreeLane GUI opened Notepad. GUI launch also preserved spaces/ampersand in a WinPE archive argument. Normal user setup, automatic startup and native Mac remain unverified.
- Headless PPM screenshots that refuse existing destinations, typed QMP key chords and bounded USB-tablet mouse clicks. GUI snapshots optionally refresh every five seconds and accept primary clicks; actual Windows Terminal minimize/restore is verified. Lost mouse acknowledgments trigger release-only cleanup, with no replayed press. Real UEFI tests verify screenshot preservation and guest keyboard effects.
- 7-Zip 25.01 ARM64 created and extracted a real test archive in WinPE; individual source/extracted SHA-256 hashes matched the independently hashed original. This is a native ARM64 app experiment, not x64/x86 translation or an installed desktop.
- 7-Zip 25.01 x86, x64 and ARM64 completed an installed Audit archive trial with create/test/extract and exact source-byte matches. Older extracted x86 bytes also matched after reboot with exit0. An earlier unrequested reboot was diagnosed as bugcheck0x139 parameter1=3; offline native DbgEng found nt/WdFilter frames, not a proven cause. One single-threaded passing trial does not establish a crash fix or everyday stability.
- A measured frametime CSV analyzer: average FPS, 1% low, percentiles, worst frame and count over 50 ms.

Real integration tests execute AArch64 machine code and observe its UART marker. They boot EDK2 UEFI, require guest firmware log output and a framebuffer different from QEMU's initial placeholder, boot a locally built ARM64 EFI application from a read-only installer CD-ROM, execute an echo command inside the UEFI shell through USB keyboard input, verify TPM attachment, pause/resume, restart and template preservation. Another test drives the CLI as separate processes and checks cleanup on SIGTERM. These tests use **TCG, not HVF**.

The Apple Silicon host and Windows ARM64 guest-agent branches pass cross-target `cargo check`. This is not a macOS runtime test. GitHub Actions workflows are configured but have not been run in this session.

## Selected baseline engine

Use a separately installed QEMU 10.0+ process with the versioned `virt-10.0` machine. This supplies a mature ARM CPU/device model and HVF backend while the host/control layer remains Rust. It replaces the need to implement every firmware/device prerequisite before the first Windows boot experiment. The native Hypervisor.framework empty-VM probe remains a separate diagnostic experiment, not the production backend.

This baseline is **not an accelerated Windows graphics implementation**. `ramfb` is a firmware framebuffer, not a DirectX GPU. Virtio networking needs a compatible Windows ARM driver. CPU virtualization does not solve these guest-driver requirements.

## Outstanding gates

1. Verify installed-desktop stability and normal user setup. Windows ARM reached its installed Audit desktop, ran Windows Terminal/CMD and the official x86 console workload, and returned to that desktop after a subsequent unrequested reboot. Diagnose that reboot before claiming stable everyday use. A user-provided Microsoft CDN URL downloaded the ARM64 en-GB build 26300.9457 ISO. SHA-256 matched a Microsoft-published entry labelled Dutch, although the volume, WIM metadata and actual Setup are en-GB; this publication-label discrepancy is recorded, rather than claiming the expected English International hash matches.
2. Verify Windows security and devices after installation. Setup rejected generic non-Secure-Boot AAVMF for TPM/Secure Boot; signed Debian AAVMF_CODE.ms.fd plus AAVMF_VARS.ms.fd passed those checks, detected NVMe storage and started installation. No installer requirement bypass is implemented. USB network attachment is exercised; guest connectivity remains unverified.
3. Run on a real Apple Silicon Mac and validate HVF, display/input, guest reboot persistence and compatible Windows ARM drivers.
4. Implement and validate a Windows ARM graphics driver and macOS rendering path before claiming DirectX acceleration or gaming readiness.
5. Add guest tools for clipboard, shared folders and individual application windows; snapshot/restore must account for disk, firmware and TPM state.
6. Measure real application/game performance against Parallels on the same hardware and workload. No performance advantage has been measured.

Paused guests retain allocated RAM. Stop discards guest RAM; this implementation does not save or restore guest memory. FreeLane code is MIT licensed; separately installed components retain their own terms. See dependency notes before packaging a release.
