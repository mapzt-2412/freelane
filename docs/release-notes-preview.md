FreeLane experimental preview for Apple Silicon Macs (macOS 14 or newer).

- Download the `macos-arm64.dmg`, open it and drag `FreeLane.app` onto the `Applications` shortcut. Eject the disk image and open FreeLane from Applications. The ZIP remains available as an alternative.
- For Windows app windows, also download the Windows ARM64 agent ZIP and install the updated agent inside the guest using the included per-user installer. See `docs/app-windows.md` in the Mac package.
- SHA256 files accompany both packages.

Includes Windows/Ubuntu VM configuration, Vietnamese and English UI, mobile environment controls, experimental separate Windows app snapshot viewers and per-app Dock launcher export.

This is an ad-hoc signed preview, without Apple notarization. QEMU, firmware, Windows/Ubuntu images and mobile SDKs are separate prerequisites; follow the included `START-HERE.md`. An iOS Simulator requires macOS/Xcode and does not run arbitrary App Store binaries.

Mac packaging, architecture, bundle signatures and native offline Dock export are checked by the release workflow. A real Windows Notepad client capture was tested on Linux ARM emulation; host JPEG display was checked separately with a read-only replay. Live Windows app interaction and Dock launch on a physical Mac remain unverified. Snapshot viewers have limited input and refresh rate; full Parallels Coherence, GPU/USB passthrough and universal Windows app compatibility are not established. Some broader MCP CI jobs have unresolved failures; this preview is not a claim that the entire CI suite passes.

Tiếng Việt: Đây là bản thử nghiệm cho Mac Apple Silicon. Giải nén bản Mac để mở ứng dụng; nếu dùng cửa sổ app Windows riêng, cài agent ARM64 mới trong Windows. Cần chuẩn bị QEMU/firmware và bộ cài hệ điều hành riêng. Chưa kiểm thử đầy đủ thao tác Windows và mở từ Dock trên Mac thật.
