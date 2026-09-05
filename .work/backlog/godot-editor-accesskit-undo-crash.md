---
id: godot-editor-accesskit-undo-crash
tags: [godot, editor, verification]
created: 2026-09-05
updated: 2026-09-05
---

# Investigate intermittent native editor accessibility crash

Nathan explicitly deferred this investigation so it does not block local Theatre
rollout or preparing Voxlar for another run. This is not a claim that the crash
is fixed or that all native verification passed.

During the development-loop authoring tests, Godot 4.7.1 crashed inside
accesskit_consumer 0.35.0 with `TreeUpdate includes duplicate child` during native
undo. Separately, import progress was swallowing shortcut input; the fixture now
waits for scan completion and keyboard blockers, and isolates XDG directories.
That input-readiness correction does not establish an accessibility crash fix.

A subsequent controlled run of all three authoring journeys passed in 1097.18s
with native shortcuts and default accessibility. The earlier failing run passed
two journeys and crashed in the all-mutators journey after 590.91s. Neither a
later pass nor the engine stack alone establishes an exclusively upstream cause.

A Director-free native comparison was prepared in
`crates/director/tests/authoring_editor.rs` and its driver. Godot 4.7.2 was downloaded
and checksum-verified in isolation, without changing the installed engine. The
Director-free native control passed on both 4.7.1 (255.39 s) and 4.7.2 (255.58 s),
with accessibility active and released control focus. This does not establish
an exclusively upstream cause. The final Director/4.7.2 comparison's late-arriving
log recorded three passes (1084.60 s). This does not establish a patch fix or
reopen the investigation Nathan explicitly deferred.
Diagnostics reported roughly 1 FPS despite focused windows and normal configured
low-processor sleep, so long runs were not repeated readiness timeouts.

Related work: `.work/active/director-consistent-authoring.md` and
`.work/active/agent-godot-development-loop.md`. Preserve the distinction between
normal native undo coverage and diagnostic reproductions; do not disable
accessibility or repeat until green and call that a fix.

## Latest baseline reproduction

The September repair baseline ran `cargo test --workspace -- --ignored --test-threads=1` after successful deployment. On Godot 4.7.1, the all-mutators authoring journey again crashed during native undo/redo around `node_set_meta`, with `accesskit_consumer-0.35.0/src/tree.rs:224:21: TreeUpdate includes duplicate child #491778050362126`. Director observed `input_readiness: editor plugin TCP I/O error: Connection reset by peer (os error 104)`. Three other authoring tests passed, including the native-only comparison. The binary replacement repair had passed deployment before this run. This confirms the baseline verification limitation, not its cause. The investigation remains explicitly excluded from the eleven-item repair boundary. Do not disable accessibility or repeat until green as a purported fix.

A filtered continuation excluding the first failing all-mutators test reproduced the same native crash in `authoring_preserves_human_history_and_selected_scene_persistence`: `TreeUpdate includes duplicate child #479932530557273`, followed by Director `inspect` connection reset. This shows the baseline limitation is not confined to one test name. Remaining coverage excludes the deferred native undo journeys rather than retrying them to obtain a pass. The full required suite remains failed.

A later complete ignored-suite invocation on the same Godot 4.7.1 environment
passed, including the authoring-editor tests. The earlier duplicate-child panic
remains reproduced evidence of an intermittent issue; no accessibility fix or
workaround was applied during the eleven-item repair delivery. A passing run
does not establish that this deferred native crash has been repaired.

The responsive-capture full rerun reproduced the crash in
`every_scene_mutator_uses_native_undo_in_single_and_batch_routes`, during undo
for batched `node_set_meta`. Godot 4.7.1 Compatibility on the RTX 4070 reported
`TreeUpdate includes duplicate child #491670676179720` at the same AccessKit
0.35.0 `tree.rs:224:21` location; Director reported
`shortcut: editor plugin TCP I/O error: early eof`. Three other authoring tests
passed. This remains excluded engine-crash evidence, not a capture regression
or a repaired accessibility issue. A separate movement-fixture startup race
also failed that invocation and is being corrected within verification.
