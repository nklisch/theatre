# E2E Test Suite Design: Theatre CLI

## Project Summary

**Binary under test:** `theatre` (crate: `theatre-cli`)
**Target users:** Human game developers (setup/config) and AI agents (programmatic invocation)
**Commands:** `install`, `init`, `deploy`, `enable`, `rules`, `mcp`

## Test Environment

**Framework:** Rust built-in `#[test]` + `std::process::Command` (subprocess invocation)
**Fixtures:** `tempfile::TempDir` for isolated project directories
**Binary access:** `env!("CARGO_BIN_EXE_theatre")` (built by `cargo test`)
**Run command:** `cargo test -p theatre-cli`

### Shared Test Helpers Needed

```rust
/// Create a minimal Godot project in a temp directory
fn make_project(dir: &Path) {
    fs::write(dir.join("project.godot"), "[editor_plugins]\nenabled=PackedStringArray()\n").unwrap();
}

/// Create a fake "installed" share directory with addon stubs
fn make_share_dir(dir: &Path) {
    let stage = dir.join("addons/stage");
    fs::create_dir_all(stage.join("bin/linux")).unwrap();
    fs::write(stage.join("plugin.cfg"), "[plugin]\nname=\"Stage\"\n").unwrap();
    fs::write(stage.join("runtime.gd"), "extends Node\n").unwrap();
    fs::write(stage.join("bin/linux/libstage_godot.so"), b"fake").unwrap();
    let director = dir.join("addons/director");
    fs::create_dir_all(&director).unwrap();
    fs::write(director.join("plugin.cfg"), "[plugin]\nname=\"Director\"\n").unwrap();
}

/// Run theatre command with isolated env (custom THEATRE_SHARE_DIR, THEATRE_BIN_DIR)
fn theatre(share: &Path, bin: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_theatre"));
    cmd.env("THEATRE_SHARE_DIR", share);
    cmd.env("THEATRE_BIN_DIR", bin);
    cmd.env("THEATRE_NO_TELEMETRY", "1");
    cmd
}
```

## Existing Coverage

5 tests in `cli_integration.rs`:
- `help_prints_subcommands` — --help lists all commands
- `version_prints_version` — --version prints version
- `enable_on_valid_project` — enable adds plugins to project.godot
- `enable_on_missing_project` — enable fails on bad path
- `init_fails_without_install` — init --yes fails when share dir missing

**Gaps:** No tests for `install` (requires cargo build), `deploy` (requires cargo build),
`rules`, `mcp`, `init` happy path, `enable --disable`, `enable --stage`/`--director`,
idempotency, error messages, graceful degradation, or multi-step workflows.

---

## Golden-Path Tests

### Journey: Init a fresh project (non-interactive)

**Priority:** High — this is the first thing every user does

#### Test: init_yes_copies_addons_and_generates_config
- **Setup:** Fake share dir with addon stubs; empty Godot project (project.godot only)
- **Steps:** `theatre init <project> --yes`
- **Assertions:**
  - Exit code 0
  - `<project>/addons/stage/plugin.cfg` exists
  - `<project>/addons/stage/runtime.gd` exists
  - `<project>/addons/stage/bin/linux/libstage_godot.so` exists (GDExtension copied)
  - `<project>/addons/director/plugin.cfg` exists
  - `<project>/.mcp.json` exists and is valid JSON
  - `.mcp.json` contains `"stage"` and `"director"` server entries
  - `.mcp.json` stage entry has `"args": ["serve"]`
  - `project.godot` contains `stage/plugin.cfg` in enabled plugins
  - `project.godot` contains `director/plugin.cfg` in enabled plugins
  - `project.godot` contains `StageRuntime` autoload entry
- **Teardown:** TempDir auto-cleanup

#### Test: init_yes_uses_default_port_9077
- **Setup:** Fake share dir; empty Godot project
- **Steps:** `theatre init <project> --yes`
- **Assertions:**
  - `.mcp.json` does NOT contain `THEATRE_PORT` env key (default port omitted)
- **Teardown:** TempDir auto-cleanup

---

### Journey: Enable/disable plugins

**Priority:** High — used after init or manual addon copy

#### Test: enable_stage_only
- **Setup:** Godot project with `project.godot`
- **Steps:** `theatre enable <project> --stage`
- **Assertions:**
  - Exit code 0
  - `project.godot` contains `stage/plugin.cfg`
  - `project.godot` does NOT contain `director/plugin.cfg`
  - `project.godot` contains `StageRuntime` autoload
- **Teardown:** TempDir auto-cleanup

#### Test: enable_director_only
- **Setup:** Godot project with `project.godot`
- **Steps:** `theatre enable <project> --director`
- **Assertions:**
  - Exit code 0
  - `project.godot` contains `director/plugin.cfg`
  - `project.godot` does NOT contain `stage/plugin.cfg`
  - `project.godot` does NOT contain `StageRuntime` autoload
