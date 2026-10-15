# Roadmap execution status

Updated 2026-10-14. The roadmap is not complete.

| Area | Status | Evidence / remaining work |
|---|---|---|
| P0 required CI | Passed for current readiness/wizard revisions | [P0](https://github.com/puma2k/freelane/actions/runs/38341927694), [P1](https://github.com/puma2k/freelane/actions/runs/38343054624), [bounded preflight](https://github.com/puma2k/freelane/actions/runs/38343264763) |
| P0 configuration diagnostics | Implemented and tested | `doctor-json`; missing/oversized config, executable discovery, host acceleration constraints, real Windows/Ubuntu preflight; no VM execution |
| P0 real Mac/HVF/Dock/sleep baseline | Deferred by user | Mac available later; no runtime pass claimed |
| P1 fresh workspace creation | First path implemented and tested | UI form + `vm-create`; real qemu-img 64 GiB creation, repeat refusal, source NVRAM preservation; guest installation/boot not run |
| P1 persistent VM library | Implemented and tested | Private/atomic bookmark updates, corrupt file preservation, actual UI restart without config arguments |
| P1 full wizard/recovery | In progress; not accepted | File pickers and ARM64 UEFI-loader inspection implemented; physical dialog acceptance, full guest checks, installation readiness/progress, repair/recovery and interruption recovery remain |
| P1 checkpoint/recovery-copy foundation | Implemented; limited acceptance | CLI full copy + fresh-directory restore; file-level tests Linux/Mac passed; real Windows/BitLocker recovery blocked by cloud capacity and pending native testing |
| P2 clipboard/sharing/agent lifecycle | In progress | v3 discovery, verified Windows agent lifecycle and opt-in manual Unicode clipboard implemented; native guest acceptance, automatic sync, transfer/document opening remain pending |
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

Creation now rejects insufficient destination capacity before backend execution or directory creation. The earlier full-virtual-growth check is superseded by preview.9 initial-allocation checks; sparse allocation is not a reservation. Checkpoint/copy budgets remain separate.

P1 media check passed on the actual supplied Windows ARM64 ISO (UDF bridge, extracted ARM64 PE bootloader). Synthetic ISO tests cover wrong architecture, missing loader and invalid extents. This is not a fresh guest installation. P0 owner checklist: [Mac acceptance](../mac-acceptance.md).

P1 runtime observation: `vm-readiness` and desktop Refresh status distinguish QMP state from agent connectivity; installation completion stays unverified. QMP socket and absent-backend tests preserve configuration and avoid creating runtime state. Guided installation/recovery remain outstanding.

P1 interruption foundation: protected staging, no-replace publication and `creation-recover` for prepared never-started workspaces implemented. Tests cover backend failure, a simultaneous destination collision, recovery, corrupted files and early-stage refusal. These use nonbootable file fixtures; native wizard creation and OS-installation recovery remain unverified.

P1 wizard resources/connectivity/progress: implemented and tested. Actual Linux UI created a 16 GiB Ubuntu workspace using signature/checksum-verified Ubuntu 24.04.5 media, saved its library entry and reopened the configuration. No guest start or installation was performed. Full workspace: 83 passed, 6 ignored. Native acceptance and guided OS installation remain pending.

P1 review fixes: request parsing now uses a nonblocking regular-file descriptor and capped read; UDF fallback explicitly rejects generic archives; read-only recovery sources are rejected before publication and post-publication marker failures have an explicit committed/warning result. These were reproduced with FIFO, ZIP-with-ARM64-loader and read-only-source regressions before fixing.

P1 settings templates: versioned mutable-settings export/import implemented; private no-overwrite export, existing-VM backups, busy-lock refusal and storage identity preservation tested. UI import previews before Save changes. Native file dialogs and full backup/relocation acceptance remain pending.

Settings UI acceptance on Linux: preview preserved the saved file, export used saved values while the draft differed, Save applied the requested CPU/RAM and preserved storage hashes. Dialog selection used a test-only zenity stub; native dialogs remain pending. Full workspace: 92 passed, 6 ignored; clippy and Apple Silicon core cross-check passed. [Evidence](../evidence/settings-bundles.json).

P1 library path recovery: explicit missing-bookmark relink in CLI and localized UI, with guest-file preservation and rejection tests. Linux UI exercised the repair with a test-only dialog stub. Full suite 94 passed, 6 ignored; clippy passed. Runtime migration and native guest relocation remain unverified. [Guide](../library-recovery.md).

P1 Ubuntu cloud image mode: independent qcow2 import added to CLI/wizard, growth/capacity checks and private source snapshots; ISO requests stay compatible. Linux UI created a VM from a signature/checksum-verified official ARM64 image; first boot reported Ubuntu 24.04/aarch64 and reboot preserved a guest marker. Native Mac and agent/desktop installation are unverified. Automated boot/reboot re-test is pending after correcting the test seed's early shutdown timing. 99 workspace tests and 11 MCP tests passed, clippy and Mac ARM64 core cross-check passed. [Guide](../cloud-images.md).

Cloud guest automated acceptance now passed on a fresh Linux TCG VM: aarch64/Ubuntu 24.04 observed, exact marker value survived reboot, two clean shutdowns and unchanged input image. Fixing Mac CI canonical-path assertions and duplicate alias handling; Linux/ARM jobs at fa22ec7 passed. Full workspace 100 passed, 6 ignored.

Ubuntu guest checkpoint acceptance: offline checkpoint created after clean shutdown, restored into a fresh workspace, original NoCloud test seed reattached through settings import, exact persisted guest marker observed after boot, clean shutdown and unchanged source VM files verified. This is Linux TCG Ubuntu evidence; Windows TPM/native Mac acceptance remains pending. [Evidence](../evidence/ubuntu-cloud-checkpoint-guest.json).

Required CI for code commit 1a5405c passed, including Linux/Mac portable suites, real ARM checks, guest-agent builds and Mac checkpoint file roundtrip: https://github.com/puma2k/freelane/actions/runs/38384374715 . Native Mac guest/HVF/Dock acceptance is still deferred.

P1 reviewable preview published: v0.1.0-preview.6, source a50687e. Mac app/DMG and Windows ARM64 agent workflow passed; the public DMG was downloaded and its SHA256/UDIF footer independently checked. [Release](https://github.com/puma2k/freelane/releases/tag/v0.1.0-preview.6), [package evidence](../evidence/release-preview6.json). This is an ad-hoc preview and does not accept P0/P1 native runtime or complete P2–P7.

P2 capability discovery: read-only v3 metadata and bounded v3→v2→v1 fallback implemented in CLI and asynchronous desktop Details probe, retaining legacy unknown semantics. 106 workspace tests passed, 6 ignored; 11 MCP tests, clippy, desktop build and Apple Silicon core cross-check passed. Linux/Windows ARM64 agents built. Real v3 virtio discovery and separate UID1000 execution checks passed on controlled Ubuntu ARM64 TCG; v1 Ping and v2 capabilities also passed. Clipboard, transfer and lifecycle are pending. Preview.6 predates this change.

P2 real capability evidence: controlled Ubuntu guest returned bounded v3 metadata through virtio; v1/v2 read-only replies and UID1000 launch checked separately. Actual English/Vietnamese Details probes displayed connected state. A missing Vietnamese availability label was corrected and rebuilt after screenshot inspection. Guest shutdown completed cleanly. [Evidence](../evidence/agent-capabilities.json). This accepts discovery only, not the complete P2 milestone.

Mac wizard onboarding: discovers a coherent Homebrew QEMU/firmware installation, fills empty technical fields without replacing manual choices, hides paths in Advanced components, and offers fixed QEMU/swtpm installation through a background worker. Installer timeout terminates its owned process group. 109 tests passed/6ignored, clippy/build/Mac core cross-check passed; real Linux English/VI wizard screenshots use a local Homebrew-shaped fixture. Actual Homebrew installation/native Mac boot remain unverified; Homebrew bootstrap and a fully bundled backend remain pending.

Onboarding follow-up includes Homebrew 7-Zip and Finder-safe media-tool discovery, required for Windows UDF ISO inspection. Full 109-test suite/clippy/build/Mac core recheck passed. Preview.8 supersedes preview.7 with this prerequisite fix.

Preview.8 published from 0851c83: native Mac package/DMG verification and Windows agent packaging succeeded, public DMG SHA256 and UDIF footer independently verified. [Release](https://github.com/puma2k/freelane/releases/tag/v0.1.0-preview.8), [evidence](../evidence/release-preview8.json). Actual Homebrew installation and physical Mac guest acceptance remain pending.

Sparse-storage/setup milestone: empty qcow2 initial capacity no longer requires the full virtual maximum free; 4TiB application cap removed with checked arithmetic. Wizard reports selected filesystem capacity asynchronously, resolves collision names in the picker worker and refreshes setup on reopen. Pinned parent/stage descriptors and guarded no-replace publication refuse changed/disconnected destinations. 116 tests/6ignored, 11MCP, clippy/build/Mac core check passed. Actual Linux UI created Windows128GiB with16.7GiB free and200704-byte disk; real QEMU8TiB sparse creation passed. No Windows install/native Mac/external-device acceptance claimed. [Guide](../setup.md), [evidence](../evidence/sparse-setup.json).

Preview.9 published from508a89a: native Mac app/DMG and Windows agent packaging/publish passed. Public DMG SHA256 and UDIF footer independently verified. [Release](https://github.com/puma2k/freelane/releases/tag/v0.1.0-preview.9), [package evidence](../evidence/release-preview9.json). Native guest/physical SSD detach remains unaccepted.

## 2026-10-14 — Single setup flow

User steering: hide normal disk/resource tuning; Windows chooses ISO, Ubuntu Server selects an LTS release and downloads automatically; host prerequisites belong in the same action. Implemented simplified EN/VI setup, bundled pinned Homebrew bootstrap and Ubuntu trust keyring, automatic QEMU/swtpm/7-Zip/GnuPG setup, signed-manifest/checksum downloads on selected storage and installer startup. Original roadmap and user-deferred physical Mac P0 acceptance remain open.

Preview.10 published from 5b63931: native Mac app/DMG and Windows agent packaging succeeded; required Linux/Mac/ARM CI passed; independently downloaded public DMG SHA256 and UDIF footer verified. [Package evidence](../evidence/release-preview10.json). Physical Mac setup/HVF remains pending. Reusable cloud install/start configuration saved with QEMU GTK and GnuPG prerequisites; environment publication remains a user settings action.

P2 lifecycle/clipboard continuation: per-user SHA256/ARM64-checked Windows install/update/rollback/uninstall with managed-file checks, active-agent refusal, transaction backups and blocked-uninstall preservation. Native lifecycle CI38506345626 passed with real ARM64 MSVC artifact, including locked-manifest automatic recovery and locked-executable uninstall. Clipboard v4 preserves v1/v2/v3, caps UTF-8 text at8KiB, validates origins/sequences and sanitizes guest clipboard errors. EN/VI explicit Send/Copy controls default off; consent resets per VM and revoked queued sends are canceled before transmission. Actual native X11 agent tests and desktop transfer between two separate authenticated displays passed; these use a private stdio/socket fixture, not a VM boot. Windows/Mac native clipboard/startup acceptance, automatic sync and P2 transfers remain pending.

Complete P2 lifecycle/clipboard branch6d43db1 required Linux/Mac/ARM checks and native Windows lifecycle CI passed, including the dedicated Linux native clipboard check: [required checks](https://github.com/puma2k/freelane/actions/runs/38507937641), [Windows lifecycle](https://github.com/puma2k/freelane/actions/runs/38507937678). Next portable P2 work remains staged file transfer/document opening and conflict-aware automatic sync; native guest acceptance remains pending.

Preview.11 packaging did not publish: the new license pre-step duplicated the Mac package's existing attribution directory and raised FileExistsError. Native app compilation/signature checks and required CI passed, but no DMG publication is claimed. Reproduced with the actual embedded attribution Python block, removed only the duplicate Mac pre-step (existing attribution remains; Windows retains the new helper), added portable packaging regression coverage and publishing an immutable preview.12 tag.

Preview.12 published from0cbebf7 after attribution staging fix: native Mac package/DMG and Windows agent publication passed, required checks passed, public DMG SHA256 and UDIF footer independently verified. [Package evidence](../evidence/release-preview12.json). P2 file transfer/document opening, automatic conflict-aware clipboard sync and native guest acceptance remain pending; no overall roadmap completion claimed.

### 2026-10-14 — P2 explicit file transfer and text-document opening

Implemented opt-in agent protocol v5, one bounded streaming transfer into a selected guest inbox, SHA-256, cancellation and atomic no-overwrite publication. Desktop provides file selection/progress/cancel in EN/VI; CLI `file-send` and `file-send-open`. Text documents open only through fixed system Notepad or installed Linux text editors after completed receipt/hash checks. Transfers are explicit copies, not background synchronization.

136 workspace tests passed / 8 ignored, strict clippy passed; actual native Linux agent + host CLI moved 3 MiB with matching SHA-256/Unicode name and preserved conflicting originals/canceled sessions. Review fixes exercised RED→GREEN: exact-handle staging publication and renamed-document refusal. Windows native transfer storage and lifecycle CI38522841949 passed after correcting the native file-link API. Mac guest-only test gating and strict lint were corrected; final required CI38523358565 and native packaging38523358555 passed. Preview.13 published atdfbd1d6; downloaded DMG SHA-256 and UDIF verified. Real Mac/Windows interactive acceptance, automatic clipboard sync and remaining roadmap are unfinished. Evidence: `docs/evidence/p2-file-transfer.json`; usage: `docs/file-transfer.md` / `.vi.md`.

### 2026-10-14 — P2 automatic clipboard authorized for publication

Implemented opt-in v6 conditional updates, observed-change/echo policy and serialized750ms desktop polling. Conflicts pause for explicit Send/Copy; VM changes/reset/disconnect revoke automatic mode.142 workspace tests/9ignored,11MCP, strictlint/build/Mac crosschecks/WindowsARM64build passed. Separately executed native two-display acceptance and actual desktop conflict/resolution/pull used synthetic strings only. Fresh review noCritical/Important; minor deferred for host clipboard initialization retry. Native API checks are not an atomic cross-process CAS. See docs/evidence/p2-clipboard-auto.json and bilingual clipboard docs/screenshots.

Automatic approval review rejected commit/push/publication because automatic clipboard transfer could involve sensitive content and required explicit authorization beyond the general feature request. Local code is complete/reviewable; not committed or published. No rejected publication was retried through another route. preview.13 remains the current published build; native Mac/Windows acceptance and roadmap remain unfinished.

User explicitly approved commit, push and publication of default-disabled, per-VM opt-in automatic clipboard synchronization. Approval blocker resolved; native CI and release verification pending.

P2 automatic clipboard released as preview.14 at e97f92e; requiredCI38530620922, Windows38530620971 and nativepackaging38530620925 Success. Public DMG SHA-2566323e7549f4f773b1d26b7d2e538854575a6d525d4d76b1a05434c32f3117433,13014354bytes/UDIFverified. Explicit publication approval resolved the earlier rejection. Native Mac/Windows guest acceptance and remaining roadmap remain unfinished.

### 2026-10-14 — P2 live read-only9p and host availability

Added Ubuntu-only live read-only9p enforced by QEMU, native folder picker and asynchronous Files status, read-only `share-status` CLI. Status explicitly leaves guest mount unknown; missing sources are not recreated, and changes still require stopped-VM save/restart. Validates selected Mac volume path before and after canonicalization.

145 workspace tests passed/9ignored,11MCP, clippy/build/Mac crosscheck passed. Real disposable ARM kernel guest refused root create/overwrite/rename/delete and observed live host changes; existing read/write acceptance rerun passed. Actual GUI missing/recovery flow captured EN/VI. Fresh review Important selected-volume symlink bypass reproduced RED and fixed GREEN. Minor deferred: report cache after import (check again), Ubuntu gate error wording. Native Mac external-drive and guest mount telemetry remain unaccepted. RequiredCI38538419989, Windows38538420007 and nativepackaging38538420067 passed; preview.15 published from038fb59. Public DMG SHA-256/UDIF verified; see docs/evidence/release-preview15.json.

### 2026-10-14 — Portable workspace batch in validation

Implemented v7 mount-tag observation (host source remains unverified), v8 bounded known-app catalog, backup/restore desktop UI, kernel process telemetry through connected QMP peer identity, stable Dock export identifiers and 21 MCP tools with asynchronous owned-checkpoint restore. Original VM files survive restore; no auto-boot or independent guest clone claim. Native Linux stdio metadata and actual QEMU memory/peer checks passed. Actual MCP qemu-img/qemu-io roundtrip preserved the source and retained artifacts after disconnect. Synthetic EN desktop Backups/Details captures inspected. Full roadmap, native Mac and Windows protected-storage/video acceptance remain unfinished. See portable-workspace-batch.md.

RequiredCI38554340167 and Windows38554340163 passed for3f2fa70. Preview.16 packaging requested; public artifacts not yet verified.

Viewer language follow-up: standalone `--app` accepts `--lang`, receives the manager language on open and exposes an EN/VI selector. Repeated per-frame notices removed. Strict lint/desktop build passed and actual owned EN/VI viewer captures inspected with a synthetic nonbootable profile; this does not validate native window input or capture. This follow-up is newer than preview.16.

Preview.16 publicly downloaded and verified:FreeLane-0.1.0-3f2fa70869f4-macos-arm64.dmg,13347781bytes,SHA-25689c677347e087911e78a7177f34686117d8d2ce95bab619453889913693985c9,validUDIFfooter. See docs/evidence/release-preview16.json. Full roadmap remains unfinished; viewer-language follow-up is on main after this release.

P3 identity/reattach foundation in validation: v9 window properties plus agent map reject observed HWND incarnation changes; v1/v2 compatibility retained. Empty-argument profiles attach to matching process image without launch, otherwise launch once; profiles with arguments preserve one literal launch and launched-PID tracking.158 portable tests/9ignored, strictlint, WindowsARM64 build and Mac core crosscheck passed. Added owned USER32 native identity/lookup test; Windows CI execution pending. No native Mac/Dock, full input, video or different-image delegated-app acceptance claimed.

P3 native Windows USER32 test passed in job115828258567 (run38556921189); Windows agent lifecycle38556921176 passed. Overall CI failed at Mac app_reattach fixture. Inherited nonblocking accepted socket behavior reproduced on Linux with identical WouldBlock→missing-CLI failure. Fixture now explicitly restores blocking mode and5s deadlines; focused test and strictlint passed. Product checks were not weakened; native CI retry pending.

### 2026-10-15 — Thin backups and adaptive idle viewer

P3 required CI38558068955 Success after explicitly restoring blocking mode on accepted fixture sockets; original failure reproduced before correction. Native Windows owned-window property/refusal/image lookup checks passed.

Thin backup refusal reproduced with a sparse256GiB virtual disk and tiny real contents. Creation now validates qemu-img allocation measurements and estimates converted allocation plus firmware/NVRAM and80MiB TPM/config/metadata allowance. Actual start/end logical-pattern backup/restore, source hashes, overwrite/corruption refusal passed. Still no Windows protected-storage restore or independent guest clone acceptance.

Adaptive viewer scheduling preserves pending-worker polling and active capture deadlines while idle/no-target repaints use5s. Owned authenticated Linux software-rendered empty-viewer CPU median was4.00%→0.25% of one core across three4s samples per build; EN-to-VI native menu interaction after idle verified. This is not VM/video/Mac/battery or Parallels evidence.159Rust/9ignored,13MCP and strictlint passed. Single fresh whole-batch review in progress before commit/publication.

Review follow-up (2026-10-15): Restore is available from an empty library; checkpoint creation still requires a selected VM. Two headless UI regressions reproduced the missing entry and now pass. Saved app launch preserves EN/VI. Full validation: 161 Rust tests passed, 9 ignored, strict workspace clippy passed. Native Mac guest acceptance and the full roadmap remain unfinished.

### 2026-10-15 — P3 extended input

Protocol v10 and matching Windows agent/viewer add targeted primary/secondary click ordinals, wheel, bounded16step drag, foreground-guarded modifier shortcuts and explicit client sizing. Mac Command maps to Control; Shift/Option text avoids duplicate chords. Legacy agents retain basic input. Single batch review Important typing duplication and horizontal direction findings reproduced RED→GREEN.170Rust passed/9ignored,13MCP, strict workspace/WindowsARM lint, WindowsARM64 build and Mac core check passed. Owned EN/VI RPC UI fixture exercised controls; native Windows execution and physical Mac acceptance pending. Native paste hotkey can be consumed when host clipboard has no text; toolbar Paste in guest works. Full P3/video and roadmap remain unfinished. See p3-window-input.md and ../evidence/p3-window-input.json.

P3 input requiredCI38672599763 and Windows lifecycle38672599738 Success. Native owned USER32 input/resize/stale identity/background-shortcut refusal passed job116218940350; no live WindowsARM/Mac foreground shortcut acceptance claimed.

P5 allocation presets: Battery/Balanced/Performance visible CPU/RAM suggestions edit only the settings draft with host reserve and guest minima. Single fresh review nested Linux cgroup over-allocation reproduced RED→GREEN; unknown layouts now disable suggestions, keeping manual controls.174Rust passed/9ignored, strictlint/Mac core/desktop build passed; actual owned EN/VI UI changed draft3CPU16384MiB while source config stayed unchanged. Initial full-suite QEMU launch failures were missing tool PATH activation, corrected without product/test changes. Minor deferred: insufficient-capacity presets are disabled without a specific explanation; documented minima/reserve and manual fallback. Native battery/pressure and full P5 remain pending.

P4 verified backup details: shared read-only manifest/file/header/config verification powers CLI checkpoint-info and async desktop label/UTC/count/size. MCP22nd tool checkpoint_info accepts only completed session-owned IDs and returns a job.175Rust passed/9ignored,14MCP, strictlint/Maccore/desktop and real sparse256GiB start/end restore/corruption/source-preservation passed. Forged runtime-verification metadata is never trusted. Single fresh batch review found no correctness findings; actual EN/VI UI metadata inspected from an owned small checkpoint. Full Windows protected restore, independent clones, video and physical Mac gates remain open.

P7 library polish: name/type search and clear, Unicode lowercase and whitespace trim; selected VM/bookmarks preserved. Two actual rendered-text regressions RED→GREEN,177Rust passed/9ignored, strictlint/build passed. Fresh review filter/state sound; small-sidebar width finished as requested responsive UI work using actual EN/VI captures. A mistaken layout scope caused existing recovery tests to fail locally, was corrected before commit, and the full suite is green. Real private-display search/clear inspected and bookmark files unchanged. No native Mac claim.

Preview17 preflight CI38674363486/e2d5ee4 retry failed ARM smoke because its catalog assertion still expected21 rather than22 tools. Reproduced locally RED, retained strict count/uniqueness and added required-backup-tool/unknownID checks. Real stdio MCP test+test_arm tasks and actual disposable ARM UEFI startup/duplicate/pause/resume/PNG/input/disconnect/adoption/shutdown cleanup passed GREEN with source NVRAM unchanged. See ../evidence/mcp-preview17-smoke.json. Publication remains pending corrected CI.

Preview.17 published fromf0893e928e801fc74e9a5ed674e73fdef097273c. Required mainCI38738680227 and all tag workflows38739130031/38739129906/38739129891 Success. Independently downloaded DMG/ZIP/Windows-agentZIP hashes verified; DMG UDIF footer, Mac ARM64 Mach-O CLI/desktop + embedded revision and Windows ARM64 PE + inner executable hash checked. DMG13885370bytes,SHA256015cfb67028480718406e905ef949df187d21e624faa820b9bf1361f134bb6d3. See ../evidence/release-preview17.json. The remaining source/design and native acceptance inventory is remaining-work.md; full roadmap not complete.


### 2026-10-15 — P2 explicit guest-to-host copies

Added independent default-off guest outbox selection (`--export-root`), v11 bounded reads/final SHA-256, `file-receive` CLI and asynchronous English/Vietnamese desktop controls. The host selects a destination folder (including external storage), preserves conflicts and publishes only after size/hash verification. One read per agent; disconnect/idle expiry revoke its token. This completes an explicit copy flow in both directions, not live Windows folder sharing or synchronization.

184 Rust checks passed/9 ignored,14 MCP checks, strict workspace/WindowsARM lint, WindowsARM64 agent build and Mac core cross-check. Actual Linux agent + host CLI transferred3MiB with Unicode names and matching SHA-256, preserved conflicts and received empty files. Fresh review caught synchronous full-file hashing exceeding the5s RPC deadline; an8GiB sparse-source regression failed before the fix and passes with hashing per bounded read and final source-metadata verification. A real transport check caught and fixed v11 replies mislabeledv1. EN/VI native UI controls also received matching bytes from the actual Linux agent over an owned private socket; the folder chooser supplied a fixed owned path, with no VM boot. Native Mac/WindowsARM/external-drive and VM channel acceptance remain open. Minor deferred: cancellation through the shared RPC path can show existing clipboard-specific text. Evidence: `docs/evidence/p2-guest-export.json`.

Preview.18 published from52a5780 after required native source/tag checks. Native Mac packaging and WindowsARM64 publication succeeded; all three publicly downloaded payloads were checksum/format/architecture verified. See `docs/evidence/release-preview18.json`. Native VM/Mac acceptance and the remaining product roadmap stay open.
