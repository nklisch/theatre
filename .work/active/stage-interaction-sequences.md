---
id: stage-interaction-sequences
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

# Bounded interaction sequences

## Accepted outcome

Use existing runtime actions for bounded input sequences, reliable input release, frame advancement and observation. No deterministic gameplay claim.

## Closure evidence

Meaningful stable-interface tests and affected real-engine journeys pass. Reconcile
affected durable truth and generated references. Complete one standard implementation
review; parent supplies combined workspace verification before closure.

## Design

**Primary lens:** new work, bounded engine action lifecycle.

Use one bounded engine-side sequence of named InputMap presses/releases and
physics-frame steps. Require an explicitly paused game, matching existing frame
advance semantics, and leave it paused after completion. Validate all steps and
action names before changing input. Bound total frames and payload so a request
has a useful completion window. The caller then observes pixels and spatial data
through existing tools; sequence success does not imply deterministic gameplay.

The engine, not an agent-server loop of requests, owns playback and input release.
A server-side press/await/release loop can leave input stuck if the client exits
mid-request. Extend the existing deferred-action lifecycle with a small sequence
owner, release sequence-held actions on completion, failure, timeout or owning
connection loss, and reject competing time-control requests while playback owns
advancement. No generic macro language, transaction service, key/mouse recorder,
or persistent playback storage belongs in this feature.

Use the existing deferred frame-advance counter and request owner rather than
a second timing system. Each step changes named inputs then advances its frame
count. A sequence owns those inputs until final release. Limit a request to 64
steps and 600 total frames, with an engine wall-clock deadline of 30 seconds.
The server waits slightly longer than that deadline so engine cleanup can report
a timeout. These bounds prevent an abandoned request from holding inputs
indefinitely; callers can compose longer interactions through separate requests.

The current poll path stops reading sockets while advancing. During sequence
playback it must still detect owning connection loss and honor the deadline.
Reject competing action/time-control requests without disturbing the owner, and
do not apply ordinary idle expiry while a valid sequence remains in progress.
Use one explicit pending-action owner at this boundary; do not add a registry.

Test held input changes a real player over bounded frames, release
occurs at the end, malformed later steps change nothing, and disconnect cleanup
leaves no sequence-held action or unintended unpaused game.

One standard Astra design pass completed. Reject zero-frame steps during whole
request validation, before changing input: the existing deferred advance counter
does not complete a zero-frame request. Test that rejection and no side effects.
Engine-side deadline cleanup requires the engine to keep running callbacks; do
not promise cleanup while the process is hung or stopped by a native debugger.

The Stage identity, viewport and diagnostic surfaces are integrated and stable.
Sequence implementation can use existing engine-start fixtures independently of
Director run controls. The final cross-tool journey still waits for those controls;
they are not a code prerequisite for this engine-owned action.

## Post-review correction evidence

- `CARGO_TARGET_DIR=/storage/cargo-target cargo build -p stage-godot` and
  `CARGO_TARGET_DIR=/storage/cargo-target cargo run -p theatre-cli -- deploy tests/godot-project`
  passed before the final engine journeys.
- The four existing native sequence regressions passed individually through
  `cargo test -p stage-wire-tests <exact-test-name> -- --ignored --exact --nocapture`:
  movement/release/pause, whole-request validation and pre-held ownership,
  competing-action rejection, and owner-disconnect cleanup.
- `stopping_an_idle_listener_does_not_pause_gameplay` passed against real Godot.
  Its deferred Player helper stops the idle server, records pause state before
  restarting the same port, and the ownership-preserving harness reconnects to
  prove gameplay remained unpaused.
- `sequence_deadline_releases_input_and_restores_pause` passed against real Godot
  in 34.35 seconds. The fixture launches without accelerated `--fixed-fps` for
  this wall-clock case, lowers `Engine.physics_ticks_per_second` to one, receives
  the real 30-second `sequence_timeout`, proves the owned action is released,
  and successfully advances another frame to prove the scene is paused.
- `CARGO_TARGET_DIR=/storage/cargo-target cargo clippy -p stage-wire-tests
  --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and scoped
  `git diff --check` passed. No production timeout knob or framework was added.

### Standard review parameter correction

The one standard Astra pass found that top-level sequence fields could be silently
dropped, and that stopping an idle listener could pause gameplay. The builder now
rejects steps on other actions and all other action-specific fields on sequences;
return_delta remains meaningful for persistent sessions. Optional echo preserves
explicit false so it cannot disappear from this validation, while key injection
still defaults to false. A 20-field MCP rejection matrix proves no engine dispatch;
CLI rejection tests prove failure before connection. Idle cancellation now does
nothing without a pending owner. Four sequence integration tests, three CLI
session tests, 39 action unit tests and scoped clippy passed. Native idle/deadline
and active-owner evidence is recorded above by the correction worker. No new
parameter framework or production timeout control was introduced.