- **Teardown:** TempDir auto-cleanup

#### Test: disable_plugins
- **Setup:** Godot project with both plugins already enabled
- **Steps:** `theatre enable <project> --disable`
- **Assertions:**
  - Exit code 0
  - `project.godot` does NOT contain `stage/plugin.cfg`
  - `project.godot` does NOT contain `director/plugin.cfg`
  - `project.godot` does NOT contain `StageRuntime` autoload
- **Teardown:** TempDir auto-cleanup

#### Test: enable_is_idempotent
- **Setup:** Godot project with `project.godot`
- **Steps:** Run `theatre enable <project>` twice
- **Assertions:**
  - Both exit code 0
  - `project.godot` contains exactly ONE `stage/plugin.cfg` entry (no duplicates)
  - `project.godot` contains exactly ONE `director/plugin.cfg` entry
  - `project.godot` contains exactly ONE `StageRuntime` autoload
- **Teardown:** TempDir auto-cleanup

---

### Journey: Generate rules file

**Priority:** Medium — optional step during setup

#### Test: rules_creates_claude_rules_file
- **Setup:** Godot project; no `.claude/` directory
- **Steps:** `theatre rules <project> --yes`
- **Assertions:**
  - Exit code 0
  - `<project>/.claude/rules/godot.md` exists
  - File contains "Never hand-edit Godot files"
  - File contains "Director" and "Stage" tool references
- **Teardown:** TempDir auto-cleanup

#### Test: rules_skips_if_already_present
- **Setup:** Godot project with existing `.claude/rules/godot.md`
- **Steps:** `theatre rules <project> --yes`
- **Assertions:**
  - Exit code 0
  - Stderr contains "already" or "skip" (warning message)
  - File content unchanged
- **Teardown:** TempDir auto-cleanup

---

### Journey: Generate .mcp.json

**Priority:** Medium — used to regenerate config after changes

#### Test: mcp_generates_valid_config
- **Setup:** Fake share dir with both addons; Godot project with addons/ copied
- **Steps:** `theatre mcp <project> --yes`
- **Assertions:**
  - Exit code 0
  - `.mcp.json` is valid JSON
  - Contains `mcpServers.stage` with `command` pointing to stage binary
  - Contains `mcpServers.director` with `command` pointing to director binary
  - Both servers have `"type": "stdio"` and `"args": ["serve"]`
- **Teardown:** TempDir auto-cleanup

#### Test: mcp_custom_port
- **Setup:** Fake share dir; Godot project with addons/
- **Steps:** `theatre mcp <project> --yes --port 8080`
- **Assertions:**
  - Exit code 0
  - `.mcp.json` stage entry contains `"env": {"THEATRE_PORT": "8080"}`
- **Teardown:** TempDir auto-cleanup

#### Test: mcp_stage_only_when_director_missing
- **Setup:** Fake share dir; Godot project with only `addons/stage/` (no director)
- **Steps:** `theatre mcp <project> --yes`
- **Assertions:**
  - Exit code 0
  - `.mcp.json` contains `mcpServers.stage`
  - `.mcp.json` does NOT contain `mcpServers.director`
- **Teardown:** TempDir auto-cleanup

---

### Journey: Full lifecycle (init → enable → rules → mcp)

**Priority:** High — validates commands compose correctly

#### Test: full_lifecycle_init_then_disable_then_reenable
- **Setup:** Fake share dir; empty Godot project
- **Steps:**
  1. `theatre init <project> --yes`
  2. Verify both plugins enabled
  3. `theatre enable <project> --disable --stage`
  4. Verify Stage disabled, Director still enabled
  5. `theatre enable <project> --stage`
  6. Verify both plugins enabled again
- **Assertions:**
  - All commands exit 0
  - project.godot reflects correct state after each step
  - .mcp.json survives enable/disable cycles unchanged
- **Teardown:** TempDir auto-cleanup

#### Test: init_then_mcp_regenerate_with_different_port
- **Setup:** Fake share dir; empty Godot project
- **Steps:**
  1. `theatre init <project> --yes` (default port)
  2. `theatre mcp <project> --yes --port 8080`
