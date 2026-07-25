---
id: director-tests-fail-godot-4.7
kind: story
tags: [director, tests, environment]
parent: null
release: null
completed: 2026-07-25
---

# Director E2E tests fail on this machine (Godot 4.7.1)

Fixed five root causes: stale cargo fingerprints across worktrees sharing
CARGO_TARGET_DIR, GODOT_BIN/GODOT_PATH resolution mismatch (now GODOT_BIN →
GODOT_PATH → PATH), Godot 4.7 engine behavior changes (connect() validates
bound callables — op now checks the return and the test attaches a real
script; duplicate VisualShader node_ids rejected; signal_list emits binds),
a stale stdout/stderr CLI assertion, and env-var test races (mutex locks).
Also: workspace target dir now resolved via cargo metadata everywhere (no
target/ assumption), and the E2E harness bootstraps
.godot/extension_list.cfg on fresh checkouts. Verified: director-tests
193/193, live-tests 12/12, stage-server e2e green.
