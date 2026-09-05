---
id: godot-project-session-identity
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

# Reliable project and run identity

## Accepted outcome

Verify fresh and reused Director editor connections against the requested project. Expose actual project and run identity. Prove wrong-project edits cannot silently succeed.

## Closure evidence

Meaningful stable-interface tests and affected real-engine journeys pass. Reconcile
affected durable truth and generated references. Complete one standard implementation
review; parent supplies combined workspace verification before closure.

## Assessment context

The accepted outcome above governs scope; the following preserves the source evidence.

# Reliable project and running-game identity

Parked from the Godot architecture assessment at Nathan's request.

Agents should be able to identify the selected project, connected editor, and
running scene rather than infer identity from a port or a successful call.

`crates/director/src/backend.rs::try_editor` reuses its cached editor connection
based on liveness without checking the requested project; `EditorHandle` in
`crates/director/src/editor.rs` contains a stream and port, not project identity.
The fresh connection path also does not establish the editor's project identity.
This creates a source-supported wrong-project routing risk when projects switch.
No live multi-project reproduction was performed.

Investigate project-aware connection selection and an agent-visible status view
that ties Director and Stage to the intended project/run. Retain the useful
editor/runtime separation; a unified status experience does not require merging
all processes or introducing a new service.

## Design

**Primary lens:** defect or reliability, with cross-process integration.

### Chosen approach

The editor is the authority for its project, not the requested path or TCP port.
Extend its read-only ping with engine project path and process identity. Verify
that response before the first operation, and key cached reuse by canonical
project path and resolved port. A different requested project must establish a
new verified connection. Native filesystem canonicalization treats symlink
aliases as the same project. Identity failure before dispatch may use the existing
headless fallback for the requested project; it must never send the mutation to
the unverified editor. Once an editor operation is dispatched, transport failure
is an uncertain outcome, not permission to replay it on another backend.

Expose project and process identity in Director editor status. Add engine-owned
runtime identity to the Stage handshake: actual project root, process id, and a
run identifier stable across client reconnects but different after a game restart.
Keep client connection session id separate. Stage verifies the actual project
before publishing a connected state or pushing project configuration when its
selected project directory contains project.godot. Outside a selected project,
allow connection and report the actual identity rather than refusing discovery.
Use shared stage-protocol types, not a separate identity registry or service.
A focused runtime status tool reports connected/readiness and current scene;
query current scene rather than presenting a launch-time path as current state.

### Alternatives

Port-only selection fails with project switching and reused connections. Merely
reporting the mismatch after a mutation cannot protect the project. A separate
session manager or project registry introduces another lifecycle without need.
Always reconnecting is unnecessary when a connection belongs to a verified
immutable editor project. Do not claim that identity provides authentication.

### Verification and integration

Use two temporary project roots and controlled TCP peers to reproduce fresh and
cached wrong-project routing. Assert that rejected peers receive no mutation.
Verify canonical path aliases and changed port selection. Exercise real Godot
editor ping/status, Stage identity across reconnects and game restart, and an
incorrect selected Stage project before config push. Existing mocked editor
fixtures must report their real fixture project identity, not bypass verification.

Own editor/backend routing, plugin ping and status, shared Stage handshake,
Stage runtime/server identity and focused boundary tests. Router/CLI/schema
integration belongs to the parent to avoid collisions with concurrent features.

### Reviewed verification refinements

One standard Astra design review completed. Add explicit fresh and cached
post-dispatch disconnect regressions: a peer consumes the mutation and closes
without replying; assert uncertain outcome and no reconnect, replay or headless
execution. Disconnected runtime status must not present retained identity as
current. Preserve actionable project mismatch diagnostics rather than replacing
them with a generic handshake timeout. Daemon retry behavior is outside this
editor-scoped identity feature and remains unchanged here.

## Implementation progress

Verified editor connections now bind canonical project identity and selected port.
Fresh/cached post-dispatch failures return an unknown-outcome error without editor
reconnect, replay, or headless fallback. Status reports engine project/process.
Stage shares engine-owned runtime identity across handshake and current-state
queries; selected-project mismatch blocks connection/config publication and retains
an actionable diagnostic. Both persistent and one-shot disconnects clear live
identity. `runtime_status` is routed through MCP, output schema, and CLI without
changing the completed CLI session restrictions. The engine identity module is
public within stage-godot for the imminent viewport implementation.

Implementation verification passes:

- Director identity transport regressions: 6 tests, including fresh/cached mismatch,
  changed port, symlink alias, and fresh/cached unknown post-dispatch outcome.
- Stage runtime identity boundaries: 6 tests, including mismatch before ACK/config,
  current-scene refresh, discovery outside a project, symlink selection, disconnect
  during a query, disconnected CLI result, and routed output schema.
- Real Godot editor ping/status and headless status agree on actual project/process.
- Real Stage reconnect preserves run identity but changes client session; game
  restart changes run identity, reports the new scene, and rejects a wrong project.
- Existing Director library tests (36), Stage TCP mock tests (52), Stage protocol
  tests (91), and ordinary/ignored CLI session-semantics tests pass.
- Targeted four-crate clippy with all targets and warnings denied, owned-file
  formatting checks, and `git diff --check` pass. No whole-workspace suite run here.

Stage's required handshake identity changes the wire protocol to v3; matched
server/addon deployment is required. The test addon binary was atomically replaced
so existing mapped engine libraries were not overwritten in place. Shared build
storage was reused; no isolated Rust build directories were created.

Parent owns combined verification, foundation/catalog reconciliation, standard
implementation review, generated references, graphical acceptance, and closure.
No commits were made.

### Integration review

One standard Astra implementation review accepted the identity boundary with no
material findings. Removed the stale fixed tool count from the contract. Targeted
identity evidence and integrated code unblock dependent implementation; final
combined verification and generated-reference reconciliation remain closure gates.
