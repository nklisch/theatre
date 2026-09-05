---
id: agent-godot-development-loop
kind: epic
status: active
tags: [agents, godot, integration]
parent: null
blocked_by: []
related_to: []
research_refs: []
mock_refs: []
created: 2026-09-04
updated: 2026-09-04
---

# Complete the agent Godot development loop

## Outcome and boundary

Deliver the ten accepted features below, covering the original eight assessment items.
An agent identifies the intended project, preserves human edits, authors and validates
a durable change, runs it, interacts, observes pixels and state, then restarts and
verifies persistence. The script feature is a learning outcome, not production tooling.
Exclude the two setup findings about listener binding and install/deploy coverage.
Preserve unrelated uncommitted setup changes and the local setup completion item.
Do not change roadmaps, publish releases, or edit external plugin settings.

## Accepted editor behavior

Nathan chose undoable live editor edits with explicit save. Preserve existing human
changes; individual and batch edits leave the open scene unsaved until saved.
Headless resource edits persist to disk. Never silently save unrelated human work.

## Delivery topology

Astra owns requirements, design, integration, foundation/catalog edits and closure.
Use autonomous execution inside this boundary, adaptive delegation, balanced
simplification, standard review, and no history rewriting. Preserve the dirty
setup baseline; use bounded working-tree review targets rather than mixing it into commits.

### Loadout and isolation

Confirmed lineup: Astra medium for coordination, high for coupled identity/editor/run
design, low-medium for settled implementation; Sol low-medium for bounded independent
implementation; Luna high for source evidence; fresh-context Astra medium for review.
Nathan explicitly selected Astra review after both GLM providers exhausted quota. Maximum two implementation workers. Use the
shared checkout with exact owned files and parent-owned shared router integration.
Read-only designers/reviewers may run alongside non-overlapping implementation.

### Waves and evidence

1. File guidance and CLI session semantics while investigating identity/editor contracts.
2. Project identity, then consistent authoring.
3. Run lifecycle and immediate viewport, then interaction sequences. API discovery can overlap.
4. Script comparison, remaining guidance, combined real-engine journey.

Formal designs receive one standard pass before implementation; completed features
receive one integrated review each. No duplicate review of corrected targets.
Workers run targeted checks and return evidence. Parent owns full workspace build,
format, clippy, deployment, ordinary and explicitly ignored tests from conventions,
plus graphical editor and viewport acceptance. Unavailable evidence blocks completion.

### Current state

Activated from Nathan's approved plan. Godot 4.7.1 is available. Prior setup changes
are uncommitted. Baseline workspace build passed. File guidance and CLI workers
are active; identity, viewport and API discovery designs await their one review.
Astra designer is investigating complete native-undo authoring. Initial workspace
tests reached a stale rule assertion in the in-progress guidance change; its worker
now owns the correction. Godot 4.7.1 graphical environment is available.

## Features

- [Practical Godot file-access guidance](agent-godot-file-access-guidance.md)
- [Reliable project and run identity](godot-project-session-identity.md)
- [Consistent native authoring](director-consistent-authoring.md)
- [Honest CLI session behavior](stage-cli-session-semantics.md)
- [Selected-scene run lifecycle](agent-godot-playtest-loop.md)
- [Immediate viewport observation](stage-immediate-viewport.md)
- [Bounded interaction sequences](stage-interaction-sequences.md)
- [Targeted engine API discovery](godot-engine-api-discovery.md)
- [Compare scripted authoring with batches](godot-script-authoring-escape-hatch.md)
- [Accurate distributed guidance](godot-agent-foundation-alignment.md)

### Added research boundary

Nathan requested Terra research on current Godot APIs, human click/shortcut-to-agent
feedback, and the sibling Krometrail Rust crate integration. Three Terra medium
source lanes gather evidence under Workbench research; parent owns synthesis and
source-support verification. This authorizes research, not raising Godot's minimum
version or building a new human interaction UI. Existing epic implementation
continues. Revisit unimplemented design only when fetched evidence changes it.

Early integrated verification passed ordinary tests and progressed through clippy,
deployment and ignored engine tests, but its final rustdoc invocation collided
with in-progress identity source changes and stale compiled protocol metadata.
Do not treat that moving-target run as final acceptance; rerun the complete suite
at a stable integration point. Site dependencies were installed from the lockfile
and the public documentation build passed.

Nathan also confirmed selected-scene-only explicit saving: use checked native
PackedScene/ResourceSaver serialization, retain undo, do not flush unrelated
external resources, and explicitly report that the native dirty marker may remain.

