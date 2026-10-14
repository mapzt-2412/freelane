# P2 live-share permissions and host availability

Add read-only live Ubuntu 9p beside existing read/write9p and immutable USB media. Enforce readonly at QEMU filesystem backend, not merely a guest mount option. Both9p modes remain Ubuntu-only.

Add read-only host availability reporting for saved configuration, separate from guest mount state (unknown without guest evidence). Refresh asynchronously in Files settings, with explicit folder selection when a destination is missing. Do not recreate missing folders or silently reconnect a replacement. Changes require stopped VM/restart through existing save lock.

Reuse external /Volumes mount validation so a disconnected disk's ordinary fallback directory cannot be exposed as the selected share. Test missing/replaced directories and external fallback refusal; exercise read-only behavior on a real disposable ARM Linux guest if trusted existing kernel/modules remain available. Keep Windows live sharing and native Mac mount acceptance pending.
