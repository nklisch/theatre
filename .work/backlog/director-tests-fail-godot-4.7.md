---
id: director-tests-fail-godot-4.7
tags: [director, tests, environment]
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
