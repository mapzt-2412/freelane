FreeLane v0.1.0-preview.15 for Apple Silicon Macs (macOS14+).

Adds live read-only Ubuntu9p sharing and host-folder availability/reselection controls. The roadmap remains in progress.

- Files settings and the VM wizard offer live read-only alongside live read/write9p and read-only USB. QEMU enforces readonly at its filesystem backend, including against guest root. Host changes remain readable. Both9p modes require Ubuntu; Windows retains USB media and explicit agent transfers.
- Choose shared folder and Check folder inspect host availability asynchronously. Missing folders are never recreated. `freelane share-status vm.toml` returns a read-only report; guest mount remains unknown, not inferred from configuration or process status.
- On Mac, selected and canonical volume paths are checked, refusing internal-disk fallback and an external selection replaced by an internal symlink. Changing the source requires stopped-VM save/restart; actual Mac physical detach/reconnect remains pending.
- Opt-in automatic clipboard, verified file transfers/text-document opening and automatic VM setup remain available.

Validated:145 workspace tests/9ignored,11MCP, strictlint/build/Mac crosscheck. A real disposable ARM kernel fixture requested a writable9p mount; guest root create/overwrite/rename/delete were refused, host edits remained visible and native guest shutdown completed. Existing read/write create/rename/delete acceptance was rerun. Actual GUI folder missing/no-recreation/recovery controls inspected EN/VI. Review Important selected-volume symlink bypass was reproduced RED and fixed GREEN. [Evidence](https://github.com/puma2k/freelane/blob/main/docs/evidence/p2-share-permissions.json).

Minor limitations: imported settings may retain cached availability; press Check folder again. The Ubuntu-only error wording still mentions read/write for read-only mode. Host availability does not verify permissions for every file or guest mount readiness. Guest mount setup remains explicit; FreeLane does not grant guest sudo access. Native Mac/Windows guest and physical-drive acceptance remain pending.

Read-only USB still uses a FAT16 volume with approximately504MiB capacity and an unchanged source requirement; live9p has neither that FAT limit nor snapshot semantics. Live sharing is not background file synchronization. Full Windows live mounts, Coherence video,3D/Metal and the rest of the roadmap remain unfinished.

The Mac package is ad-hoc signed, not notarized. Verify checksums and read [sharing instructions](https://github.com/puma2k/freelane/blob/main/docs/sharing.md).

Tiếng Việt: Có chia sẻ Ubuntu9p chỉ đọc được QEMU chặn ghi, kiểm tra/chọn lại thư mục và giữ nguyên khi nguồn mất. Chưa xác nhận guest mount hay tháo/lắp ổ Mac thật; nhập cấu hình xong nên bấm Kiểm tra thư mục lại.
