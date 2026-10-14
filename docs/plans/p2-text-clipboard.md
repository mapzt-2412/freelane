# P2 opt-in plain-text clipboard

Extend the approved roadmap with protocol v4 text clipboard operations, capability `text_clipboard`, max 8 KiB UTF-8 text, 32-hex source IDs and monotonic source sequences. Preserve v1/v2 mutations and v3 read-only discovery. Unsupported/legacy agents are refused before clipboard mutation. Bound and correlate replies; never include guest clipboard failure text or contents in routine errors.

Use the interactive user's OS clipboard via the existing arboard dependency, retain ownership on X11, and advertise the feature only when a native clipboard session can be opened. A guest without a desktop (Ubuntu Server over terminal) has no OS clipboard capability; terminal copy/paste remains a host terminal feature.

The first UI has explicit Send/Copy buttons after opt-in, not continuous synchronization. Reset consent on VM changes, check consent again after asynchronous responses, retain native clipboard ownership, and do not retry mutations automatically. Explicit read responses can be copied again; replayed incoming guest mutation requests are rejected. Images/rich text and automatic conflict-aware synchronization remain later work.

Checks: Unicode framing, limits including JSON escaping, origin preservation and replay refusal; host socket response identity/source validation; actual authenticated X11 agent/native clipboard roundtrip; disconnect/refusal behavior, Windows ARM64 build and whole-workspace/CI checks.
