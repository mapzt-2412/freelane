FreeLane v0.1.0-preview.16 for Apple Silicon Macs (macOS14+).

Adds known guest app discovery, stopped-VM backup UI/MCP jobs and observed host resource counters. The full product roadmap remains in progress.

- Updated agents support read-only v7 Linux mount-tag telemetry and v8 paginated application metadata. Windows scans App Paths/system tools; Linux discovers a small known app set. No metadata request launches apps. Mount observation does not verify the host source directory.
- Applications UI offers catalog search/selection. Re-exporting the same VM/executable/arguments uses a stable Dock bundle identifier; existing bundles remain protected from overwrite. Icons, automatic reattach and full Coherence remain unfinished.
- Backups tab creates a stopped-VM full-copy checkpoint and restores to a fresh VM folder. Original files are preserved. Guest identity is retained; this is not an independent clone or live snapshot.
- Details and CLI expose resident memory/cumulative CPU time observed from the connected QMP process. Configured RAM is not represented as actual consumption.
- MCP expands to21 tools, including readiness/capabilities/catalog/share/resource reads and asynchronous checkpoint/restore jobs. Restore requires a completed session-owned backup and unused VM name. Restored files survive disconnect; registration is session-only and no VM is automatically booted.

Validation:155 portable workspace tests/9ignored,13MCP checks, strictlint, WindowsARM64 build and MacARM64 core cross-check. Native Linux metadata RPC, disposable actual QEMU identity/memory checks and real qemu-img/qemu-io MCP disk backup roundtrip passed. UI screenshots used synthetic configurations on an authenticated Linux display. File-state checks do not validate Windows protected storage/TPM recovery.

Native Mac HVF, Dock behavior, sleep/wake, devices and guest application/video acceptance remain pending. Ad-hoc signed, not notarized; QEMU and mobile backends are separate prerequisites. See docs/plans/portable-workspace-batch.md.

Tiếng Việt: Có tìm app đã biết, UI/MCP sao lưu và khôi phục sang thư mục mới, số liệu RAM/CPU thực dùng và định danh Dock ổn định. Chưa hoàn tất Coherence, clone độc lập, GPU3D hay kiểm thử Mac thật; không coi bản này là hoàn thành toàn bộ roadmap.
