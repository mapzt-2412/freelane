FreeLane v0.1.0-preview.19 for Apple Silicon Macs (macOS14+).

Adds remembered per-user Windows guest inbox/outbox selections and repairs file-operation cancellation wording. The full roadmap remains in progress.

- Select existing folders when installing the matching Windows ARM64 agent with `-InboxDirectory` and/or `-OutboxDirectory`. A fresh install without these options leaves both transfer directions disabled.
- Use `install-windows-agent.ps1 -Action Configure` to change selections without replacing the executable, Startup shortcut or verified rollback binary. Pass an explicit empty value to disable a direction; omit a parameter to preserve it. Stop the installed agent first, then restart it in the user session or sign in again.
- The Windows agent reads bounded installer settings beside its executable. Update/Rollback preserve current selections and do not resurrect revoked outbox access. Missing roots are never recreated; reconnect the drive or reconfigure the selection. Linux retains explicit session flags. Older agents may ignore saved settings, so use the matching updated package.
- Shared guest RPC cancellation now reports the operation, in English/Vietnamese, rather than displaying an unrelated clipboard message.

Validation:188 Rust checks passed/9ignored,14MCP checks, strict workspace/WindowsARM lint, WindowsARM64 executable build and Mac core cross-check. Actual native Linux agent/hostCLI3MiB transfers still pass both directions. Native Windows lifecycle exercises install/update/Configure/Rollback/uninstall, atomic failure preservation, Unicode paths and rejection of control characters. A native x64 agent process on Windows reads installer-produced settings and advertises only the selected file capabilities; this does not claim ARM64 execution on the x64 CI runner.

Normal-user WindowsARM Startup/login, physical removable drives and native Mac/HVF/Dock/sleep/device acceptance remain pending. This is persisted explicit file-copy configuration, not hot reload, automatic agent restart or Windows live folder synchronization. Ad-hoc signed; no Developer ID/notarization or full Coherence video claim.

Tiếng Việt: Agent Windows mới nhớ thư mục nhận/xuất đã chọn sau khi đăng nhập lại; có lệnh Configure để đổi hoặc tắt riêng từng chiều mà không thay file chương trình hay mất bản rollback. Update/rollback giữ lựa chọn hiện tại; không khôi phục quyền xuất đã tắt, không tự tạo thư mục bị mất kết nối. Cài mới không chọn thư mục thì chia sẻ file vẫn tắt. Đã sửa thông báo hủy bị nhầm sang clipboard. Toàn bộ roadmap và kiểm thử Mac thực vẫn chưa hoàn tất.