### Approved Godot minimum-version change

Nathan explicitly approved raising the minimum to Godot 4.7. Align extension
bindings, deployment/runtime compatibility declarations, CI and public guidance
with that floor, and verify on the installed 4.7 engine. Native 4.7 APIs may now
replace workarounds for older engines where useful. Choose the binding upgrade
from verified available API support; do not assume an api-4-7 feature exists in
the current dependency. This does not authorize preview engines or change the
selected-scene-only save policy.

#### Stage binding migration evidence

The Stage engine boundary now declares `godot` 0.5.5 with `api-4-7`,
`experimental-godot-api`, and `lazy-function-tables`; `Cargo.lock` resolves the
0.5.5 binding family. `stage-godot` declares Rust 1.94 because that is the
binding's required compiler floor. Validation used installed Rust 1.96.1 rather
than claiming a Rust 1.94 minimum-version test. The extension manifest minimum is
4.7, and CI's real-engine job selects Godot 4.7.2 stable.

The required 0.5 source migration is confined to the Stage engine boundary:
nullable scene-tree handling uses `get_tree_or_null`, required scene-tree roots
use the native non-optional return, variant dictionaries follow typed `AsArg`
borrowing, and Godot-exposed frame signals use Godot's signed `i64` integer
boundary while capture counters remain unsigned internally. Const-qualified
getters no longer carry unnecessary mutable bindings. The internal TCP status
method was renamed to `has_stage_connection` with its GDScript caller updated,
avoiding a new 0.5 base-method shadowing warning. No compatibility shim or
capture-semantic change was added. The gdext skill now reflects the 0.5.5/4.7
setup and these source rules.

Verification on installed Godot 4.7.1 stable and Rust 1.96.1:

- `cargo fmt -p stage-godot -- --check` passed.
- `cargo clippy -p stage-godot --all-targets -- -D warnings` passed.
- `cargo build -p stage-godot` passed.
- `cargo test -p stage-godot` passed: 40 tests.
- `cargo test -p stage-server --test viewport` passed: 1 test.
- `cargo test -p stage-server --test viewport_engine -- --ignored --test-threads=1 --nocapture`
  passed both isolated real-engine journeys. Graphical captures decoded at
  1280×720 and 2048×1152 with recording disabled; measured requests were 54.1 ms
  and 66.9 ms. The headless journey explicitly reported unavailable pixels while
  spatial observation remained available.

### Human feedback extension — design pending

Nathan selected both running-game and editor feedback, with queued evidence
surfaced by a small notice on the next tool/API response. Evidence should be
retrievable as selection/pointer context, image and an optional note. He proposed
an optional Claude/Codex plugin or hook for a working agent. Verify actual client
capabilities before choosing the integration: next-boundary context injection is
not necessarily asynchronous steering. Automatic idle-agent wake is not selected.
Terra is researching existing distribution and official client hook contracts;
implementation/design boundaries remain to be settled from that evidence.

The Godot4.7/gdext0.5 migration passed one standard Astra implementation review
without material defects. Corrected the old installation version example and
addon-skill connectivity accessor. Also replaced the skill's impossible editor
lookup of the game autoload with the actual debugger-message boundary. Rust1.94
is the declared dependency floor, not an independently tested compiler here;
CI4.7.2 remains configuration evidence until executed.

## Integrated verification checkpoint

Workspace build, all non-ignored workspace tests (including documentation tests),
and workspace/all-target warnings-denied clippy passed after the review fixes.
The earlier rustdoc runtime-module failure did not recur. Generated tool schemas
and the site build also passed. Full ignored/native acceptance remains open:
import-progress keyboard interception was corrected in the isolated editor fixture,
but its full mutator journey subsequently reproduced an AccessKit duplicate-child
crash under Godot 4.7.1. Native-only comparison and stable-patch investigation are
continuing; this checkpoint is not final acceptance or closure.

## Authorized rollout checkpoint

Nathan explicitly requested that the intermittent accessibility investigation be
backlogged and not block preparing Theatre for another run. It is captured in
`.work/backlog/godot-editor-accesskit-undo-crash.md`. Local release installation,
Voxlar setup and GitHub checkpoint publication proceed now. The ordinary
workspace gates passed; this is not a claim that the complete ignored suite or
the single combined cross-tool acceptance journey has passed. Preserve those
verification limits rather than treating the parked investigation as a fix.
