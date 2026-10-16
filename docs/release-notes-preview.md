FreeLane v0.1.0-preview.22 for Apple Silicon Macs (macOS14+).

Prevents a due app-viewer snapshot from starving behind continuous input. Full roadmap development continues.

- With live refresh enabled, a due capture gets a turn after at most8 completed inputs even when the queue stays nonempty. Queue remains bounded32 and only one RPC runs at a time; input stays FIFO. Successful frames retain the1second periodic delay and640×480 JPEG bound. No protocol change or new codec.
- Live-off still accepts input and manual Refresh frame. Selecting another window drops unsent old-window input/drag/texture; late frames must match the selected target. Connection failures stop live refresh and discard pending input without replay. Existing read-only timeout retry behavior is unchanged.
- This bounds scheduling fairness by operation count. Slow RPCs still wait for completion/timeout; it is not a video, FPS, latency or Parallels performance claim.

Validation:214 Rust checks passed/9ignored,14MCP, strict workspace lint and Mac core cross-check. Original native viewer reproduced starvation after12additional inputs; updated owned EN/VI viewer captured after8total completed inputs. Authenticated X11/synthetic guest tests cover held replies/no second request, real combo target switching with old queued input and pending frame, failure/no replay and live-off input/manual refresh. A fresh review independently passed8policy+4handler tests. Two acceptance gaps were corrected; one minor remains: inventory cleanup unit test initializes no texture, so that assertion alone does not guard texture clearing.

Installed WindowsARM, native Mac/HVF/Dock/sleep, real latency/CPU measurements and continuous video remain unaccepted. Ad-hoc signed; no Developer ID/notarization.

Tiếng Việt: Viewer không còn đợi hết toàn bộ input mới lấy hình; frame đến hạn được một lượt sau tối đa8input hoàn tất. Giữ queue32, một request đang chạy và không gửi lại input khi lỗi. Đã kiểm tra UI Anh/Việt với guest giả lập giao thức riêng; chưa phải video Coherence hoặc nghiệm thu Mac thật.
