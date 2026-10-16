FreeLane v0.1.0-preview.23 for Apple Silicon Macs (macOS14+).

Reduces repeated host work when an app-viewer frame is unchanged.

- Exact matches to the last validated window identity/incarnation, dimensions and JPEG skip repeated hex parsing, JPEG decoding and texture creation. Changed frames still use bounded decoding. Selection/inventory changes and errors invalidate the retained key.
- If a manual unchanged reply arrives after selecting away and back, the viewer can restore its cleared texture using one local decode worker, even with live refresh off and without another guest request. Late local results still require the selected target to match.
- Keeps snapshot cadence/quality, guest capture/encoding, bandwidth, protocol versions, input order/no-replay and one-worker/queue limits unchanged. This is not a video transport or a claim of Coherence parity.

Validation:222Rust passed/9ignored,14MCP, strict lint and Mac core cross-check. Actual texture allocation regression RED→GREEN; exact/changed/invalid/dimensions/incarnation decoder tests and cache invalidation tests pass. Owned native EN/VI viewer/synthetic guest tests cover manual selection roundtrip, no extra guest request, stale results, held requests, input fairness and failure/no replay. One review Important was reproduced and fixed; no deferred minors.

In short Linux software-rendered debug static640x360 samples (three4second samples each), viewer CPU medians were14.50% before and8.25% after of one core. Raw samples/binary hashes are in docs/evidence/viewer-frame-cache.json. This is not customer release, native Mac, guest CPU, latency, battery or Parallels performance evidence. Installed WindowsARM/native Mac and remaining roadmap acceptance stay pending. Ad-hoc signed; no Developer ID/notarization.

Tiếng Việt: Bỏ xử lý lại JPEG/texture khi frame giống hệt frame đã xác minh. Frame thay đổi vẫn được kiểm tra và giải mã; đổi cửa sổ không dùng nhầm cache. Đã sửa trường hợp chụp thủ công rồi đổi đi/đổi lại bị trống hình, không cần thêm request guest. Số đo CPU chỉ thuộc bài thử Linux debug, chưa phải kết quả trên Mac.
