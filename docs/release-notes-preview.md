FreeLane v0.1.0-preview.10 for Apple Silicon Macs (macOS 14+).

Open the DMG, drag FreeLane to Applications and open **Create virtual machine**. Windows chooses your ARM64 ISO; Ubuntu Server chooses an LTS version and downloads automatically. Click **Set up and start**. CPU/RAM/disk and technical paths stay in Advanced settings; storage can be on a mounted external drive.

Missing VM tools are installed in the same flow: QEMU, swtpm, 7-Zip and GnuPG. If Homebrew is absent, the app opens its bundled official bootstrap in Terminal. Complete macOS administrator/developer-tools/automation prompts locally. Host binaries are downloaded during setup; they are not bundled into the DMG. Windows ISO and Android/Xcode runtime setup remain separate inputs.

Ubuntu Server 26.04.1 and 24.04.5 ARM64 installers use official Ubuntu HTTPS sources, a bundled signing keyring, signed SHA256 manifests and exact ISO checksum/ARM64 verification. Download outputs and cleanup are bound to selected-storage directory handles. Owned background jobs terminate as a group when canceled. A Terminal bootstrap is separate; close its tab to stop it after cancellation. Canceled workspace finalization preserves a stopped VM instead of booting it.

Validated locally: 120 workspace tests passed, 7 ignored; one additional real Ubuntu download acceptance passed, 11 MCP tests passed; clippy, desktop/CLI build and Mac ARM64 core cross-check passed. Actual downloads/checks passed for both Ubuntu versions. Linux GUI creation, real Ubuntu Server GRUB boot and Windows workspace automatic QEMU start were observed after adding the missing cloud GTK dependency. File-picker probes use a test-only zenity stub. These checks do not establish completed guest installation. [Setup evidence](https://github.com/puma2k/freelane/blob/main/docs/evidence/quick-setup.json).

Actual Homebrew bootstrap on Mac, HVF/native guest execution, physical external-drive detach and Dock acceptance remain pending. Windows reliability and Android software-emulation System UI ANRs remain unresolved. This is an experimental, ad-hoc-signed package without Apple notarization. Experimental Windows app snapshot viewers are not full Coherence or live video streaming.

Tiếng Việt: Một nút setup tự cài công cụ còn thiếu, tải Ubuntu Server LTS và mở VM. Windows chọn ISO ARM64 của anh. CPU/RAM/đĩa nằm trong Cài đặt nâng cao; có thể lưu ở ổ ngoài. Mac vẫn có thể hiện yêu cầu quyền cài đặt trong Terminal. Chưa hoàn tất kiểm thử trên Mac thật hoặc khẳng định đã cài xong hệ điều hành guest.
