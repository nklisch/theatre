---
id: stage-snapshot-string-name-variants
kind: story
status: completed
completed: 2026-09-15
---
# Keep StringName state from crashing Stage snapshots

Stage snapshots and inspections now serialize exported `StringName` values and
textual dictionary keys through their matching Godot Variant types instead of
panicking during a `GString` conversion. The real-engine exported-state journey
reproduced the timeout before correction and passes afterward for empty, populated,
and nested `StringName` data across summary, standard, full, and inspect queries.

The affected Rust packages pass strict Clippy, formatting is clean, and a bounded
inline review found no remaining issue in the changed conversion boundary.
