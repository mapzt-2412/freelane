FreeLane v0.1.0-preview.13 for Apple Silicon Macs (macOS 14+).

This P2 preview adds explicit file transfer into an opt-in guest inbox, cancellation/conflict preservation and optional text-document opening. The roadmap remains in progress.

- In Display, Send file to guest opens a file picker with upload progress and Cancel transfer. The updated guest agent requires an existing inbox selected with `--transfer-root`. Transfers verify declared size/SHA-256 and publish the exact file handle without overwriting existing files. Unicode/spaces are supported; traversal and Windows reserved names are refused.
- Open text document after transfer supports txt/csv up to32MiB, using system Notepad on Windows or installed gnome-text-editor/gedit on graphical Linux. No script/executable associations are used. File receipt/hash checks precede the explicit opening request. Ubuntu Server can receive files without a desktop; opening requires a GUI editor.
- Agent protocolv5 is opt-in; versions1–4 remain compatible. One transfer per agent,16KiB chunks,64GiB maximum per file. This is not a VM disk-size limit. No automatic mutation retries. Unsupported anonymous-file/hard-link filesystems fail closed; Linux ext4 and Windows NTFS are the intended guest storage backends. Pre-upload hashing currently has no progress display.
- Clipboard Send/Copy, verified Windows agent lifecycle and the single automatic VM setup flow from preview.12 remain available. Background clipboard synchronization and live shared-folder synchronization are still pending.

Validated:136 local workspace tests passed/8 ignored,11MCP tests, strict clippy, build and Mac ARM64 core check passed. A real native Linux stdio agent and host CLI transferred3MiB with a Vietnamese filename and matching SHA-256; conflicting originals and canceled sessions were preserved. Native Windows CI passed transfer storage, document refusal and Windows agent lifecycle checks after correcting handle publication. Fresh review exposed staging substitution and renamed-document issues; regression tests failed before fixes and passed afterward. [Evidence](https://github.com/puma2k/freelane/blob/main/docs/evidence/p2-file-transfer.json).

Mac HVF/Dock/sleep and physical-device tests remain deferred. Normal-user Windows guest startup and interactive document-editor acceptance on a real Mac/Windows guest remain pending. Clipboard auto-sync, continuous Coherence video,3D/Metal and universal Windows application compatibility are not accepted by this preview.

The Mac package is ad-hoc signed and not notarized. Verify release checksums and read [file transfer instructions](https://github.com/puma2k/freelane/blob/main/docs/file-transfer.md). The Windows installer does not yet add the inbox argument automatically to login startup. An abruptly terminated Windows agent may retain a staging file; inspect only after stopping it.

Tiếng Việt: Có gửi file, tiến độ, hủy, xác minh SHA-256 và giữ nguyên file trùng tên. Agent mới cần chỉ định thư mục nhận. Có tùy chọn mở txt/csv bằng trình đọc cố định; Ubuntu Server nhận file nhưng cần GUI để mở văn bản. Chưa đồng bộ nền, chưa hoàn tất Coherence và kiểm thử Mac thật.