- **Assertions:**
  - Both exit 0
  - `.mcp.json` now has port 8080 (overwritten)
  - Addons still intact (mcp doesn't touch addons/)
- **Teardown:** TempDir auto-cleanup

---

### Journey: Deploy to project (no-build path)

**Priority:** Medium — tests the deploy-from-share-dir path (skipping cargo build)

#### Test: deploy_from_share_dir_copies_addons
- **Setup:** Fake share dir with addon stubs; Godot project with project.godot only; set `THEATRE_ROOT` to nonexistent path (forces share-dir-only mode)
- **Steps:** `theatre deploy <project>`
- **Assertions:**
  - Exit code 0
  - `<project>/addons/stage/plugin.cfg` exists
  - `<project>/addons/stage/bin/linux/libstage_godot.so` exists
  - `<project>/addons/director/plugin.cfg` exists
- **Teardown:** TempDir auto-cleanup

#### Test: deploy_skips_symlinked_addons
- **Setup:** Godot project with `addons/stage` as a symlink to another directory
- **Steps:** `theatre deploy <project>` (with share dir set)
- **Assertions:**
  - Exit code 0
  - Symlink still intact (not replaced with directory)
  - Stderr contains warning about symlink
- **Teardown:** TempDir auto-cleanup

#### Test: deploy_multiple_projects
- **Setup:** Fake share dir; two Godot projects
- **Steps:** `theatre deploy <project1> <project2>`
- **Assertions:**
  - Exit code 0
  - Both projects have addons/ populated
- **Teardown:** TempDir auto-cleanup

---

## Adversarial / Failure-Mode Tests

### Category: User Mistakes

#### Test: init_without_project_godot
- **Scenario:** User points to a directory that isn't a Godot project
- **Action:** `theatre init /tmp/empty-dir --yes`
- **Expected:** Exit code 1; stderr contains "project.godot" (tells user what's missing)
- **Verify:** No files created in target directory

#### Test: init_nonexistent_path
- **Scenario:** User provides a path that doesn't exist
- **Action:** `theatre init /tmp/does-not-exist-12345 --yes`
- **Expected:** Exit code 1; stderr contains error about path not found
- **Verify:** No side effects

#### Test: enable_nonexistent_project
- **Scenario:** User provides invalid project path
- **Action:** `theatre enable /tmp/does-not-exist-12345`
- **Expected:** Exit code 1; clear error message
- **Verify:** No side effects

#### Test: mcp_invalid_port_zero
- **Scenario:** User provides port 0
- **Action:** `theatre mcp <project> --yes --port 0`
- **Expected:** Exit code non-zero OR port validation error (port must be >= 1024)
- **Verify:** No .mcp.json written

#### Test: mcp_invalid_port_privileged
- **Scenario:** User provides privileged port (< 1024)
- **Action:** `theatre mcp <project> --yes --port 80`
- **Expected:** Exit code non-zero; error about port range
- **Verify:** No .mcp.json written

#### Test: rules_nonexistent_project
- **Scenario:** User provides invalid project path
- **Action:** `theatre rules /tmp/does-not-exist-12345 --yes`
- **Expected:** Exit code 1; clear error message
- **Verify:** No files created

#### Test: deploy_no_share_dir_no_source
- **Scenario:** Theatre not installed and no source repo available
- **Action:** `theatre deploy <project>` with `THEATRE_SHARE_DIR=/tmp/nonexistent` and `THEATRE_ROOT=/tmp/nonexistent`
- **Expected:** Exit code 1; stderr mentions "install" (tells user to run `theatre install` first)
- **Verify:** Project unchanged

---

### Category: Bad Environment

#### Test: init_share_dir_missing
- **Scenario:** Theatre was never installed (share dir doesn't exist)
- **Action:** `theatre init <project> --yes` with `THEATRE_SHARE_DIR=/tmp/nonexistent-share`
- **Expected:** Exit code 1; stderr tells user to run `theatre install`
- **Verify:** Project directory unchanged (no partial addon copies)

#### Test: init_share_dir_incomplete
- **Scenario:** Share dir exists but is missing GDExtension binary
- **Action:** `theatre init <project> --yes` with share dir that has addon dirs but no .so file
- **Expected:** Either succeeds with warning about missing binary, or fails with clear message
- **Verify:** If partial success: addons copied, GDScript present; GDExtension missing is clearly communicated

#### Test: enable_readonly_project_godot
- **Scenario:** project.godot exists but is read-only
- **Action:** `theatre enable <project>`
- **Expected:** Exit code 1; error about write permission
- **Verify:** project.godot unchanged

#### Test: deploy_readonly_project_dir
- **Scenario:** Project addons/ directory is read-only
- **Action:** `theatre deploy <project>` with read-only addons/ dir
- **Expected:** Exit code 1; error about write permission
- **Verify:** No partial writes

---

### Category: Boundary Conditions

#### Test: enable_empty_project_godot
- **Scenario:** project.godot is an empty file (no sections at all)
- **Action:** `theatre enable <project>`
- **Expected:** Exit code 0; project.godot now contains `[editor_plugins]` section with plugins
- **Verify:** File is well-formed after modification

#### Test: enable_project_godot_no_editor_plugins_section
- **Scenario:** project.godot has content but no `[editor_plugins]` section
- **Action:** `theatre enable <project>`
- **Expected:** Exit code 0; `[editor_plugins]` section created and plugins added
- **Verify:** Existing content preserved; new section appended

#### Test: enable_project_godot_with_existing_plugins
- **Scenario:** project.godot already has other plugins in `[editor_plugins]`
- **Action:** `theatre enable <project>`
- **Expected:** Exit code 0; Theatre plugins added alongside existing ones
- **Verify:** Existing plugins not removed or reordered

#### Test: init_project_with_existing_addons
- **Scenario:** Project already has addons/stage/ and addons/director/ from prior install
- **Action:** `theatre init <project> --yes`
- **Expected:** Exit code 0; addons overwritten with fresh copies
- **Verify:** New addon files present; old files replaced

#### Test: mcp_overwrites_existing_mcp_json
- **Scenario:** Project has an existing .mcp.json with custom content
- **Action:** `theatre mcp <project> --yes`
- **Expected:** Exit code 0; .mcp.json overwritten with generated content
- **Verify:** Old content replaced; new content is valid JSON

#### Test: rules_append_to_existing_claude_md
- **Scenario:** Project has existing CLAUDE.md with user content
- **Action:** `theatre rules <project>` (selecting CLAUDE.md target, or test the append logic directly)
- **Expected:** Rules appended to CLAUDE.md; original content preserved
- **Verify:** Original content intact at top; rules at bottom; no duplicates

#### Test: help_and_version_always_work
- **Scenario:** No project, no share dir, nothing installed
- **Action:** `theatre --help`, `theatre --version`, `theatre init --help`
- **Expected:** All exit 0; help shows all subcommands; version shows correct version
- **Verify:** No filesystem side effects

---

### Category: Graceful Degradation

#### Test: init_continues_after_optional_failure
- **Scenario:** Share dir has addons but rules template write fails (e.g., .claude/ dir not writable)
- **Action:** `theatre init <project> --yes` where addons copy succeeds but rules can't be written
- **Expected:** Addons copied and plugins enabled (hard dependencies succeed); warning about rules failure
- **Verify:** Addons present, plugins enabled in project.godot, .mcp.json generated — only rules missing

#### Test: deploy_continues_for_second_project_if_first_fails
- **Scenario:** Two projects passed to deploy; first one is invalid, second is valid
- **Action:** `theatre deploy /tmp/bad-project <good-project>`
- **Expected:** Error for first project; second project still deployed (or: fail fast for all?)
- **Verify:** Document actual behavior — this test discovers the current contract

---

## Implementation Notes

### Test File Location

Expand the existing `crates/theatre-cli/tests/cli_integration.rs` file. The 5 existing tests
already use the right pattern (`Command` + `tempfile`). New tests follow the same pattern.

### Shared Fixtures

Add a `support` module or helper functions at the top of `cli_integration.rs`:
- `make_project(dir)` — writes minimal project.godot
- `make_share_dir(dir)` — creates fake share dir with addon stubs + fake GDExtension
- `theatre(share, bin)` — returns Command with isolated env vars

### Environment Isolation

Every test MUST set these env vars to avoid touching real installed Theatre:
- `THEATRE_SHARE_DIR` → temp dir (prevents reading `~/.local/share/theatre`)
- `THEATRE_BIN_DIR` → temp dir (prevents writing to `~/.local/bin`)
- `THEATRE_ROOT` → temp dir or nonexistent path (prevents source repo discovery)
- `THEATRE_NO_TELEMETRY=1` (prevents HTTP calls)
- `DO_NOT_TRACK=1` (belt-and-suspenders telemetry disable)

### Tests That Cannot Be Automated Here

- `theatre install` — requires `cargo build` of the full workspace (slow, CI-only)
- `theatre deploy` with source build — same reason
- Interactive prompts (dialoguer) — require TTY; test `--yes` paths instead

### Deploy No-Build Path

The `deploy` command has a code path that skips `cargo build` when no source repo
is found and just copies from the share dir. This path IS testable with fake share dirs.

## Priority Order

1. **init_yes_copies_addons_and_generates_config** — validates the primary user journey
2. **enable_is_idempotent** — validates the most critical contract
3. **init_share_dir_missing** — validates the #1 error case (not installed yet)
4. **enable_stage_only / enable_director_only / disable_plugins** — flag combinations
5. **full_lifecycle_init_then_disable_then_reenable** — multi-step composition
6. **mcp_generates_valid_config / mcp_custom_port** — config generation
7. **Boundary conditions** (empty project.godot, existing plugins, etc.)
8. **rules tests** — lower priority since rules are optional
9. **deploy tests** — the no-build path is testable; build path is CI-only
10. **Graceful degradation** — discovers/documents current behavior
