# Viewer Scheduling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Keep due snapshot captures from starving under continuous viewer input while retaining bounded queues and no mutation replay.

**Architecture:** Extract a pure scheduling policy from the existing single-worker Viewer. Keep request execution, action storage and target checks in Viewer; introduce no transport or protocol change.

**Tech Stack:** Rust1.99, egui/eframe, existing guest RPC, Python private-socket acceptance fixture.

**Spec:** `docs/superpowers/specs/2026-10-16-viewer-scheduling-design.md` (proposed continuation design; planning is not implementation acceptance).

## Global Constraints

- Keep at most32 queued input actions and one RPC worker.
- When live/target/due conditions hold, choose capture after at most8 completed input actions; no input reorder or replay.
- Successful capture schedules the next periodic frame after1second; retain640×480 JPEG and existing RPC/frame bounds.
- Keep explicit manual refresh, legacy agent behavior and current conditional capture-timeout retry; add no mutation retry.
- No new dependencies; English/Vietnamese UI; no FPS, latency, native Mac or full Coherence claim.

## Review Focus

- Continuous input: a due capture gets a turn even while queue stays nonempty — Task1 fairness test and Task2 transport trace.
- Slow guest: no second request starts while the worker is busy — Task1 busy test and Task2 held-reply fixture.
- Live disabled: no automatic capture, but input and manual refresh still work — Task1 disabled test and Task2 UI acceptance.
- Target change during capture: stale frame is discarded and pending input never follows the old target — Task2 target-switch fixture.
- Input failure with pending actions: live stops, pending actions clear and no mutation is retried — Task2 failure fixture.

---

### Task1: Implement and integrate fair snapshot scheduling

**Files:** Create `crates/desktop/src/viewer_schedule.rs`; modify `crates/desktop/src/main.rs` module declaration and `crates/desktop/src/app_ui.rs` scheduler state/dispatch. Unit tests live in the new module.

**Interfaces:**
- Create `enum Decision { Idle, Input, Capture }`, deriving Debug/PartialEq/Eq.
- Create `struct Schedule { next: Instant, completed_inputs: u8 }` with `new(now: Instant) -> Self`, `choose(&self, now: Instant, busy: bool, live: bool, has_target: bool, queued: bool) -> Decision`, `input_completed(&mut self, now: Instant, queue_empty: bool)`, `frame_completed(&mut self, now: Instant)`, `reset(&mut self, now: Instant)`, and `deadline(&self) -> Instant`.
- Input count saturates at8. `choose` is pure: busy→Idle; live+target+due+(burst8 or emptyqueue)→Capture; otherwise queued→Input; otherwise Idle. `input_completed` increments count and, if queue emptied, brings next deadline to now without moving an already-overdue deadline later. `frame_completed` resets burst and sets now+1second. `reset` starts due with burst0.
- Viewer owns queue/worker, invokes completion methods only for successful corresponding events, and uses deadline for repaint. Scheduler never owns/actions or performs RPC.

- [x] Add tests `due_frame_gets_a_turn_after_eight_inputs`, `busy_never_dispatches`, `live_disabled_keeps_input`, `future_frame_does_not_delay_input`, `idle_due_frame_dispatches`, `input_does_not_postpone_due_frame`, `reset_clears_burst` and `frame_completion_sets_one_second_deadline`. Use a fixed Instant and deterministic Duration offsets; assert exact Decision sequences and deadlines, no sleeps.
- [x] Run `cargo test --locked -p freelane-desktop viewer_schedule` and record failure from the missing module/types before implementation. Then implement the policy, rerun and require all named tests pass.
- [x] Replace Viewer `next` with Schedule; preserve manual capture guards, selected-target checks, queue32 refusal, existing worker/error handling and retry conditions. After successful input, call input_completed with actual queue emptiness. On inventory/target reset call reset; on matching successful frame call frame_completed. Keep stale-frame branch requesting a fresh frame for current target.
- [x] Run `cargo test --locked -p freelane-desktop` and `cargo clippy --locked -p freelane-desktop --all-targets -- -D warnings`; require successful exits and executed scheduling tests.
- [x] Commit the integrated policy: `git commit -m "fix(viewer): prevent snapshot starvation during input"` with only the listed source changes.

### Task2: Verify actual dispatch, target safety and UI behavior

**Files:** Create `tests/viewer_schedule_acceptance.py`; extend focused Viewer tests in `crates/desktop/src/app_ui.rs` only where a pure state test can exercise real event handling; create `docs/evidence/p3-viewer-scheduling.json`; update `docs/app-windows.md`, `docs/plans/progress.md` and release notes after measured results.

**Interfaces:** Consume Task1 Schedule and existing Viewer request/event paths. Fixture uses only owned profile/config/socket/display/processes; replies use existing WindowTarget and bounded JPEG outcomes. Do not add a public test control endpoint. If event handling cannot be driven without copying implementation, extract the actual handling function in app_ui.rs and test that path.

- [x] First create a private authenticated Xvfb/native viewer fixture with a synthetic guest socket and valid owned profile. Record request types/target identities only. Hold one reply while injecting more events; assert no second request arrives until released. Feed continuous focused input; after8 successful inputs while a frame is due, assert capture precedes input9. Run against pre-fix source/binary to establish the starvation failure without changing assertions or user VMs.
- [x] Exercise selected target change while capture is pending; release old reply and assert it cannot become the selected texture. Queue input then return a transport error; assert pending input clears, live stops and no request replay occurs. Use bounded waits and always terminate only fixture-owned processes.
- [x] Run the fixture against Task1 and require all dispatch/target/no-replay assertions pass. Test live off with input and explicit manual refresh in owned EN/VI viewer windows, capture and inspect actual screenshots. No visual snapshot assertions that mirror layout.
- [x] Run `cargo fmt --all --check`, `cargo test --locked --workspace`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `python3 -m unittest discover -s tests/mcp -v`, and platform checks affected by the final diff. Record actual counts/ignored checks; require required native CI green. Existing tool activation/setup instructions apply.
- [x] Request one fresh whole-batch review under requesting-code-review; fix Critical/Important and record minors/rulings. Update evidence/docs with actual source, limitations and results; commit `test(viewer): verify fair dispatch and stale frame handling`.
- [ ] Push the reviewed source using existing authorization. Only after required CI succeeds, prepare the next unused preview tag, publish through existing workflow and verify downloaded payload checksums/architectures/revision. Update EN/VI README release links. Do not reuse preview.21 or claim physical Mac acceptance.

## Self-review and handoff

Both tasks consume the same Schedule API; no protocol or guest change is required. All five review conditions map to explicit tests/fixtures. Only Task2 publishes evidence; proposed limits are acceptance expectations until measured. Implement inline with one whole-batch review, matching the existing session workflow; no new execution-method choice is required. This plan does not implement subsequent video, sharing, backup or hardware subsystems.
