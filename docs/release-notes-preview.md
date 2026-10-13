FreeLane experimental preview for Apple Silicon Macs (macOS 14 or newer).

- Download the `macos-arm64.dmg`, open it and drag `FreeLane.app` onto the `Applications` shortcut. Eject the disk image and open FreeLane from Applications. The ZIP remains available as an alternative.
- For Windows app windows, also download the Windows ARM64 agent ZIP and install the updated agent inside the guest using the included per-user installer. See `docs/app-windows.md` in the Mac package.
- SHA256 files accompany all packages.

Includes a refreshed Mac-style interface with blue accents, a compact sidebar, OS icons, centered settings dialogs and optional local macOS system typography; Windows/Ubuntu VM configuration, Vietnamese and English UI, mobile environment controls, experimental separate Windows app snapshot viewers and per-app Dock launcher export.

This is an ad-hoc signed preview, without Apple notarization. QEMU, firmware, Windows/Ubuntu images and mobile SDKs are separate prerequisites; follow the included `START-HERE.md`. An iOS Simulator requires macOS/Xcode and does not run arbitrary App Store binaries.

Mac packaging, architecture, bundle signatures and native offline Dock export are checked by the release workflow. Real Windows Notepad client capture, targeted text entry, host UI text submission and live viewer display were verified under Linux ARM emulation in a Windows Audit session. Closing the host viewer preserved the guest app and VM. A guest-reported snapshot timeout gets one bounded read-only retry; input and transport failures are never replayed. Normal-user startup and Windows/Dock interaction on a physical Mac remain unverified. Snapshot viewers have limited input and refresh rate; full Parallels Coherence, GPU/USB passthrough and universal Windows app compatibility are not established. Required Rust/ARM CI passed for the readiness/wizard revision. Physical-Mac guest/runtime acceptance is pending.

Tiếng Việt: Đây là bản thử nghiệm cho Mac Apple Silicon. Mở DMG và kéo FreeLane vào Applications; nếu dùng cửa sổ app Windows riêng, cài agent ARM64 mới trong Windows. Cần chuẩn bị QEMU/firmware và bộ cài hệ điều hành riêng. Chưa kiểm thử đầy đủ thao tác Windows và mở từ Dock trên Mac thật.

New in this candidate: structured read-only `doctor-json`, first fresh-workspace creation wizard/`vm-create`, and private persisted VM bookmarks. Workspace creation does not install or boot an OS; automatic media verification, disk-space checks and full checkpoint/recovery acceptance is unfinished. See `docs/vm-creation.md` and `docs/mac-acceptance.md` in the package.

Offline checkpoint/recovery-copy CLI is now available for clean standalone qcow2 workspaces. It preserves sources and only restores to fresh directories, with checksums, startup markers, read-only checkpoint files and atomic no-replace publication. Linux/Mac file-roundtrip CI passed; no real Windows/BitLocker restore or live/memory snapshot is claimed. See `docs/checkpoints.md`.
