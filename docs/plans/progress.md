# Roadmap execution status

Updated 2026-10-13. The roadmap is not complete.

| Area | Status | Evidence / remaining work |
|---|---|---|
| P0 required CI | Passed for current readiness/wizard revisions | [P0](https://github.com/puma2k/freelane/actions/runs/38341927694), [P1](https://github.com/puma2k/freelane/actions/runs/38343054624), [bounded preflight](https://github.com/puma2k/freelane/actions/runs/38343264763) |
| P0 configuration diagnostics | Implemented and tested | `doctor-json`; missing/oversized config, executable discovery, host acceleration constraints, real Windows/Ubuntu preflight; no VM execution |
| P0 real Mac/HVF/Dock/sleep baseline | Deferred by user | Mac available later; no runtime pass claimed |
| P1 fresh workspace creation | First path implemented and tested | UI form + `vm-create`; real qemu-img 64 GiB creation, repeat refusal, source NVRAM preservation; guest installation/boot not run |
| P1 persistent VM library | Implemented and tested | Private/atomic bookmark updates, corrupt file preservation, actual UI restart without config arguments |
| P1 full wizard/recovery | In progress; not accepted | File pickers and ARM64 UEFI-loader inspection implemented; physical dialog acceptance, full guest checks, installation readiness/progress, repair/recovery and interruption recovery remain |
| P1 checkpoint/recovery-copy foundation | Implemented; limited acceptance | CLI full copy + fresh-directory restore; file-level tests Linux/Mac passed; real Windows/BitLocker recovery blocked by cloud capacity and pending native testing |
| P2 clipboard/sharing/agent lifecycle | Not started | Depends on selected sharing backend and normal-user agent validation |
| P3 app video/Coherence direction | Not started beyond existing snapshot prototype | Requires transport/driver feasibility and native Mac evidence |
| P4 complete snapshot/clone/backup UX | Not started | Consistent disk/NVRAM/TPM transactions precede restore/clone |
| P5 optimization | Not started | Measure a native baseline before performance claims |
| P6 audio/USB/graphics | Not started | Requires physical hardware and backend/driver go/no-go decisions |
| P7 expanded MCP/stable distribution | Not started beyond existing MCP/previews | Follows accepted primitives; notarization requires secure signing access |

## Changes delivered

- Installed CI process utilities needed by the ARM signal test.
- Replaced repeated socket receive-timeout changes with a poll-based absolute read deadline; EOF and timeout tests preserve bounded responses and no input replay.
- Made accepted test sockets explicitly blocking for macOS/BSD listener inheritance.
- Added structured, read-only VM preflight with input size limits and actionable failures.
- Added first fresh-VM workspace creation and persistent library paths.

Next work: P1 installation readiness and interruption recovery; native dialog/guest acceptance deferred. The checkpoint foundation is now implemented; full guest restore acceptance requires sufficient storage. Native acceptance remains pending; these statuses do not authorize calling P0/P1 fully accepted or jumping to claims of complete Coherence.

[Checkpoint scope and tests](../checkpoints.md). Latest completed checkpoint CI: https://github.com/puma2k/freelane/actions/runs/38348621836 (Linux/Mac file roundtrip and required checks passed).

Creation now rejects insufficient destination capacity before backend execution or directory creation. The conservative check includes full virtual disk growth; sparse allocation is not a reservation.

P1 media check passed on the actual supplied Windows ARM64 ISO (UDF bridge, extracted ARM64 PE bootloader). Synthetic ISO tests cover wrong architecture, missing loader and invalid extents. This is not a fresh guest installation. P0 owner checklist: [Mac acceptance](../mac-acceptance.md).
