---
id: stage-cli-session-semantics
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

# Honest CLI session behavior

## Accepted outcome

Expose one-shot baseline, watch and configuration limitations accurately with actionable errors and tests. Keep persistent MCP as stateful path. No new session service.

## Local design and progress

Implementation verified and stable for parent review; loaded Workbench 0.19.0
matches conventions.
Keep the one-shot transport and shared handlers. Before connecting, reject
`spatial_delta`, all watch operations (including the misleading fresh empty list),
configuration updates, and actions requesting `return_delta: true` with a JSON
`persistent_session_required` usage error (exit 2). Reject the action before any
engine mutation. Explain `stage serve` as an MCP-client command, with snapshot,
watch/config, action and delta calls in that same session. No daemon or disk state.
Preserve `spatial_config {}` (including null optional values) as a defaults read,
and ordinary observations, actions without delta, and addon-owned clips.
Configuration detection uses the existing typed parameters, not another schema.
Binary tests capture status and JSON; existing real-engine CLI journeys supply
successful observation/action coverage under the parent's combined verification.
Parent owns standard review, foundation reconciliation, and closure.

### Verification evidence

- `cargo test -p stage-server --test cli_session_semantics --test cli_binary`:
  8 ordinary binary tests passed (before addition of the engine case).
- `cargo test -p stage-server --test cli_session_semantics`: 2 passed;
  the one engine case was then run explicitly.
- `cargo test -p stage-server --test cli_session_semantics -- --ignored --test-threads=1`:
  1 passed against real Godot; snapshot succeeds, next invocation's delta is
  rejected, and empty config read succeeds.
- `cargo test -p stage-server --test cli_journeys -- --ignored --test-threads=1`:
  all 5 real-engine journeys passed, including mutations, observations and clips.
- Scoped rustfmt and `git diff --check` passed. No custom build directory created.

### Parent reconciliation needed

Update `docs/CONTRACT.md` session semantics and `docs/JOURNEYS.md` session/config
instructions to name the pre-connection CLI usage rejection and retained empty
config read. Public CLI/session guides need the same limitation, not changes to
MCP parameter schemas. The owned Stage skill paragraphs have been updated.
No MCP handler, wire schema, shared catalog, index, or foundation was changed.

## Closure evidence

Meaningful stable-interface tests and affected real-engine journeys pass. Reconcile
affected durable truth and generated references. Complete one standard implementation
review; parent supplies combined workspace verification before closure.

## Assessment context

The accepted outcome above governs scope; the following preserves the source evidence.

# Make CLI and MCP session guarantees explicit

Parked from the Godot architecture assessment at Nathan's request.

`crates/stage-server/src/cli.rs` creates fresh SessionState for every invocation;
`crates/stage-server/src/mcp/delta.rs` requires a pre-existing snapshot baseline.
A snapshot in one CLI process therefore cannot establish the next process's delta
baseline. Watches and session configuration have similar lifetime concerns.
The distributed Stage skill describes identical capabilities while noting only
some persistence limitations.

Keep persistent MCP as the straightforward stateful path and document/express
one-shot limitations accurately. Consider a persistent CLI session mechanism only
if real CLI workflows justify its operational cost; do not introduce another
background service solely to make an equivalence claim true.

Source behavior is verified; no new runtime exercise was performed. Related:
[project/run identity](godot-project-session-identity.md).
