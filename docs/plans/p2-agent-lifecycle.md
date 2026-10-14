# P2 guest-agent lifecycle

Continue the approved P2 roadmap. Keep install/update/uninstall inside the interactive Windows user's LocalAppData and Startup folder, without elevation, services or driver changes. The existing install-only helper lacks artifact verification and update recovery.

Implement a required SHA256 for install/update, verify a private staged copy before mutation, reject non-ARM64 PE binaries, preserve existing/changed files, lease a per-installation lock, and refuse an active agent instead of terminating unrelated processes. Maintain an ownership manifest and verify the managed executable/shortcut before update/uninstall. Updates retain a verified old binary; explicit rollback restores it. Uninstall retains backups and unrelated user files. Neither guest contents nor clipboard contents belong in logs.

Package an executable checksum next to the ARM64 binary. Add native Windows CI tests covering install, duplicate install, tampered source/installed files, update, rollback, invalid backup metadata, uninstall and preserved unrelated files. Build the real ARM64 MSVC executable for these file/shortcut tests; do not label a GitHub admin runner as normal-user or native guest acceptance.

After this lifecycle milestone, continue P2 opt-in Unicode clipboard, transfer and document-open work. Those depend on the agent and feature negotiation, and do not replace native acceptance.
