# Identical-frame viewer optimization

The viewer retains the previous successfully decoded JPEG key. An exact match on window identity (including incarnation), dimensions and JPEG bytes skips repeated hex parsing, JPEG decoding and texture replacement. Changed data takes the existing bounded decode path. Selection/inventory changes and errors invalidate the retained key. Only the previous key is retained, shared immutably with the single worker.

If an unchanged reply arrives after A→B→A selection cleared the displayed texture, one local worker decodes the validated key and restores it even with live refresh off. This sends no additional guest request. Its result still must match the current window.

No guest capture/encoding, bandwidth, frame cadence, quality, input or RPC behavior is changed. This is an optimization of the existing snapshot viewer, not a video transport. Bounds remain640×480 and4MiB decode allocation, with at most64KiB encoded hex per retained key.

## Measurement

Owned Linux software-rendered debug binaries, constant640×360 synthetic-peer JPEG, existing1second cadence. Three4second process-CPU samples after3second warmup, sequential before/after runs without concurrent builds. Median CPU of one core: 14.50% before versus 8.25% after. These are short single-host observations, not a Mac/release/guest CPU, latency or battery result. The guest still captures/encodes/sends every requested frame.

[Raw samples, binary hashes, review and acceptance](evidence/viewer-frame-cache.json). To repeat on Linux, build the baseline and current debug binaries, then run the owned fixture separately for each:

```sh
FREELANE_VIEWER_BINARY=/path/to/binary FREELANE_VIEWER_BENCHMARK=sample python3 tests/viewer_schedule_acceptance.py
```

Requires Pillow, Xvfb, X11/XTest libraries and an available display number170–199; `FREELANE_XVFB` can select the Xvfb executable. The fixture creates private directories/socket/authentication and cleans up only its own processes/files. The regular invocation instead runs input, target-switch, failure and live-off/manual-refresh acceptance.

Validation:222Rust checks passed/9ignored,14MCP, strict lint and Mac core cross-check; native EN/VI owned-viewer/synthetic-peer checks. One review Important was reproduced and fixed: manual unchanged capture after selection roundtrip must restore without another RPC. No current deferred minors. Native Mac/installed Windows performance and functional acceptance remain pending.
