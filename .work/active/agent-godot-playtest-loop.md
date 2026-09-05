---
id: agent-godot-playtest-loop
kind: feature
status: active
tags: [godot]
parent: agent-godot-development-loop
blocked_by: []
related_to: []
research_refs: []
mock_refs: []
created: 2026-09-04
updated: 2026-09-04
---

# Selected-scene run lifecycle

## Accepted outcome

Start a selected scene, stop, restart, report actionable current-run diagnostics and distinguish process start from Stage readiness. Integrate with verified project/run identity.

## Closure evidence

Meaningful stable-interface tests and affected real-engine journeys pass. Reconcile
affected durable truth and generated references. Complete one standard implementation
review; parent supplies combined workspace verification before closure.

## Assessment context

The accepted outcome above governs scope; the following preserves the source evidence.

# Repeatable agent-driven run, interact, and observe loop

Parked from the Godot architecture assessment at Nathan's request.

The useful agent workflow is read → change → validate → run → interact → see →
diagnose. Theatre has many constituent capabilities, but the inspected public
surface lacks selected-scene run/stop/restart and immediate viewport capture.
Shell-launching Godot is possible; integrating readiness, logs, and runtime
attachment could remove coordination guesswork.

Existing building blocks:
- `crates/director/src/mcp/mod.rs`: `project_reload` and `editor_status`.
- `crates/stage-server/src/mcp/action.rs`: input, pause, and frame advancement.
- `crates/stage-server/src/mcp/clips.rs`: saved-clip screenshots and visual artifacts.

Explore a small run lifecycle surface, immediate screenshot plus optional
selected-node state, and input sequences with bounded frame advancement and
observation (for example hold right, jump, release, then inspect). Do not promise
deterministic gameplay merely because inputs are scheduled by frame.

Current screenshots need not wait for the broader
[live-ring visual queries](../backlog/live-ring-visual-queries.md) idea. Structured spatial
state and pixels answer different questions; preserve both. Keep diagnostics
focused on actionable file/line errors and the current run rather than building
a replacement code debugger or testing framework.

This is a workflow hypothesis, not a settled API. Validate its benefit through
real author/change/run/verify sessions. Related:
[agent perception evaluation](../backlog/agent-perception-eval-corpus.md).

## Design

**Primary lens:** new work, editor/runtime lifecycle integration.

### Chosen approach

Add one Director run-control tool with start, stop, restart and status actions.
Use the verified editor connection and native EditorInterface lifecycle rather
than creating another child-process owner. Start/restart select a saved scene;
status reports the editor's actual playing state and selected scene. Stop is
idempotent. If no verified editor is available, return an actionable editor-required
result; do not silently start a differently owned headless run.

Run the saved scene without implicitly saving open human work. Scope the native
run/auto_save/save_before_running setting to false around the synchronous play
call and restore its original value immediately, without yielding or persisting
settings. A temporary real-editor probe on installed Godot 4.7.1 confirmed native
play starts while both unsaved live edits and the previous saved file survive.
The setting probe used isolated editor configuration and removed all artifacts.

Director owns native play state, not Stage readiness. A successful start means
launch requested, not runtime attached. The agent checks Stage runtime_status,
verifies project and current scene, and compares run identity across restart.
Keep these two explicit authorities instead of a third session service or a
second Stage connection inside Director. Surface errors with the selected
project/scene and recovery action; use existing project_reload for file/line
validation and editor_status for editor log context. Do not label historical
editor log lines as current-run diagnostics.

### Alternatives and verification

A Rust-owned launch process duplicates Godot editor lifecycle and makes one-shot
CLI ownership surprising. Waiting for arbitrary fixed sleeps cannot establish
Stage readiness. Forcing a save before play violates explicit-save collaboration.

Exercise the actual editor through Director: start selected saved scene, observe
native running state, obtain matching Stage runtime status, stop, and restart with
a new runtime identity. Verify missing scene/editor errors and repeated stop.
Keep an unsaved human edit throughout, proving neither start nor restart saves
it or discards it. Verify the scoped editor setting is restored. Use existing
validation diagnostics to explain a deliberately invalid script and then run the
fixed scene. Do not claim every runtime error is captured by Theatre.

### Review disposition and remaining readiness

