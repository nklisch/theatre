---
id: director-consistent-authoring
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

# Consistent native authoring

## Accepted outcome

Preserve human edits. Open-scene agent edits are undoable and remain unsaved until explicit save, including batch operations. Headless edits save to disk. Align property validation, partial failures and persistence reporting. Verify with real EditorInterface, save/reopen and undo.

## Closure evidence

Meaningful stable-interface tests and affected real-engine journeys pass. Reconcile
affected durable truth and generated references. Complete one standard implementation
review; parent supplies combined workspace verification before closure.

## Assessment context

The accepted outcome above governs scope; the following preserves the source evidence.

# Consistent authoring across editor, headless, and batch operations

Parked from the Godot architecture assessment at Nathan's request.

An agent needs an edit's success to mean the same thing regardless of execution
backend: the intended content changed, rejected properties are reported, and
persistence is clear. Preserve the distinction between authored content and
runtime debugging changes.

Source inspection found that `addons/director/editor_ops.gd` routes individual
active-scene operations to live-tree mutations, but routes `batch` through
`MetaOps.op_batch` and file-based operations. The live mutations do not implement
the undo/save integration claimed in the
[pre-setup Director specification](https://github.com/nklisch/theatre/blob/2486f021bad0c81efab5a04f982614b5bc81938e/docs/director-spec.md).
`addons/director/ops/node_ops.gd` also has different unknown-property behavior
between `_set_properties_on_node` and `_apply_properties`.

Investigate shared mutation semantics with context-specific loading, undo, and
saving. Clear partial-failure and saved/unsaved results matter more than adding a
transaction framework. This is a direction to evaluate, not an accepted design.

Verification gap: `tests/director-tests/src/harness.rs` uses
`addons/director/mock_editor_server.gd`, which delegates to file operations and
does not exercise EditorInterface. A real editor journey should cover an unsaved
human edit, individual and batch agent edits, saving/reopening, and running the
result. The source differences are verified; interactive consequences were not
reproduced during the assessment.

## Design

**Primary lens:** defect or reliability, balanced simplification.

### Accepted persistence semantics

Nathan confirmed undoable edits with explicit save, then chose selected-scene-only
save after the native editor Save side effects were explained. scene_save packs
the selected live root and checks ResourceSaver.save. It preserves undo history
and does not flush unrelated edited external resources. Its response and guidance
must explicitly say the editor dirty marker may remain until a native editor save.
Do not clear history or create a second dirty-state ledger to hide this limitation.
Standalone resource/project operations keep their existing disk persistence.
A saved scene that references an externally edited resource does not imply that
resource's unsaved in-memory changes were written.

### Shared mutation context

Add a small ops/scene_edit.gd context holding a resolved root and whether the
operation owns it. File contexts load, pack/save and free their detached root.
Editor contexts borrow the actual open root, record native undo and never save
implicitly. Keep one mutation implementation in existing operation-family modules
and remove duplicate editor _live_* mutators. Native Godot objects remain on the
main thread, and the context never outlives the synchronous operation.

Read inactive open roots without switching tabs. Before mutating an inactive open
scene, activate that existing tab and verify the expected root so native history
selects the scene rather than global history. Restore the previous named tab
afterward. Actual editor probes showed inactive roots alone route to history 0;
custom_context=root without activation is not enough. Do not replace whole roots
or duplicate the scene as a generic undo snapshot: prior human actions reference
those exact objects.

Prevalidate full property/cell collections and convert values through one shared
validation path. Unknown properties cannot be silently skipped. Preserve explicit
engine failures and actual partial effects; arbitrary custom setters are not
transactional. Align node names/collisions/cycles, descendant counts, instance
ownership and reparent transforms across contexts. Preserve the existing headless
transform policy where possible and state it consistently rather than selecting
a different policy for live edits.

### Native undo

Use EditorUndoRedoManager with the active scene root as action context. Use
operation-specific inverses: old property/resource values; exact added/removed
node objects; original parent/index/name/owners/transforms for reparent; prior
membership and key presence for groups/metadata; full prior script and stored
export values; previous affected tile/grid cells including emptiness, alternatives
and orientation; exact signal callable/binds/flags. Retain additions with native
do references and removed subtrees with undo references. Never free an undoable
removal. Execute fallible engine operations once and record their actual
effect using commit_action(false), not a second execution.

Cover all existing scene mutations, including shape_create attachment. Validate
its scene target before writing a standalone shape file. Existing animation tools
operate on resource_path, not scene/player targets; keep them file operations and
do not invent scene animation tools. Prevent scene_create or destination replacement
from overwriting an already-open scene before any write, directing callers to
existing mutation operations or a new destination.

### Dispatch and results

Consolidate repeated operation dispatch in a small shared dispatcher, not a registry
framework. Batch receives the caller's individual-operation dispatcher so editor
entries use the same context as single calls. Keep sequential stop/continue behavior,
one native undo action per entry that changes the live scene, including partial failures, and rejection of nested
batches. Later entries observe prior changes. Remove open-scene disk reload sync;
scan the filesystem only for actual file writes.

Expose one typed persistence shape listing saved paths and unsaved scene paths.
It describes this operation's effects, not all editor dirty state. Carry it through
every mutating typed response, including mixed file-and-scene effects. Preserve
per-entry operation, success, data, error, context and persistence in batch output.
When batch stops on failure, preserve successful earlier results and partial
persistence in MCP error data instead of discarding them in into_data/error conversion.
Shared Rust types own output structure; generated schemas describe it.

### Verification

Use a small real-editor fixture in an isolated temporary project with the actual
Director plugin and a test-only editor driver, reusing existing framing/process
cleanup. Exercise native shortcut undo/redo, not only raw UndoRedo methods that
bypass editor manager bookkeeping. Preserve a human action and two dirty tabs;
verify correct history, live object identity and unchanged disk before explicit save.

Exercise each existing scene-mutator family through individual/batch routes:
properties, add/remove/reparent/instance, scripts/exports, metadata/groups, signals,
physics, tile/grid cells and shape attachment. Keep resource-animation journeys;
verify validation failure does not dirty a cached resource that later gets saved.
Check stop/continue batch partial results through the actual MCP handler. Verify
save/reopen/run, selected-file-only save, explicit save failures, and retained undo.
Native API use is checked on installed Godot 4.7.1. Nathan's approved 4.7 minimum
permits get_open_scene_roots and get_unsaved_scenes; parent owns binding and
compatibility-declaration migration.

### Alternatives and risks

Adding undo to the duplicate live implementation preserves known backend drift.
Whole-root replacement breaks object identity and native human undo. Generic
transactions/rollback journals add unnecessary state; sequential partial results
and native undo are enough. The selected serializer cannot mark native history
clean, which is an accepted visible limitation, not a missing framework to build.

### Review disposition

One standard Astra design pass completed. An entry that changes a live scene gets
one native undo action even when it ultimately returns a partial failure. Preserve
its effects and persistence through single-call errors and both batch modes.
Undo callbacks must retain their actual node/resource arguments, not depend on an
expired SceneEdit context. Restore relevant ownership and sibling placement as
well as object identity. Do not reuse the old recursively owner-rewriting save
helper on a borrowed editor root: explicit save must not mutate human state.

Nathan subsequently approved Godot 4.7 as the minimum. The earlier 4.5 restriction
is superseded: native get_unsaved_scenes may be used. Selected-scene-only save,
retained undo and truthful dirty-marker reporting remain unchanged.

## Implementation evidence

Shared `addons/director/ops/dispatcher.gd` now serves editor, daemon, one-shot and
mock transports, preserving identity ping/status and engine_api dispatch.
`scene_edit.gd` owns detached-root lifetime, selected serialization and native
inverse arguments. Operation families resolve through the same context; duplicate
editor mutators and open-scene disk reload synchronization are removed. Inactive
named tabs are activated for history selection and the prior named tab restored.
Added/removed objects, subtree ownership, sibling placement, script exports,
group persistence, metadata presence, signal callables/binds/flags and affected
cells are retained through native undo. Property/cell collections prevalidate;
native float precision and resource-class alternatives are respected. Path aliases
cannot bypass open-scene destination protection or root/same-scene validation.

Rust exposes scene_save through MCP and CLI and a shared Persistence type on every
mutating response. Batch entries retain operation, success, data, error, context
and persistence; both stop and continue failures preserve their partial results in
MCP error data. Standalone animation mutation avoids poisoning cached editor
resources. Selected-scene save checks pack/save errors, retains undo, does not
rewrite borrowed owners, and reports the accepted dirty-marker limitation.

Verification performed on Godot 4.7.1:

- `cargo test -p director-tests -- --ignored --test-threads=1`: 200 existing
  Director engine tests passed at the broad regression checkpoint. Later target
  validation changes were retested through node (6), reparent (5), wiring (8)
  and the expanded real-editor journeys.
- `cargo test -p director --tests`: all ordinary Director tests passed.
- `cargo test -p director --test authoring_editor -- --ignored --test-threads=1`:
  both final graphical editor journeys passed. They use the actual Director
  plugin and real editor keyboard undo/redo, two dirty tabs, preserved human
  history/object identity, all existing scene-mutator families through single
  and batch calls, subtree/instance ownership, exact tile/grid state, bound
  signals, exports, partial setter errors in both batch modes, closed-scene
  partial persistence, selected save via MCP/CLI, unsaved referenced external
  material, save/reopen/run, unowned descendants and checked save failure.
- `cargo test -p director --test editor_identity_engine -- --ignored --test-threads=1`:
  existing real-editor identity journey passed after dispatcher integration.
- `cargo clippy -p director --all-targets -- -D warnings`,
  `cargo fmt -p director -- --check` and owned-path `git diff --check` passed.

The graphical fixture now leaves Godot accessibility at its default and still
uses real native editor shortcuts, not raw UndoRedo calls. Two separate Godot
4.7.1 probes were previously reported passing without an override; no universal
accessibility or other-platform safety is claimed. In the bounded parent
integration rerun, `cargo test -p director --test authoring_editor -- --ignored
--nocapture --test-threads=1` reported 1 passed and 2 failed: both native-undo
journeys missed their first Ctrl+Z. A focused rerun reproduced the history
journey failure; one later isolated retry passed after a startup delay, but the
delay did not stabilize the full suite and was removed. Another full run exited
one editor after AccessKit panicked on a duplicate child. This current failure
remains visible rather than restoring the unconditional accessibility override.
Fixture projects/processes and temporary
authoring helpers were cleaned; the shared Cargo target was reused. Parent
retains integrated implementation review, foundation/reference reconciliation,
final stable-workspace verification and closure. No commit or second design
review was performed.

### Review corrections and default-editor evidence

The one standard implementation review found an instance-name validation
inconsistency and a material default-accessibility evidence gap. Supplied names
are now checked before Godot can normalize them; all four focused instance tests
passed. A separate bounded probe removed only the accessibility override from
the graphical fixture's launch. Both authoring journeys passed on Godot 4.7.1
(3.51 s and 8.43 s), without AccessKit panic or crash. The earlier failure did not
reproduce, so no engine-only attribution or universal safety claim is made.
Integration is removing the unconditional override and rerunning the combined
fixture with the shared feedback payload. No repeat formal review is needed.

### Native-shortcut integration correction

The combined fixture reproduced both first-undo assertion failures with fresh
XDG config/data/cache and default accessibility. Diagnostic input tracing showed
that the fixture window had focus, Ctrl+Z matched the enabled native Undo menu,
and the selected scene history contained the Director action. The key reached
window input but not shortcut handling: initial filesystem import was still in
progress, and EditorNode's progress handler was consuming keys. No feedback
button matched the event. The source's `EditorNode::input` explicitly implements
that progress-time interception.

The fixture now polls actual filesystem scan and editor input-interception state
before preparing scenes and before shortcuts; scan completion alone briefly
precedes removal of the input blocker. The driver rechecks readiness after tab
activation. This replaces timing guesses with a bounded condition wait, not a
startup sleep. It keeps real Ctrl+Z/Ctrl+Shift+Z and native history, uses separate
press/release event objects, and isolates editor/runtime/saved-scene-run XDG
config/data/cache in owned temporary storage. No production addon change, global
key injection, OS window-focus manipulation, accessibility override, or unrelated
process termination was needed.

A focused diagnostic history journey passed (152.26 s including cold startup).
The cleaned three-journey run (`cargo test -p director --test authoring_editor
-- --ignored --nocapture --test-threads=1`, log
`/tmp/theatre-authoring-final-suite.log`) passed history and run control, then
failed the mutator journey with a distinct AccessKit duplicate-child panic
(590.91 s total). The original missed-shortcut assertions did not recur. A focused
mutator run with per-operation diagnostics reproduced that panic after the
`node_set_properties` batch undo (273.00 s, log
`/tmp/theatre-authoring-mutator-accesskit.log`). Giving the native 2D viewport
control focus instead of releasing control focus passed the entire mutator
journey (955.75 s, log `/tmp/theatre-authoring-mutator-viewport-focus.log`). This
contrast does not prove the AccessKit internals or establish universal safety.
The final driver verifies that native control focus synchronously, without an
extra experimental frame wait, and still delivers real keyboard events.

All three requested journeys passed on that checked-focus fixture (1097.18 s):
`cargo test -p director --test authoring_editor -- --ignored --nocapture
--test-threads=1`, log `/tmp/theatre-authoring-native-focus-full-suite.log`.
Default accessibility remains unchanged. This is passing integration evidence,
not proof of the AccessKit crash's internal cause. Director clippy with all
targets and warnings denied, formatting, and owned-path whitespace checks passed
on that fixture. Parent
retains full-workspace acceptance, commits and closure; no new formal review.

The requested Director-free control now mirrors the add/instance/remove/reparent/
property actions leading to the reproduced crash. It disables the Director plugin,
builds actions directly with EditorUndoRedoManager, retains the original
no-control-focus keyboard path, and checks undo/redo identity and position. This
comparison passed on 4.7.1 (255.39 s): `cargo test -p director --test
authoring_editor native_only_undo_comparison -- --ignored --nocapture
--test-threads=1`, log `/tmp/theatre-authoring-native-only-4.7.1.log`.
Accessibility was active. Startup/input readiness took 69.58 s; the subsequent
native cycles took 182.64 s with repeated roughly one-second requests even when
readiness reported no blocker. The crash is therefore not established as a
general native undo failure. The identical control also passed on isolated
4.7.2 (255.58 s), log `/tmp/theatre-authoring-native-only-4.7.2.log`. Both controls
used released control focus and active accessibility. The 4.7.2 diagnostics
reported 1 FPS, window focus true, and the normal 6,900 µs low-processor sleep;
one-poll readiness requests therefore consumed about one second each. The long
runs are not repeated 90-second readiness timeouts.

The passing three-journey run used its earlier copied fixture, before this
diagnostic control was added. Startup, readiness waits and mutator
phase elapsed times are now labeled separately to account for long graphical
runs instead of extending unexplained sleeps.

The official GitHub release API confirms `4.7.2-stable`, published 2026-08-18,
`prerelease: false`. Its Linux archive was downloaded into an isolated temporary
directory and checked against the published SHA-256
`cadd3204e728a35d3f13adb7fd0d7902636b79f6b95c40c265eb73b6c35329e4`.
The isolated executable reports `4.7.2.stable.official.ed1daf0bf`; the user install
was not replaced. Its curated changelog includes the AccessKit 0.22.3 update
(Godot PR 121393), but that PR describes upcoming Orca compatibility, not this
specific duplicate-child failure. Runtime comparison is still needed; no patch
fix or baseline recommendation is established.

The remaining patch comparison is running all three requested Director journeys
on that isolated 4.7.2 executable, explicitly restoring the original released-
control-focus shortcut context through a test-driver-only environment switch.
Command: `cargo test -p director --test authoring_editor -- --ignored --nocapture
--test-threads=1 --skip native_only_undo_comparison`; environment:
`GODOT_BIN=/tmp/theatre-authoring-godot-4.7.2.dfoaV4/Godot_v4.7.2-stable_linux.x86_64`
and `THEATRE_AUTHORING_RELEASE_CONTROL_FOCUS=1`; log:
`/tmp/theatre-authoring-release-focus-4.7.2.log`. The skipped comparison was already
run separately on both stable versions; none of the three authoring journeys is
omitted. No production UI change or accessibility override is involved.

Nathan subsequently stopped the investigation and explicitly deferred it to
`.work/backlog/godot-editor-accesskit-undo-crash.md` so rollout can proceed.
The final 4.7.2 Director comparison was interrupted without a final result. The
installed engine remains 4.7.1. Passing controls and the checked-focus journey
remain bounded evidence; no patch-fix or exclusively upstream claim is made.
