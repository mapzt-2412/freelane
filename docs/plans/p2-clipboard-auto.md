# P2 conflict-aware text clipboard synchronization

Continue P2 in the authorized main checkout. Preserve explicit v4 Send/Copy.

1. Add a bounded policy with host/guest snapshots and stable guest-instance identity. Initial unequal content and simultaneous edits require explicit choice. Equal content does not write; successful mutations alone advance the baseline. Disconnect/restart clears baseline and requires opt-in again.
2. Add v6 clipboard read and conditional set. Guest rereads expected snapshot before writing and refuses observed concurrent edits. Host also rereads before applying an automatic pull. Clipboard OS ownership APIs do not expose an atomic cross-process compare-and-set; document this practical limitation, never claim a universal race-free clipboard transaction.
3. Add opt-in automatic mode, serialized asynchronous polling (750ms), cancellation while waiting for channel lease, explicit conflict resolution with existing Send/Copy, no automatic mutation retries after uncertain outcomes. No content in notices/logs. Preserve native clipboard session ownership when paused.
4. Run meaningful policy/conditional-write/socket tests, actual independent native X11 clipboard acceptance, full workspace/MCP/lint and platform checks. Fresh whole-batch review then commit/push; package accepted preview with verified public checksum.

Expected: policy/conditional-write regressions fail before implementation; Vietnamese/emoji transfers and echo suppression pass, conflicts preserve both contents, revocation prevents queued mutation. Native Mac and normal-user Windows clipboard acceptance remain deferred.
