---
id: director-tests-fail-godot-4.7
kind: story
status: active
tags: [director, tests, environment]
parent: null
blocked_by: []
related_to: []
research_refs: []
mock_refs: []
created: 2026-07-25
updated: 2026-07-25
---

# Director E2E tests fail on this machine (Godot 4.7.1)

Found during visual-storyboards verification (2026-07-25). 12 director-tests
fail with GODOT_BIN=~/godot/Godot_v4.7.1-stable_linux.x86_64:
test_cli (4), test_cli_journey (6), test_gaps (2). Reproduced on the
pre-feature baseline 209d1d2, so unrelated to the visual-storyboards work —
likely an environment or Godot-version drift issue (project targets 4.6 per
tests/godot-project/project.godot features).

Repro: `GODOT_BIN=~/godot/Godot_v4.7.1-stable_linux.x86_64 cargo test -p director-tests -- --include-ignored`
Example: `test_cli::cli_rejects_invalid_json` — "stderr should mention
invalid JSON:" with empty stderr.

Also two environment papercuts found the same day:
- theatre-cli deploy and several test harnesses hardcode `target/` and break
  when CARGO_TARGET_DIR is redirected (workaround: `ln -s $CARGO_TARGET_DIR target`).
- Godot headless runs need `.godot/extension_list.cfg`; a fresh checkout
  requires one `--headless --editor --quit` pass before E2E journeys can load
  the GDExtension.

## Scope (2026-07-25)

Reproduce the 12 failures, diagnose root cause (environment vs code), fix the
smallest coherent boundary so the full director test layer passes on this
machine with Godot 4.7.1 — or, if genuinely environmental, document the exact
remediation. Includes the target/-dir hardcoding and extension_list bootstrap
papercuts where they block the suite.

## Diagnosis & fixes (2026-07-25)

Five root causes, all fixed in a258f91:

1. **Stale cargo fingerprints across worktrees** — a test binary built in a
   throwaway worktree (/tmp/theatre-base) was reused by the main checkout
   (shared CARGO_TARGET_DIR), baking a deleted CARGO_MANIFEST_DIR into
   env!() paths. Cleared via cargo clean; noted as environment hazard.
2. **GODOT_BIN vs GODOT_PATH mismatch** — director resolved Godot only via
   GODOT_PATH/PATH; harnesses set GODOT_BIN. resolve.rs now tries
   GODOT_BIN → GODOT_PATH → which godot.
3. **Godot 4.7 engine behavior changes** — (a) connect() now validates bound
   callables at connect time (ERR 31 if method missing); signal_ops ignored
   the return code and the test targeted a nonexistent method. Op now checks
   the return; test attaches fixtures/signal_target.gd. (b) VisualShader
   add_node with duplicate id now errors + no-ops; shader_ops rejects
   duplicates per shader function. (c) op_signal_list never emitted binds;
   added.
4. **Stale test assertion** — CLI emits structured JSON errors on stdout (by
   design, b4f82a6); test asserted stderr. Fixed to stdout.
5. **Env-var test races** — editor.rs/daemon.rs port tests raced under
   parallel threads; serialized behind per-module mutexes.

Papercuts fixed: workspace target dir now resolved via CARGO_TARGET_DIR env
→ cargo metadata → target/ fallback in theatre-cli paths.rs and all test
harnesses (suite passes with no target symlink — verified); E2E harness
bootstraps .godot/extension_list.cfg on fresh checkouts.

Verified: director-tests 193/193, live-tests 12/12, stage-server e2e 7/7,
workspace non-ignored green. Note: parallel suite execution (multiple Godot
instances) causes timing flakes in live-tests physics journeys — run suites
sequentially.

Pending: cross-model review pass before closing.
