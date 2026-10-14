FreeLane v0.1.0-preview.14 for Apple Silicon Macs (macOS 14+).

This P2 preview adds opt-in automatic Unicode text clipboard synchronization with observed conflict detection. The roadmap remains in progress.

- In Display, enable Share text clipboard, then Automatically synchronize text for the selected VM. Both default off, and opening a VM resets consent. Manual Send/Copy remain available. Clipboard payloads never enter routine notices/logs.
- Initially different contents or observed edits on both sides pause synchronization and show a choice beside Send/Copy. Explicitly choose which content to retain. Equal text does not write; source identifiers suppress echo loops.
- Updated guest agents advertise protocolv6 conditional clipboard writes and a stable instance identity. The recipient checks its snapshot before applying an update. Disconnect/restart stops automatic mode until reenabled. Revoked queued requests are canceled before transmission; uncertain mutations are not retried.
- Native clipboard APIs do not support a global cross-process atomic compare-and-set. An unrelated application can still edit between the final check and native write. Manual controls remain available when per-transfer control matters. Text is limited to8KiB UTF-8; images/rich text remain pending.
- File transfers and txt/csv opening from preview.13, verified Windows agent lifecycle and automatic VM setup remain available. Live folder synchronization, continuous Coherence video and accelerated3D/Metal remain pending.

Validated:142 workspace tests passed/9 ignored,11MCP, strict clippy/build, Mac crosschecks and Windows ARM64 build passed. Separately executed native fixture tests used only predefined synthetic text on fresh authenticated X11 displays: Unicode push/pull, echo suppression, simultaneous conflict preservation and conditional refusal passed. Actual desktop acceptance covered initial conflict, explicit resolution and automatic pull. Fresh review found no Critical/Important defects. [Evidence](https://github.com/puma2k/freelane/blob/main/docs/evidence/p2-clipboard-auto.json).

Known minor: if native host clipboard initialization fails, automatic mode retries at its polling interval rather than disabling itself. No content is transferred on that failure path. Source-file hashing before upload also currently has no progress display.

Clipboard requires an interactive graphical guest session; Ubuntu Server without a desktop has no GUI clipboard. Real Mac/Windows guest clipboard, normal-user Windows startup and physical Mac HVF/Dock/sleep testing remain pending. Native compile/packaging checks are not runtime guest acceptance.

The package is ad-hoc signed, not Apple-notarized. Verify checksums and read [clipboard instructions](https://github.com/puma2k/freelane/blob/main/docs/clipboard.md). Upgrade the Windows ARM64 guest agent using the included verified lifecycle helper to enable protocolv6.

Tiếng Việt: Có tự đồng bộ clipboard tiếng Việt/emoji hai chiều, mặc định tắt và chỉ bật cho VM đang chọn. Xung đột sẽ dừng để chọn Gửi/Lấy, không tự quyết định nội dung cần giữ. Cần cập nhật guest agent; kiểm thử guest Mac/Windows thật vẫn còn chờ.
