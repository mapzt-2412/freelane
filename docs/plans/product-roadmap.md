# FreeLane: implementation and completion roadmap

Status: proposed implementation plan, 2026-10-13. Planning does not establish that any feature below is implemented or tested. Superpowers was requested, but its tools/skills were unavailable; this plan was prepared directly against the repository.

## Product objective and scope

Build a free VM/app workspace for Apple Silicon Macs, with Windows ARM and Ubuntu ARM64 as the primary guests. Android Emulator and Xcode Simulator remain managed SDK backends. Prioritize reliable installation, data preservation and ordinary desktop apps before advanced graphics or additional guest platforms.

Universal Windows compatibility and outperforming Parallels are research goals, not acceptance criteria that can presently be promised. Publish a tested compatibility matrix and comparable performance measurements instead. FreeLane's license does not remove Windows, SDK, application, certificate or distribution licensing requirements.

## Starting point

- Rust host CLI, egui desktop manager, Windows/Linux agents and 14-tool stdio MCP server exist.
- Mac ARM64 ZIP/DMG packaging, ad-hoc signatures and offline Dock export passed on Mac CI; physical-Mac guest and Dock behavior remain unverified.
- Windows cloud acceptance demonstrated Notepad client capture, targeted input, live host viewer input and preservation of the guest when the viewer closes. This used a Windows Audit session, not a normal-user startup installation.
- Ubuntu read/write 9p sharing and basic Virtio 2D have recorded Linux guest evidence. Windows sharing currently uses read-only USB media.
- App viewers use bounded JPEG snapshots. Clipboard, video, automatic window tracking and complete keyboard support are unfinished.
- Broad CI has unresolved Mac Rust/ARM integration failures. The locally passing suites are not a full CI pass.

## Delivery sequence

P0 → P1 → P2 → P3 → P4 → P5 → P6 → P7. P4 starts with the stopped-VM checkpoint foundation during P1; its complete user experience follows P3. Hardware/graphics feasibility work may start early but must not block the basic release. Each milestone ends with a working build, evidence and a reviewable release candidate. Fix failed acceptance checks before advertising the affected capability.

### P0 — Establish reliable development and real-Mac evidence

1. Diagnose and fix remaining Mac Rust and ARM integration CI failures without weakening assertions or publishing unrestricted logs.
2. Add repeatable host/backend diagnostics: architecture, QEMU, firmware, TPM, guest-agent compatibility and relevant permissions; actionable errors with bounded diagnostic export.
3. Establish an Apple Silicon test host, starting with one supported macOS release; record Mac model, OS, RAM and backend versions. A Mac with 16 GiB RAM is preferred for combined host/Windows tests.
4. Verify Windows/Ubuntu boot with HVF, start/pause/resume/shutdown, host sleep/wake, network, actual display/input and Finder/Dock launch.
5. Test the Windows agent in a normal interactive account: install, login startup, logout/login, update and uninstall. Keep secure desktops and UAC failures explicit.
6. Measure a baseline before tuning: cold boot, app launch, frame delivery, idle/load CPU, memory and host responsiveness.

Acceptance: required CI passes with intended tests executed; the physical-Mac matrix has reproducible evidence. Linux emulation results cannot substitute for HVF, Finder/Dock, sleep or device tests. Cloud-independent work continues while Mac access is pending.

### P1 — VM creation and recovery foundation

1. Add a VM wizard: guest type, legitimate ISO/cloud-image selection, resource presets, storage location, firmware/TPM and network/share options.
2. Validate image architecture, format, capacity and available disk space before writing; display exact destination and prerequisite failures. User-selected existing images are preserved.
3. Show real backend/setup progress and distinguish started, booting, ready, failed and unknown. Do not infer installation success from elapsed time or a PID.
4. Persist the VM library and recover connections to existing supervisors after manager restart; prevent duplicate starts and duplicate installation requests.
5. Add stopped-VM checkpoint transactions containing disk state, firmware variables, TPM state and configuration. Validate manifests and atomically finish or roll back the transaction. Start with offline checkpoints; defer live/memory snapshots.
6. Add import/export configuration and guided recovery with backups; bundle path handling must survive valid moves or explain how to repair them.

Acceptance: create Windows and Ubuntu from the UI on Mac, reboot into the installed guest, reopen the manager and retain VM identity. Interrupted creation/checkpoint operations preserve originals and leave recoverable state.

