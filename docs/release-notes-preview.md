FreeLane v0.1.0-preview.21 for Apple Silicon Macs (macOS14+).

Keeps the guest agent usable when a configured file-transfer folder cannot open at startup, with explicit English/Vietnamese folder checks and recovery. The full roadmap remains in progress.

- A missing inbox or outbox disables only that direction. Launcher/clipboard and an available transfer direction remain usable. Malformed saved settings still refuse startup; folders are never created automatically.
- In Display, click Check guest folders. Restore a missing configured directory, then click Reconnect guest folders. The matching updated agent advertises a separate file_folders protocolv13. Older agents keep their existing copy features and request an update for folder recovery.
- Reconnect accepts no paths and only retries roots that failed to open. It keeps already opened directory handles pinned, refuses active uploads/exports without losing their session, and never replays file transfers. Checks are read-only; no background retries or drive monitoring.
- CLI: freelane file-folders vm.toml inspects independent inbox/outbox state; freelane file-reconnect vm.toml explicitly retries opening unavailable roots. Changing stored folder selections still requires restarting the agent.

Validation:202 Rust checks passed/9ignored,14MCP checks, strict workspace/WindowsARM lint, WindowsARM64 build and Mac core cross-check. Actual Linux hostCLI/guest-agent checks preserved the healthy direction, refused busy reconnect without losing bytes, recovered a startup-missing outbox and received3MiB with matching SHA-256. Owned native EN/VI UI checked unavailable state, restored a fixture directory, reconnected and selected/received a Unicode filename; only the host folder chooser supplied a fixed owned path. A fresh whole-batch review found no Critical/Important or Minor defects.

Ready means a directory handle was opened, not proof of physical-drive identity, current availability or future I/O success. Live unmount/hotplug and already-open handle invalidation may need agent restart and remain separate work. Native Mac/installed WindowsARM/physical external-drive acceptance remains pending. Windows live sharing, Coherence video, clones and other roadmap work remain unfinished. Ad-hoc signed; no Developer ID/notarization or performance comparison claim.

Tiếng Việt: Thư mục nhận/xuất bị thiếu lúc khởi động chỉ tắt chiều chia sẻ đó, không làm agent thoát. Có nút kiểm tra và chủ động kết nối lại sau khi khôi phục thư mục; không tự tạo thư mục, đổi đường dẫn hay gửi lại file. Khi đang chuyển file thì từ chối kết nối lại và giữ phiên hiện tại. Cần host/agent mới cùng hỗ trợ v13. Chưa giải quyết rút/cắm ổ đang dùng hoặc hoàn tất kiểm thử Mac thực và toàn bộ roadmap.
