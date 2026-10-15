FreeLane v0.1.0-preview.17 for Apple Silicon Macs (macOS14+).

Adds extended Windows app input, verified backup details, host-aware resource suggestions and library search. The full roadmap remains in progress.

- Updated Windows agent v9/v10 checks window incarnations, reattaches exact-image apps with no arguments before launching, and adds targeted right/double-click, wheel, a bounded drag gesture, guarded foreground shortcuts and explicit client sizing. Command maps to guest Control. Input is never automatically replayed. Drag is delivered on release; this remains a JPEG snapshot viewer, not Coherence video.
- Viewer language follows the manager and can be changed in the viewer. Idle repaint scheduling reduces unnecessary work; the recorded Linux idle-viewer benchmark is not a Mac/VM/battery or Parallels performance result.
- Backup space estimates use measured qcow2 allocation rather than the virtual maximum. Recovery is available from an empty library. Verified backup details show label, UTC creation time, component count and stored size; restore rechecks data before writing to a new destination. Guest identities are retained: this is not an independent clone or live snapshot.
- Hardware settings offer Battery/Balanced/Performance CPU/RAM suggestions, reserving host capacity and preserving guest minimums. They edit the draft and require stopped-VM save/restart. No measured energy/speed claim or live CPU quota. Unknown Linux cgroup layouts disable suggestions, leaving manual controls.
- Library search matches names and guest types, including Vietnamese case conversion, without removing bookmarks or changing the selected VM.
- MCP now exposes22 tools. The new asynchronous checkpoint_info job verifies only completed session-owned backups and returns real job status/exit/output.

Validation:177 portable Rust tests passed/9ignored,14MCP checks, strict workspace/WindowsARM lint, WindowsARM64 build and Mac core cross-check. Native Windows CI exercised owned USER32 pointer messages, resize, stale identity and background-shortcut refusal. Real sparse256GiB disk tests preserved start/end patterns, source hashes and overwrite/corruption refusal. Owned authenticated Linux EN/VI UI captures were inspected. These checks do not establish installed WindowsARM foreground shortcuts, protected-storage/TPM recovery, physical Mac/HVF/Dock/sleep/device acceptance or complete Coherence.

Known details: guest paste hotkey can be consumed by the host UI when its clipboard has no text; use Paste in guest. Small hosts can show disabled resource presets when guest minima plus host reserve do not fit. Publisher signing/notarization, independent clones, live video, Windows live folder sharing and advanced graphics remain open. Ad-hoc signed; prerequisites install through the VM setup flow.

Tiếng Việt: Bổ sung thao tác chuột/phím tắt có kiểm tra cửa sổ, chọn CPU/RAM theo máy chủ, tìm VM, backup theo dung lượng thực và kiểm tra bản sao trước khôi phục. Có22 công cụ MCP. Chưa hoàn thành toàn bộ roadmap; video Coherence, clone độc lập, GPU3D và kiểm thử Mac thật vẫn còn.
