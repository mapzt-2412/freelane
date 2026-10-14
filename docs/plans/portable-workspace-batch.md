# Portable workspace batch — roadmap remains in progress

This batch advances independently testable P2/P3/P4/P5/P7 foundations while physical Mac acceptance is deferred. It does not complete Coherence, advanced graphics, independent guest cloning or the full roadmap.

- Agent v7 reports bounded Linux `freelane` 9p mount tags, paths and mount flags. This does not authenticate the running backend's host directory; `source_identity_verified` remains false.
- Agent v8 returns pages of at most 12 local known applications. Windows reads App Paths registry entries and installed system tools; Linux uses a small known desktop application set. No registry command is executed. Windows enumeration has one bounded worker and a deadline. UWP/Start Menu/icons and fully consistent catalogs during concurrent installation remain unfinished.
- Applications UI searches/selects metadata and clears arguments on selection. App launch is a separate explicit operation. Dock bundle identifiers now derive from canonical VM path, executable and literal arguments; moving a VM changes identity. Existing bundles are never overwritten.
- Backups UI creates immutable stopped-VM full-copy checkpoints and restores into fresh folders. This preserves guest disk/NVRAM/TPM identity; it is not an independent guest clone or a live/memory snapshot. Native file dialogs and Windows protected-storage restore remain unaccepted.
- `vm-resources` samples kernel counters of the connected QMP process. Resident host memory differs from configured guest RAM; CPU time is cumulative, not utilization percentage. Both status probes must succeed. Details displays manually refreshed observations.
- MCP has 21 tools, including read-only resources, readiness, capabilities, app catalog and sharing status. Backup/restore jobs return real completion states via `dev_result`; only completed session-owned checkpoints may be restored, always to a fresh workspace. Registration is session-only. Artifacts are retained after disconnect and must be registered in the client configuration for subsequent sessions.

Validation: portable workspace suite, strict lint, Windows ARM64 cross-build, Mac ARM64 core cross-check, native Linux stdio metadata, disposable QEMU peer/memory observation, synthetic UI captures and real qemu-img/qemu-io MCP backup roundtrip. A file-state roundtrip does not establish guest boot or TPM recovery.

Next: P3 window incarnation/input/reattach, icons and measured separate video transport; P4 independent clone and Windows guest restore; P5 tuning/limits; P6 native devices/audio/graphics; P7 update integrity/rollback and native release installation. Developer ID/notarization requires certificate/account access. Mac HVF/Dock/sleep and device tests remain explicitly deferred, not passed.