One standard Astra design pass identified a real gap: existing project_reload
and editor log context do not provide current-run diagnostics. Do not silently
narrow the accepted feature to launch/validation errors. Terra is checking native
Logger and debugger-session interfaces for a small bounded runtime error collector
attributed by engine-owned run identity. Settle this supported subset and its
threading/source semantics before implementation; do not build a debugger framework.

### Current-run diagnostic source settled

Terra's fetched Logger and EditorDebuggerSession evidence is retained in
.research/briefs/godot-agent-interaction-interfaces.md. Use a small runtime-owned
GDScript Logger registered early by the Stage autoload. Retain a bounded queue of
structured errors, warnings, script errors and shader errors under a Mutex.
Callbacks may arrive on worker threads: no scene-tree traversal, network writes,
recursive logging or captured variables in them. Read the queue on the Godot main
thread through a focused runtime_diagnostics query/tool, reusing engine run identity.
This keeps potentially large diagnostic text separate from cheap runtime_status.

Expose bounded origin/message/backtrace data, explicit retained/omitted counts,
and the actual run identity. Do not equate readback frame with error occurrence.
Honor the existing response-budget machinery rather than dumping the whole queue.
Retain diagnostics across client reconnects, reset with the game process, and
report unavailable collection rather than stale editor logs if no runtime exists.
Registration cannot recover engine initialization or earlier errors; project log
settings can suppress delivery, and release builds may omit backtraces. Keep
project_reload validation, native launch failures and captured runtime diagnostics
separate in guidance. This is the supported current-run subset, not a debugger.

Verify deliberate push_error/push_warning and script-error origin in a real run,
worker-thread delivery, bounded overflow, reconnect/restart identity, and no stale
run evidence after disconnect. Native breakpoints or a hung engine can prevent
queries; do not promise diagnostics delivery while callbacks cannot execute.

## Run-control implementation evidence

Director now exposes typed `editor_run` start, stop, restart, and status actions
through MCP and CLI. The operation is editor-only, uses verified project identity,
and preserves uncertain post-dispatch outcomes without replay or headless fallback.
Native launch temporarily disables `run/auto_save/save_before_running` only around
the synchronous `play_custom_scene` call and immediately restores the in-memory
value.

Focused Director tests pass, including the real graphical Godot 4.7.1 editor
journey. That journey launches the selected saved scene while preserving an
unsaved human edit and the unchanged file, distinguishes launch request from a
separate Stage `runtime_status` readiness query, stops idempotently, restarts with
a new engine run id, checks wrong-project and missing-scene failures, and exercises
the CLI path. The shared fixture no longer forces accessibility off, preserving
default-behavior coverage without claiming universal platform safety.

Run-control verification completed with:

- `cargo test -p director`
- `cargo test -p director --test authoring_editor editor_run_controls_saved_scene_without_saving_human_work -- --ignored --nocapture --test-threads=1`
- `cargo clippy -p director --all-targets -- -D warnings`
- `cargo fmt --all` and `git diff --check`

The bounded parent-integration rerun used `cargo build -p stage-server`, then
`cargo test -p director --test authoring_editor -- --ignored --nocapture
--test-threads=1`. The run-control journey passed with the shared support payload,
default accessibility behavior and a pending-feedback CLI success notice. The
same full invocation remained red because the two sibling native-undo journeys
missed their first Ctrl+Z (1 passed, 2 failed); this does not invalidate the
run-control result or establish broader accessibility safety. Focused
`editor_run_` CLI feedback tests reported 3 passed and 5 filtered out.

Parent integration still owns generated references, affected foundation updates,
combined workspace verification, the standard feature implementation review, and
closure.

## Standard implementation review and corrections

One standard Astra pass covered native run controls and runtime diagnostics.
The review accepted the lifecycle architecture and found an empty-page diagnostic
budget bypass. Final serialized envelopes are now checked against the effective
budget, including empty queues and exhausted pages; errors explain the required
budget/hard cap without modifying retained evidence. Seven diagnostic integration
tests passed, including soft/hard-cap empty-page recovery, alongside scoped
warnings-denied clippy. Supplied engine lifecycle evidence was not independently
rerun by the reviewer. No repeat formal pass is required for this correction.
