# P2 agent capability discovery

Continue the approved roadmap with read-only protocol v3 discovery before clipboard/transfer changes. Existing v1 ping/launch and v2 window operations stay wire-compatible. v3 accepts Ping only and declares package version, OS/architecture, supported protocol versions, bounded feature names with operation protocols, and the existing 64 KiB frame limit. No launch/input is used to discover support.

Host negotiation probes v3, v2, then v1 at most once each using correlated read-only requests and existing channel leases/deadlines. Legacy unreported features remain unknown; the v2 snapshot boolean retains explicit true/false meaning. Each future action still uses its declared operation protocol. Discovery does not prove OS-installation completion, permissions, a usable desktop or successful app execution.

Expose `agent-capabilities CFG` and an asynchronous Details probe. Clear stale metadata on VM/status/settings changes and failed probes. Do not alter backend status from capability failures or log clipboard/file contents. The preview.6 package predates this P2 change; this work is initially main-only.

Verification: protocol/guest tests reject v3 mutations before dispatch; host tests cover modern metadata, legacy fallback, malformed bounds and explicit-denial versus unknown. Run the workspace/MCP suites and Windows/Linux ARM builds. Install the static Linux ARM agent as guest user ubuntu through a controlled NoCloud seed and check the real virtio channel; separately verify the UI. Normal Windows-account/Mac runtime acceptance remains owner-deferred.
