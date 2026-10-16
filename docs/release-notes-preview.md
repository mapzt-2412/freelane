FreeLane v0.1.0-preview.20 for Apple Silicon Macs (macOS14+).

Adds an English/Vietnamese guest outbox file picker, so receiving a file no longer requires typing its name. The full roadmap remains in progress.

- In Display, click Choose guest file, select a filename, then Receive file from guest and choose a host destination. Small sizes display as bytes/KiB/MiB; long names have full-name hover text. Refresh list reads another inventory when clicked.
- The matching updated agent advertises file_export_catalog protocolv12 only for an explicitly enabled outbox. Saved Windows folder selections from preview.19 continue working after update. Manual filename-based v11 receiving remains available with older agents.
- The listing reads metadata in the pinned selected outbox only, examining at most512 entries and returning up to128 eligible regular files. Large/partial inventories show a limited notice and retain manual entry. Hidden/nonportable names, directories and symlinks are omitted. No background scan or automatic file-content reading/opening.
- A VM change clears cached listing/selection, and late results are checked against the current VM. Selecting a name still uses the existing size/SHA-256 verified copy and preserves conflicting host files. Refresh/receive can observe subsequent source changes; no filesystem snapshot is claimed.
- CLI: freelane file-list vm.toml returns the bounded JSON inventory. Guest setup remains separately opted in; update the agent and enable an outbox to use the picker.

Validation:195 Rust checks passed/9ignored,14MCP checks, strict workspace/WindowsARM lint, WindowsARM64 build and Mac core cross-check. Real Linux guest-agent/hostCLI listed and received3MiB. Owned native EN/VI desktop controls selected a Vietnamese filename by click and received matching bytes without typing its name; only the host folder chooser supplied a fixed owned path. Tests cover opt-in, sorting, nonregular entries, scan/result bounds, directory replacement, invalid/duplicate/mismatched replies and late VM responses. A fresh review native probe fit128 maximum-length filenames into33503-byte responses twice and then began a v11 transfer.

Native Mac filesystems/dialogs, installed WindowsARM/reparse behavior, physical removable drives and VM channel reconnect acceptance remain pending. Windows live folder sharing, Coherence video, clones and remaining roadmap work are unfinished. Ad-hoc signed; no Developer ID/notarization or performance comparison claim.

Tiếng Việt: Thêm cửa sổ chọn file từ thư mục xuất của máy ảo bằng giao diện Anh/Việt; chọn tên rồi nhận về thư mục trên máy như trước, không cần gõ tên. Có tải lại, hiển thị dung lượng rõ hơn, giới hạn danh sách và giữ cách nhập tên cho agent cũ hoặc thư mục lớn. Cần agent mới và thư mục xuất đã bật. Chưa hoàn thành toàn bộ roadmap hay kiểm thử Mac thực.