### P2 — Clipboard, file sharing and agent lifecycle

1. Version and negotiate agent capabilities; report minimum compatible agent version and provide explicit install/update/uninstall paths with integrity verification and recovery.
2. Implement Unicode text clipboard in both directions, with an opt-in connection setting, size limits, source identifiers to avoid echo loops and disconnect handling. Add images/rich text after plain text works.
3. Evaluate a Windows ARM sharing backend: supported SMB or an agent transfer service; retain Ubuntu 9p where supported. Decide after checking ARM compatibility, Mac packaging and network isolation.
4. Provide folder selection, read-only/read-write choices, guest mount status and reconnect behavior. State whether the backend is a live shared mount or file synchronization.
5. Add file transfer with staged writes, completion checks, cancellation and conflict handling. Do not silently replace a changed destination or interpret filenames as shell commands.
6. Add host-to-guest document opening through literal paths after transfer/mount readiness is confirmed.

Acceptance: Vietnamese text copy/paste both ways; large-file hash matches; spaces/Unicode filenames; reconnect and permission failures; simultaneous edits and canceled transfers. Agent startup/update passes in a normal Windows user account. No clipboard content or file contents appear in routine logs.

### P3 — Seamless app workspace (Coherence direction)

1. Improve app discovery: installed-app catalog, executable metadata, extracted icons and persistent per-app profiles.
2. Track window creation, closure, dialogs and process delegation with fresh process/window identities. Resolve single-instance app ownership without blindly launching again.
3. Add reliable modifiers/shortcuts, scroll, double-click, drag and resize with clear focus/coordinate behavior; avoid global guest input when a targeted path is available.
4. Prototype a video transport separate from the existing 64 KiB control RPC. Compare agent capture/encoding, SPICE and RDP/RemoteApp only after verifying Windows ARM support, required drivers and licensing. Select based on measured latency, image quality and CPU use.
5. Implement bounded queues, frame dropping, congestion handling, backpressure and capability fallback. Never replay uncertain input or launch requests.
6. Integrate clipboard/document transfer into app viewers; handle cross-process dialogs and unsupported secure/GPU windows explicitly.
7. Finish per-app Dock identity/icon, quick-open/re-attach, multiple viewers and upgrade-safe launcher regeneration. Closing a viewer preserves guest apps unless the user explicitly requests closure.

Acceptance: a declared suite of ordinary apps, including classic Notepad and 7-Zip, exercises typing, shortcuts, menus, dialogs, resize and reconnect on Mac. Include a delegated/single-instance case. Measure frame rate and median/p95 input-to-visible latency on specified hardware; choose numerical release targets from P0 measurements before implementation is declared complete. Until the native/video suite passes, label the capability an app viewer rather than full Coherence.

### P4 — Snapshots, clones and backup UX

1. Build the snapshot/restore UI on the P1 stopped-VM checkpoint format, with names, timestamps, size estimates and explicit restore consequences.
2. Verify Windows disk + NVRAM + TPM consistency across restore; preserve a recovery checkpoint before a destructive restore.
3. Clone into a distinct VM identity, using supported copy-on-write storage where appropriate. Address guest identity/network collisions and TPM handling before claiming an independent clone.
4. Add full backup export/import with a manifest, checksums, backend compatibility and relocatable paths.
5. Handle low disk space, missing/corrupt components, interrupted writes and busy VM locks. Add live snapshots only after a backend-specific consistency design is proven.

Acceptance: saved guest data reappears after restore/reboot; originals survive failed restore; Windows protected storage behavior is validated; clone and source run independently. Backup restoration is tested, not just archive creation.

### P5 — Resource optimization and observability

1. Add balanced/performance/battery presets with visible CPU/RAM values and restart requirements.
2. Add actual resource telemetry and per-VM limits using supported host/backend controls. Separate configured RAM from observed host consumption.
3. Tune snapshot/video scheduling, encoding, idle work and multi-viewer coordination using P0/P3 measurements.
4. Add low-storage/pressure handling and host sleep policy without silently terminating guests.
5. Maintain reproducible benchmarks for boot, launch, input latency and resource use; compare against Parallels only on equivalent hardware, guests and workloads.

