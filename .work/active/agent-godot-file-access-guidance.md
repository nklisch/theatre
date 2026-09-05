---
id: agent-godot-file-access-guidance
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

# Practical Godot file-access guidance

## Accepted outcome

Allow reads and diffs, keep scripts and shaders code-first, prefer Godot-backed structural mutations. Reconcile distributed rules and tool guidance; no arbitrary text-write automation.

## Design

- Generated project rules permit reading and diffing Godot project files.
- GDScript and shader source remain normal code-first files.
- Structural scene, resource, and project-setting mutations prefer Director or another
  Godot-backed path; guidance does not invent an automatic text-edit fallback.
- Director descriptions explain the native-serialization benefit without blanket claims
  that direct inspection or editing necessarily corrupts files.
- Guidance reflects delivered editor behavior: open-target changes use the live root and
  native undo, remain unsaved until explicit `scene_save`, and selected saving does not
  imply unrelated external resources were persisted. Verification distinguishes current
  editor state from saved content.

## Progress

Implemented the owned rules template, CLI distribution checks, Director instruction
strings, Director agent guidance, and the rules-generation integration assertions. Guidance
now states the delivered boundary: open-target changes use the live root and native undo,
remain unsaved until `scene_save`, and detached headless operations persist their files.

Reviewer correction applied without a second review cycle: append targets now detect the
legacy generated `Never hand-edit Godot files` heading before the current marker, preserve
the entire user-owned file, and emit an actionable reconcile-and-rerun warning instead of
appending contradictory guidance. Direct append regressions cover both successful append
with user content preserved and refusal for legacy sections in `CLAUDE.md` and `AGENTS.md`.
The misleading `--yes` integration test is renamed to describe the separate-rules-file path
it actually exercises.

Focused verification passes: `CARGO_TARGET_DIR=/storage/cargo-target cargo test -p
theatre-cli rules` reports 4 CLI guidance unit tests and 4 matching CLI rules integration
tests passed after their assertions were reconciled to the current native-undo/save boundary;
29 Director library tests passed on the stable pre-review target. Workspace formatting and
scoped diff checks pass. The bounded working-tree diff across the implementation/guidance
files is stable after the accepted correction.

The parent epic owns generated Director references, foundation reconciliation, integrated
review, and closure.

## Closure evidence

Meaningful stable-interface tests and affected real-engine journeys pass. Reconcile
affected durable truth and generated references. Complete one standard implementation
review; parent supplies combined workspace verification before closure.

## Assessment context

The accepted outcome above governs scope; the following preserves the source evidence.

# Make Godot file-access guidance useful rather than prohibitive

Parked from the Godot architecture assessment at Nathan's request.

`rules-template.md` forbids both reading and editing `.tscn`, `.tres`, `.cfg`,
`.import`, and `project.godot`. Reading or diffing those files cannot corrupt them
and can expose authored resource structure omitted by tool summaries.
`crates/director/src/mcp/mod.rs` also describes hand-editing as necessarily
producing corrupt scenes. `crates/theatre-cli/src/rules.rs` distributes the rule.

Reconsider the blanket rule: always allow reads and diffs, prefer Godot-backed
operations for structural scene/resource mutations, keep scripts and shaders
code-first, and evaluate a carefully validated text-edit fallback. This is a
proposed guidance change, not a permission change already adopted by projects.

Keep Godot's serialization benefits without forcing normal source-code work
through Theatre. Review distributed rules, tool descriptions, and skills together
so agents receive one consistent operating recommendation.
