FreeLane v0.1.0-preview.9 for Apple Silicon Macs (macOS 14 or newer).

Download the `macos-arm64.dmg`, drag `FreeLane.app` to Applications, eject the image and open the app. ZIP and Windows ARM64 companion-agent packages are also available, with SHA256 files. QEMU, firmware, guest images and mobile SDKs remain separate prerequisites; follow the packaged `START-HERE.md`.

This preview adds:

- Sparse VM disk creation checks initial storage rather than requiring the entire virtual maximum free. The arbitrary 4 TiB app cap is removed; guest minima and backend representation/format limits remain.
- Destination free-space display, worker-based fresh folder selection, automatic setup refresh and pinned-directory creation/publication guards. External storage is used when selected; drives are not pooled automatically. [Setup guide](https://github.com/puma2k/freelane/blob/main/docs/setup.md).

- Automatic discovery of installed QEMU and ARM firmware, with technical paths tucked into Advanced components. Macs with Homebrew can install QEMU/swtpm/7-Zip from the wizard using a fixed command in a background worker; Finder launch resolves one coherent installed toolchain. Homebrew itself remains an external prerequisite; QEMU is not bundled. Actual Homebrew installation/native Mac guest execution remain unverified.
- Read-only v3 guest-agent capability discovery in CLI and Details, retaining v1/v2 compatibility. Real Ubuntu ARM64 virtio discovery, UID1000 execution and English/Vietnamese UI probes passed.

- Windows/Ubuntu creation presets, network/share options, native file dialogs and actual setup progress. ISO checks verify an ARM64 UEFI loader; UDF media requires 7-Zip.
- Ubuntu ARM64 cloud-image selection, private standalone qcow2 conversion and disk growth without changing the source image. Optional NoCloud media configures first boot; architecture/authenticity must be confirmed separately.
- Versioned mutable-settings import/export with preview, saved-versus-draft distinction, backups and preserved disk/firmware/NVRAM/TPM identity.
- Explicit recovery of missing VM bookmarks, including canonical aliases; protected staged creation/recovery; offline checkpoints restored only into fresh workspaces.
- Runtime observations that separate backend state and guest-agent connectivity from unverified installation completion.

The earlier P1 code revision passed required Linux/Mac/ARM CI: https://github.com/puma2k/freelane/actions/runs/38384374715 . Linux TCG acceptance used a signature/checksum-verified Ubuntu Server ARM64 image: UI creation, observed aarch64/Ubuntu first boot, exact guest-data persistence after reboot, clean shutdowns, unchanged image source, and guest marker recovery after checkpoint restore. Native Mac guest/HVF/Dock, Windows protected-storage/TPM restore and Ubuntu Desktop acceptance remain pending.

The existing Vietnamese/English interface, Android Emulator/Xcode Simulator management, experimental Windows app snapshot viewers and Dock launcher export remain available. App viewers are not full Coherence or a video transport. Universal Windows compatibility and superior performance have not been demonstrated.

This is an ad-hoc signed, unnotarized prerelease. NoCloud seed structure/contents are not automatically verified; use a seed you intend to run. External seed/ISO media is not included in checkpoints and may need explicit reconnection.

Tiếng Việt: Preview này thêm tạo Ubuntu từ ảnh cloud ARM64, nhập/xuất cấu hình có xem trước, tìm lại VM đã chuyển thư mục và khôi phục workspace/checkpoint. Đã kiểm thử boot, giữ dữ liệu qua reboot và phục hồi marker guest trên Linux; CI Linux/Mac/ARM đều qua. Kiểm thử guest trên Mac thật và Windows TPM vẫn để sau. Bản DMG ký ad-hoc, chưa notarize; cần QEMU/firmware riêng và seed NoCloud phù hợp nếu dùng ảnh cloud.

Current local validation: 116 workspace tests passed (6 ignored), clippy, desktop build and Apple Silicon core cross-check. Installer timeout regression uses an owned fake installer and verifies descendant termination; this does not test actual Homebrew downloads. Native package publication has its own workflow checks.

Sparse acceptance: real Linux UI created a Windows128GiB workspace with16.7GiB free; initial disk allocation200704 bytes. Real QEMU8TiB sparse creation and directory replacement guards passed. These checks do not establish guest installation, runtime behavior when the selected disk fills, physical SSD detach or native Mac execution.