Acceptance: measured improvement over the recorded FreeLane baseline without data-loss or lifecycle regressions. No speed/battery claims based only on release compiler optimization.

### P6 — Audio, USB and graphics feasibility

1. Verify audio output, microphone selection, permissions, reconnect and mute behavior using a Windows ARM-compatible backend/driver. Show available capabilities by guest.
2. Complete USB selection with friendly identity, exact device ownership, attach/detach, hotplug and conflict errors. Test representative physical storage/input devices; do not claim arbitrary-device support.
3. Evaluate Ubuntu 3D and Windows ARM virtual graphics drivers/backend options on Mac. Keep current Virtio 2D scope clear.
4. Treat physical Apple GPU passthrough as a separate feasibility decision requiring actual host support. If unsupported, document that result and pursue supported virtual/remote acceleration.

Acceptance: actual Mac devices and guest drivers work in the published test matrix. Each graphics branch has a written go/no-go result before a performance commitment or UI promise.

### P7 — MCP automation and product release

1. Extend registered MCP tools for wizard-created environments, readiness, capability discovery, app catalogs, checkpoints, restore and bounded diagnostics. Separate read-only operations from mutations.
2. Implement disposable test environments and explicit cleanup ownership; never erase a user VM as part of test cleanup. Return job IDs, real exit codes and artifact provenance.
3. Supply connection examples for clients supporting stdio MCP; document client-specific restrictions rather than promising every AI editor works identically.
4. Polish first-run UX, library search, recent apps, accessibility, small-window layouts, Vietnamese/English coverage and error recovery. Use actual UI captures for reviews.
5. Add versioned settings migration, update checking, download/signature verification and rollback. Avoid replacing a running binary or silently updating guest images/backends.
6. Add Developer ID signing and notarization when secure certificate/account access exists; keep credentials out of the repository and logs. Continue clearly labeled ad-hoc previews meanwhile.
7. Publish a Mac DMG, companion agents, checksums, compatibility matrix, installation/upgrade/uninstall instructions and release notes for each accepted milestone.

Acceptance: install and upgrade a release candidate on a clean Mac; run an MCP-directed test in a disposable guest, collect results and restore/clean up correctly; uninstall preserves explicitly retained user VMs. Required CI and the supported native-Mac matrix pass.

## Implementation boundaries

- Host/backend lifecycle and checkpoint transactions: `src/qemu.rs`, `src/lib.rs`, host CLI and new focused modules.
- Agent capabilities, input, transfer and clipboard: `crates/agent-protocol`, `crates/guest-agent`, `src/agent.rs`. Preserve v1 compatibility or reject unsupported operations before mutation.
- Desktop/wizard/library/app viewers: `crates/desktop`; retain asynchronous workers and responsive rendering.
- MCP: `scripts/freelane_mcp.py`, `tests/mcp`, real-guest smoke checks; only registered profiles and literal validated arguments.
- Release/build: existing scripts and `.github/workflows`; portable checks, cross-compilation, native packaging and real guest acceptance remain separate evidence categories.

## Execution and review rules

For each task: identify the failing/missing behavior, implement a focused change, exercise meaningful checks, record limitations, commit separately and produce a reviewable result. Keep tasks small enough to diagnose failures. New protocol or storage behavior needs meaningful failure-path tests; cosmetic edits need actual visual checks rather than tests that mirror styling.

Use Linux cloud for portable development and disposable QEMU/agent tests. Use Mac CI for native compilation/packaging. Physical-Mac acceptance requires a reachable Mac, OS media the user may use, interactive guests and representative peripherals. Publication signing additionally requires a secure Developer ID/notarization setup. No such hardware or credential access is assumed to exist.

Do not set completion dates until P0 exposes native testing capacity and P2/P3/P6 feasibility decisions. Track each phase as Not started / In progress / Blocked / Accepted, with links to commits, checks and real evidence. A successful build or screenshot alone does not complete a milestone.

## First execution batch

1. Resolve remaining CI failures; preserve test coverage.
2. Run/record the P0 Mac baseline as soon as a Mac is available.
3. Meanwhile implement diagnostic prerequisites and design the P1 wizard + offline checkpoint transaction format.
4. Ship a focused wizard milestone, then agent/clipboard/sharing, then the app-window/video milestone.
5. Keep P4–P7 in the backlog until their dependencies or feasibility gates are met.
