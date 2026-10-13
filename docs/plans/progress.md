# Roadmap execution status

Updated 2026-10-13. The roadmap is not complete.

| Area | Status | Evidence / remaining work |
|---|---|---|
| P0 required CI | Passed for current readiness/wizard revisions | [P0](https://github.com/puma2k/freelane/actions/runs/38341927694), [P1](https://github.com/puma2k/freelane/actions/runs/38343054624), [bounded preflight](https://github.com/puma2k/freelane/actions/runs/38343264763) |
| P0 configuration diagnostics | Implemented and tested | `doctor-json`; missing/oversized config, executable discovery, host acceleration constraints, real Windows/Ubuntu preflight; no VM execution |
| P0 real Mac/HVF/Dock/sleep baseline | Deferred by user | Mac available later; no runtime pass claimed |
| P1 fresh workspace creation | First path implemented and tested | UI form + `vm-create`; real qemu-img 64 GiB creation, repeat refusal, source NVRAM preservation; guest installation/boot not run |
| P1 persistent VM library | Implemented and tested | Private/atomic bookmark updates, corrupt file preservation, actual UI restart without config arguments |
| P1 full wizard/recovery | In progress; not accepted | Native pickers, automatic media architecture/guest checks, disk-space checks, installation readiness/progress, repair/recovery, offline checkpoint transactions remain |
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

Next cloud work: complete P1 media/file selection and storage preflight, then design and implement stopped-VM checkpoint transactions. Native acceptance remains pending; these statuses do not authorize calling P0/P1 fully accepted or jumping to claims of complete Coherence.
