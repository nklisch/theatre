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
an exclusively upstream cause. The final Director/4.7.2 comparison was interrupted
when Nathan stopped the investigation; no patch-fix conclusion is established.
Diagnostics reported roughly 1 FPS despite focused windows and normal configured
low-processor sleep, so long runs were not repeated readiness timeouts.

Related work: `.work/active/director-consistent-authoring.md` and
`.work/active/agent-godot-development-loop.md`. Preserve the distinction between
normal native undo coverage and diagnostic reproductions; do not disable
accessibility or repeat until green and call that a fix.
