# Roadmap execution status

Updated 2026-10-13. The roadmap is not complete.

| Area | Status | Evidence / remaining work |
|---|---|---|
| P0 required CI | Passed for current readiness/wizard revisions | [P0](https://github.com/puma2k/freelane/actions/runs/38341927694), [P1](https://github.com/puma2k/freelane/actions/runs/38343054624), [bounded preflight](https://github.com/puma2k/freelane/actions/runs/38343264763) |
| P0 configuration diagnostics | Implemented and tested | `doctor-json`; missing/oversized config, executable discovery, host acceleration constraints, real Windows/Ubuntu preflight; no VM execution |
| P0 real Mac/HVF/Dock/sleep baseline | Deferred by user | Mac available later; no runtime pass claimed |
| P1 fresh workspace creation | First path implemented and tested | UI form + `vm-create`; real qemu-img 64 GiB creation, repeat refusal, source NVRAM preservation; guest installation/boot not run |
| P1 persistent VM library | Implemented and tested | Private/atomic bookmark updates, corrupt file preservation, actual UI restart without config arguments |
| P1 full wizard/recovery | In progress; not accepted | File pickers and ARM64 UEFI-loader inspection implemented; physical dialog acceptance, full guest checks, installation readiness/progress, repair/recovery and interruption recovery remain |
| P1 checkpoint/recovery-copy foundation | Implemented; limited acceptance | CLI full copy + fresh-directory restore; file-level tests Linux/Mac passed; real Windows/BitLocker recovery blocked by cloud capacity and pending native testing |
| P2 clipboard/sharing/agent lifecycle | In progress | Read-only v3 capability discovery implemented; clipboard/transfer and agent lifecycle remain pending |
| P3 app video/Coherence direction | Not started beyond existing snapshot prototype | Requires transport/driver feasibility and native Mac evidence |
| P4 complete snapshot/clone/backup UX | Not started | Consistent disk/NVRAM/TPM transactions precede restore/clone |
| P5 optimization | Not started | Measure a native baseline before performance claims |
| P6 audio/USB/graphics | Not started | Requires physical hardware and backend/driver go/no-go decisions |
| P7 expanded MCP/stable distribution | Not started beyond existing MCP/previews | Follows accepted primitives; notarization requires secure signing access |

## Changes delivered

- Installed CI process utilities needed by the ARM signal test.
- Replaced repeated socket receive-timeout changes with a poll-based absolute read deadline; EOF and timeout tests preserve bounded responses and no input replay.
- Made accepted test sockets explicitly blocking for macOS/BSD listener inheritance.
- Added structured, read-only VM preflight with input size limits and actionable failures.
- Added first fresh-VM workspace creation and persistent library paths.

Next work: P1 installation readiness and interruption recovery; native dialog/guest acceptance deferred. The checkpoint foundation is now implemented; full guest restore acceptance requires sufficient storage. Native acceptance remains pending; these statuses do not authorize calling P0/P1 fully accepted or jumping to claims of complete Coherence.

[Checkpoint scope and tests](../checkpoints.md). Latest completed checkpoint CI: https://github.com/puma2k/freelane/actions/runs/38348621836 (Linux/Mac file roundtrip and required checks passed).

Creation now rejects insufficient destination capacity before backend execution or directory creation. The conservative check includes full virtual disk growth; sparse allocation is not a reservation.

P1 media check passed on the actual supplied Windows ARM64 ISO (UDF bridge, extracted ARM64 PE bootloader). Synthetic ISO tests cover wrong architecture, missing loader and invalid extents. This is not a fresh guest installation. P0 owner checklist: [Mac acceptance](../mac-acceptance.md).

P1 runtime observation: `vm-readiness` and desktop Refresh status distinguish QMP state from agent connectivity; installation completion stays unverified. QMP socket and absent-backend tests preserve configuration and avoid creating runtime state. Guided installation/recovery remain outstanding.

P1 interruption foundation: protected staging, no-replace publication and `creation-recover` for prepared never-started workspaces implemented. Tests cover backend failure, a simultaneous destination collision, recovery, corrupted files and early-stage refusal. These use nonbootable file fixtures; native wizard creation and OS-installation recovery remain unverified.

P1 wizard resources/connectivity/progress: implemented and tested. Actual Linux UI created a 16 GiB Ubuntu workspace using signature/checksum-verified Ubuntu 24.04.5 media, saved its library entry and reopened the configuration. No guest start or installation was performed. Full workspace: 83 passed, 6 ignored. Native acceptance and guided OS installation remain pending.

P1 review fixes: request parsing now uses a nonblocking regular-file descriptor and capped read; UDF fallback explicitly rejects generic archives; read-only recovery sources are rejected before publication and post-publication marker failures have an explicit committed/warning result. These were reproduced with FIFO, ZIP-with-ARM64-loader and read-only-source regressions before fixing.

P1 settings templates: versioned mutable-settings export/import implemented; private no-overwrite export, existing-VM backups, busy-lock refusal and storage identity preservation tested. UI import previews before Save changes. Native file dialogs and full backup/relocation acceptance remain pending.

Settings UI acceptance on Linux: preview preserved the saved file, export used saved values while the draft differed, Save applied the requested CPU/RAM and preserved storage hashes. Dialog selection used a test-only zenity stub; native dialogs remain pending. Full workspace: 92 passed, 6 ignored; clippy and Apple Silicon core cross-check passed. [Evidence](../evidence/settings-bundles.json).

P1 library path recovery: explicit missing-bookmark relink in CLI and localized UI, with guest-file preservation and rejection tests. Linux UI exercised the repair with a test-only dialog stub. Full suite 94 passed, 6 ignored; clippy passed. Runtime migration and native guest relocation remain unverified. [Guide](../library-recovery.md).

P1 Ubuntu cloud image mode: independent qcow2 import added to CLI/wizard, growth/capacity checks and private source snapshots; ISO requests stay compatible. Linux UI created a VM from a signature/checksum-verified official ARM64 image; first boot reported Ubuntu 24.04/aarch64 and reboot preserved a guest marker. Native Mac and agent/desktop installation are unverified. Automated boot/reboot re-test is pending after correcting the test seed's early shutdown timing. 99 workspace tests and 11 MCP tests passed, clippy and Mac ARM64 core cross-check passed. [Guide](../cloud-images.md).

Cloud guest automated acceptance now passed on a fresh Linux TCG VM: aarch64/Ubuntu 24.04 observed, exact marker value survived reboot, two clean shutdowns and unchanged input image. Fixing Mac CI canonical-path assertions and duplicate alias handling; Linux/ARM jobs at fa22ec7 passed. Full workspace 100 passed, 6 ignored.

Ubuntu guest checkpoint acceptance: offline checkpoint created after clean shutdown, restored into a fresh workspace, original NoCloud test seed reattached through settings import, exact persisted guest marker observed after boot, clean shutdown and unchanged source VM files verified. This is Linux TCG Ubuntu evidence; Windows TPM/native Mac acceptance remains pending. [Evidence](../evidence/ubuntu-cloud-checkpoint-guest.json).

Required CI for code commit 1a5405c passed, including Linux/Mac portable suites, real ARM checks, guest-agent builds and Mac checkpoint file roundtrip: https://github.com/puma2k/freelane/actions/runs/38384374715 . Native Mac guest/HVF/Dock acceptance is still deferred.

P1 reviewable preview published: v0.1.0-preview.6, source a50687e. Mac app/DMG and Windows ARM64 agent workflow passed; the public DMG was downloaded and its SHA256/UDIF footer independently checked. [Release](https://github.com/puma2k/freelane/releases/tag/v0.1.0-preview.6), [package evidence](../evidence/release-preview6.json). This is an ad-hoc preview and does not accept P0/P1 native runtime or complete P2–P7.

P2 capability discovery: read-only v3 metadata and bounded v3→v2→v1 fallback implemented in CLI and asynchronous desktop Details probe, retaining legacy unknown semantics. 106 workspace tests passed, 6 ignored; 11 MCP tests, clippy, desktop build and Apple Silicon core cross-check passed. Linux/Windows ARM64 agents built. Real v3 virtio discovery and separate UID1000 execution checks passed on controlled Ubuntu ARM64 TCG; v1 Ping and v2 capabilities also passed. Clipboard, transfer and lifecycle are pending. Preview.6 predates this change.

P2 real capability evidence: controlled Ubuntu guest returned bounded v3 metadata through virtio; v1/v2 read-only replies and UID1000 launch checked separately. Actual English/Vietnamese Details probes displayed connected state. A missing Vietnamese availability label was corrected and rebuilt after screenshot inspection. Guest shutdown completed cleanly. [Evidence](../evidence/agent-capabilities.json). This accepts discovery only, not the complete P2 milestone.

Mac wizard onboarding: discovers a coherent Homebrew QEMU/firmware installation, fills empty technical fields without replacing manual choices, hides paths in Advanced components, and offers fixed QEMU/swtpm installation through a background worker. Installer timeout terminates its owned process group. 109 tests passed/6ignored, clippy/build/Mac core cross-check passed; real Linux English/VI wizard screenshots use a local Homebrew-shaped fixture. Actual Homebrew installation/native Mac boot remain unverified; Homebrew bootstrap and a fully bundled backend remain pending.

Onboarding follow-up includes Homebrew 7-Zip and Finder-safe media-tool discovery, required for Windows UDF ISO inspection. Full 109-test suite/clippy/build/Mac core recheck passed. Preview.8 supersedes preview.7 with this prerequisite fix.

Preview.8 published from 0851c83: native Mac package/DMG verification and Windows agent packaging succeeded, public DMG SHA256 and UDIF footer independently verified. [Release](https://github.com/puma2k/freelane/releases/tag/v0.1.0-preview.8), [evidence](../evidence/release-preview8.json). Actual Homebrew installation and physical Mac guest acceptance remain pending.
