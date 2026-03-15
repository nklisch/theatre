# Design: `project_reload` & `editor_status`

## Overview

Two new Director tools giving agents visibility into project health and editor state:

1. **`project_reload`** — replaces `filesystem_scan`. Kills the daemon, runs a validation
   one-shot that captures Godot's stderr, and returns structured diagnostics (parse errors,
   missing identifiers, broken references) alongside the reload confirmation.

2. **`editor_status`** — a Krometrail-style viewport into the Godot editor. Returns which
   scenes are open, which is active, whether the game is running, and registered autoloads.
   Works in headless mode too (returns autoloads only).

### Why

Agents building Godot projects are currently blind to:
- Script parse errors (e.g. `Identifier "EventBus" not declared`)
- Whether the editor is running and which scene is active
- Autoload registration state
- Whether the game is running

The old `filesystem_scan` tool was a silent no-op — it restarted the daemon but gave no
feedback. `project_reload` merges reload + validation into one tool. `editor_status` gives
agents on-demand orientation without requiring a mutation.

---

## Implementation Units

### Unit 1: Stderr Parsing Utility

**File**: `crates/director/src/diagnostics.rs`

```rust
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// A single diagnostic parsed from Godot's stderr output.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GodotDiagnostic {
    pub file: String,
    pub line: u32,
    pub severity: String, // "error" or "warning"
    pub message: String,
}

/// Parse Godot's stderr for error/warning lines.
///
/// Godot emits errors in the format:
///   `ERROR: res://path/file.gd:42 - Parse Error: Some message.`
///   `WARNING: res://path/file.gd:10 - Some warning message.`
///
/// Also handles the `modules/gdscript/gdscript.cpp` summary lines:
///   `ERROR: modules/gdscript/gdscript.cpp:2907 - Failed to load script "res://file.gd" ...`
///
/// Returns deduplicated diagnostics (Godot often repeats errors).
pub fn parse_godot_stderr(stderr: &str) -> Vec<GodotDiagnostic> {
    // implementation
}

#[cfg(test)]
mod tests {
    // covered in Unit 9
}
```

**Implementation Notes**:
- Regex pattern: `(ERROR|WARNING): (res://\S+):(\d+) - (.+)`
- For `modules/gdscript/` lines, extract the `res://` path from the message body using
  a secondary pattern: `Failed to load script "(res://[^"]+)"`
- Strip the `res://` prefix from file paths in output (agents work with project-relative paths)
- Deduplicate by `(file, line, message)` tuple — Godot repeats errors when re-parsing
- Sort by file path, then line number

**Acceptance Criteria**:
- [ ] Parses `ERROR: res://foo.gd:42 - Parse Error: ...` into `{file: "foo.gd", line: 42, severity: "error", message: "Parse Error: ..."}`
- [ ] Parses `WARNING:` lines with severity `"warning"`
- [ ] Extracts script path from `modules/gdscript/gdscript.cpp` summary lines
- [ ] Deduplicates identical diagnostics
- [ ] Returns empty vec for clean stderr

---

### Unit 2: `run_validation` Function

**File**: `crates/director/src/oneshot.rs`

```rust
/// Result of a validation run — operation result plus captured stderr.
pub struct ValidationOutput {
    pub result: OperationResult,
    pub stderr: String,
}

/// Run a Director operation via headless one-shot, returning stderr alongside the result.
///
/// Identical to `run_oneshot` but always returns stderr (even on success).
/// Used by `project_reload` to capture Godot's parse error output.
pub async fn run_validation(
    godot_bin: &Path,
    project_path: &Path,
    operation: &str,
    params: &serde_json::Value,
) -> Result<ValidationOutput, OperationError> {
    // implementation
}
```

**Implementation Notes**:
- Nearly identical to `run_oneshot`. Factor the shared subprocess-spawning code into a
  private helper `run_subprocess` that returns `(stdout, stderr, exit_status)`, then
  have both `run_oneshot` and `run_validation` call it.
- `run_validation` wraps the return as `ValidationOutput { result, stderr }`.
- On parse failure of stdout JSON, still return the stderr in the error (already happens
  via `ProcessFailed`).

**Acceptance Criteria**:
- [ ] Returns `ValidationOutput` with both the parsed JSON result and raw stderr
- [ ] Captures stderr even when the operation succeeds
- [ ] Shares subprocess logic with `run_oneshot` (no code duplication)

---

### Unit 3: `project_reload` GDScript Operation

**File**: `addons/director/ops/project_ops.gd`

Replace `op_filesystem_scan` with `op_project_reload`:

```gdscript
static func op_project_reload(params: Dictionary) -> Dictionary:
    ## Reload the project and report basic stats.
    ##
    ## The real diagnostics come from stderr (captured by the Rust side).
    ## This GDScript op provides supplementary data: script count and
    ## registered autoloads.
    ##
    ## Returns: { success, data: { scripts_checked, autoloads } }

    var scripts: Array = []
    _collect_gd_files("res://", scripts)

    var autoloads: Dictionary = {}
    var cfg := ConfigFile.new()
    if cfg.load("res://project.godot") == OK:
        if cfg.has_section("autoload"):
            for key in cfg.get_section_keys("autoload"):
                var value: String = str(cfg.get_value("autoload", key, ""))
                # Strip "*" prefix (enabled marker) and "res://" prefix
                autoloads[key] = value.trim_prefix("*").trim_prefix("res://")

    return {"success": true, "data": {
        "scripts_checked": scripts.size(),
        "autoloads": autoloads,
    }}


static func _collect_gd_files(dir_path: String, result: Array) -> void:
    ## Recursively collect .gd files.
    var dir = DirAccess.open(dir_path)
    if dir == null:
        return
    dir.list_dir_begin()
    var file_name = dir.get_next()
    while file_name != "":
        if file_name != "." and file_name != ".." \
                and not file_name.begins_with("."):
            var full = dir_path.trim_suffix("/") + "/" + file_name
            if dir.current_is_dir():
                if file_name != ".godot":
                    _collect_gd_files(full, result)
            elif file_name.get_extension() == "gd":
                result.append(full)
        file_name = dir.get_next()
    dir.list_dir_end()
```

**Implementation Notes**:
- Remove the old `op_filesystem_scan` function entirely.
- The `_collect_gd_files` helper is similar to the existing `_collect_resource_files` but
  only collects `.gd` files and skips `.godot/` directory.
- Autoload reading uses `ConfigFile` (same as `autoload_add`/`autoload_remove`).
- The GDScript side does NOT need to detect errors — Godot's own startup phase does that
  automatically and writes errors to stderr. The Rust side captures stderr.

**Acceptance Criteria**:
- [ ] Returns `scripts_checked` count of all `.gd` files in project
- [ ] Returns `autoloads` dictionary mapping name → script path
- [ ] Old `op_filesystem_scan` function removed

---

### Unit 4: `project_reload` Rust Handler

**File**: `crates/director/src/mcp/mod.rs` (handler), `crates/director/src/mcp/project.rs` (params)

Replace `filesystem_scan` handler and params:

```rust
// --- project.rs ---

/// Parameters for `project_reload`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ProjectReloadParams {
    /// Absolute path to the Godot project directory (must contain project.godot).
    pub project_path: String,
}

// --- responses.rs ---

/// Response for project_reload.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProjectReloadResponse {
    pub result: String,
    pub scripts_checked: u32,
    pub autoloads: serde_json::Value,
    pub errors: Vec<crate::diagnostics::GodotDiagnostic>,
    pub warnings: Vec<crate::diagnostics::GodotDiagnostic>,
}

// --- mod.rs handler ---

#[tool(
    name = "project_reload",
    description = "Reload the project and validate all scripts. Call this after creating or \
        modifying .gd script files outside of Director (e.g. via Write tool). Returns \
        structured diagnostics — script parse errors, missing identifiers, broken \
        references — so you can fix issues before they cause failures in scene operations. \
        In headless mode this restarts the daemon; in editor mode it triggers a filesystem \
        rescan. Replaces the old filesystem_scan tool."
)]
pub async fn project_reload(
    &self,
    Parameters(params): Parameters<ProjectReloadParams>,
) -> Result<String, McpError> {
    // implementation below
}
```

**Implementation Notes**:

The handler is custom (no `director_tool!` macro) because it needs:
1. To always use one-shot mode (never daemon — we need stderr)
2. To kill the daemon
3. To merge stderr diagnostics with the GDScript result

Flow:
```
1. resolve godot binary + validate project path
2. kill daemon (same as old filesystem_scan)
3. run_validation(godot, project, "project_reload", params) → ValidationOutput
4. parse ValidationOutput.stderr → Vec<GodotDiagnostic>
5. split diagnostics into errors and warnings
6. merge GDScript data (scripts_checked, autoloads) with parsed diagnostics
7. best-effort: try editor "filesystem_scan" op (triggers post-op sync)
8. serialize and return ProjectReloadResponse
```

Step 7 detail: after the validation one-shot, if the editor is reachable, send the old
`filesystem_scan` no-op through `try_editor` to trigger `_post_operation_sync` (which
calls `EditorInterface.get_resource_filesystem().scan()`). Ignore errors — this is
best-effort. Keep the GDScript `op_filesystem_scan` function around (renamed to a
private helper or inlined) solely for the editor sync path, OR just send a `ping`
and let the next real operation trigger the sync.

Simpler approach for step 7: skip it. The next operation the agent runs through the
editor will trigger `_post_operation_sync` automatically. Document that `project_reload`
restarts the daemon and validates — the editor resync happens on the next editor operation.

```rust
pub async fn project_reload(
    &self,
    Parameters(params): Parameters<ProjectReloadParams>,
) -> Result<String, McpError> {
    use crate::diagnostics::parse_godot_stderr;
    use crate::oneshot::run_validation;
    use crate::resolve::{resolve_godot_bin, validate_project_path};

    let godot = resolve_godot_bin().map_err(McpError::from)?;
    let project = std::path::Path::new(&params.project_path);
    validate_project_path(project).map_err(McpError::from)?;

    // Kill stale daemon so next operation spawns fresh.
    self.backend.kill_daemon().await;

    // Run validation via one-shot (captures stderr).
    let op_params = serialize_params(&params)?;
    let validation = run_validation(&godot, project, "project_reload", &op_params)
        .await
        .map_err(McpError::from)?;

    // Parse stderr for Godot diagnostics.
    let diagnostics = parse_godot_stderr(&validation.stderr);
    let errors: Vec<_> = diagnostics.iter().filter(|d| d.severity == "error").cloned().collect();
    let warnings: Vec<_> = diagnostics.iter().filter(|d| d.severity == "warning").cloned().collect();

    // Extract GDScript data.
    let data = validation.result.into_data().map_err(McpError::from)?;
    let scripts_checked = data.get("scripts_checked")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let autoloads = data.get("autoloads").cloned()
        .unwrap_or(serde_json::Value::Object(Default::default()));

    serialize_response(&ProjectReloadResponse {
        result: "ok".to_string(),
        scripts_checked,
        autoloads,
        errors,
        warnings,
    })
}
```

**Acceptance Criteria**:
- [ ] Kills daemon before validation
- [ ] Runs one-shot subprocess (never uses daemon for validation)
- [ ] Returns script parse errors from Godot stderr as structured diagnostics
- [ ] Returns autoloads dictionary from project.godot
- [ ] Returns scripts_checked count
- [ ] Old `filesystem_scan` handler removed

---

### Unit 5: `editor_status` GDScript Operation

**File**: `addons/director/ops/project_ops.gd`

```gdscript
static func op_editor_status(params: Dictionary) -> Dictionary:
    ## Return a snapshot of editor state (or basic project state in headless).
    ##
    ## In editor context (dispatched via editor_ops.gd), this is augmented
    ## by _live_editor_status which adds open scenes, active scene, etc.
    ##
    ## In headless context, returns autoloads and editor_connected=false.
    ##
    ## Returns: { success, data: { editor_connected, active_scene,
    ##   open_scenes, game_running, autoloads } }

    var autoloads: Dictionary = {}
    var cfg := ConfigFile.new()
    if cfg.load("res://project.godot") == OK:
        if cfg.has_section("autoload"):
            for key in cfg.get_section_keys("autoload"):
                var value: String = str(cfg.get_value("autoload", key, ""))
                autoloads[key] = value.trim_prefix("*").trim_prefix("res://")

    # Read recent log (works in headless too — same log file)
    var recent_log: Array[String] = []
    var log_path := OS.get_user_data_dir() + "/logs/godot.log"
    if FileAccess.file_exists(log_path):
        var file := FileAccess.open(log_path, FileAccess.READ)
        if file != null:
            var content := file.get_as_text()
            var lines := content.split("\n")
            var start := maxi(0, lines.size() - 50)
            for i in range(start, lines.size()):
                var line := lines[i].strip_edges()
                if line != "":
                    recent_log.append(lines[i])

    return {"success": true, "data": {
        "editor_connected": false,
        "active_scene": "",
        "open_scenes": [],
        "game_running": false,
        "autoloads": autoloads,
        "recent_log": recent_log,
    }}
```

**File**: `addons/director/editor_ops.gd`

Add `editor_status` to `_dispatch_live` as a special case (it doesn't target a scene but
needs the editor), and also handle it in `_dispatch_headless`. Better: handle it in
`dispatch()` directly before the `SCENE_OPS` check, since it's editor-global not
scene-specific.

```gdscript
# In dispatch(), before the SCENE_OPS check:
static func dispatch(operation: String, params: Dictionary) -> Dictionary:
    var scene_path: String = params.get("scene_path", "")

    # Editor-global operations that need EditorInterface but not a specific scene.
    if operation == "editor_status":
        return _editor_status()

    # For scene-targeting operations, check if the scene is the active tab.
    if scene_path != "" and operation in SCENE_OPS:
        # ... existing logic

# New function:
static func _editor_status() -> Dictionary:
    var open_scenes := EditorInterface.get_open_scenes()
    var active_root := EditorInterface.get_edited_scene_root()
    var active_scene := ""
    if active_root != null:
        active_scene = active_root.scene_file_path.trim_prefix("res://")

    var playing := EditorInterface.is_playing_scene()

    # Read autoloads from project.godot
    var autoloads: Dictionary = {}
    var cfg := ConfigFile.new()
    if cfg.load("res://project.godot") == OK:
        if cfg.has_section("autoload"):
            for key in cfg.get_section_keys("autoload"):
                var value: String = str(cfg.get_value("autoload", key, ""))
                autoloads[key] = value.trim_prefix("*").trim_prefix("res://")

    # Clean up open_scenes paths
    var cleaned_scenes: Array[String] = []
    for s in open_scenes:
        cleaned_scenes.append(s.trim_prefix("res://"))

    # Read recent log
    var recent_log: Array[String] = _read_recent_log()

    return {"success": true, "data": {
        "editor_connected": true,
        "active_scene": active_scene,
        "open_scenes": cleaned_scenes,
        "game_running": playing,
        "autoloads": autoloads,
        "recent_log": recent_log,
    }}


static func _read_recent_log() -> Array[String]:
    ## Read the last 50 non-empty lines from godot.log.
    var log_path := OS.get_user_data_dir() + "/logs/godot.log"
    var result: Array[String] = []
    if not FileAccess.file_exists(log_path):
        return result
    var file := FileAccess.open(log_path, FileAccess.READ)
    if file == null:
        return result
    var content := file.get_as_text()
    var lines := content.split("\n")
    var start := maxi(0, lines.size() - 50)
    for i in range(start, lines.size()):
        var line := lines[i].strip_edges()
        if line != "":
            result.append(lines[i])
    return result
```

**Implementation Notes**:
- In the editor path, `_editor_status` is called directly from `dispatch()` — it
  never falls through to `_dispatch_headless`.
- In headless mode (daemon, one-shot), the normal `op_editor_status` in
  `project_ops.gd` runs, which returns `editor_connected: false`.
- `EditorInterface.get_open_scenes()` returns `PackedStringArray` of `res://` paths.
- `EditorInterface.is_playing_scene()` returns `true` when F5 is active.
- Paths are stripped of `res://` prefix for consistency with all other Director tools.

#### Log file reading

Godot writes ALL output (errors, warnings, script backtraces, print statements) to a
log file at `<user_data_dir>/logs/godot.log`. The user data dir is project-specific:
`~/.local/share/godot/app_userdata/<project_name>/logs/` on Linux.

Both the GDScript and Rust sides can read this. The GDScript approach is simpler since
`OS.get_user_data_dir()` resolves the path automatically:

```gdscript
# In _editor_status() and op_editor_status():
var log_path := OS.get_user_data_dir() + "/logs/godot.log"
var recent_log: Array[String] = []
if FileAccess.file_exists(log_path):
    var file := FileAccess.open(log_path, FileAccess.READ)
    if file != null:
        var content := file.get_as_text()
        var lines := content.split("\n")
        # Take the last N lines (token budget)
        var start := maxi(0, lines.size() - 50)
        for i in range(start, lines.size()):
            if lines[i].strip_edges() != "":
                recent_log.append(lines[i])
```

The `recent_log` array is included in the response. The Rust `EditorStatusResponse`
struct gets a `recent_log: Vec<String>` field.

The same `diagnostics::parse_godot_stderr` function can parse the log file content
to extract structured errors. The Rust handler does this after receiving the GDScript
response — it takes the `recent_log` lines, joins them, runs `parse_godot_stderr`,
and populates `errors` and `warnings` fields on the response.

**Acceptance Criteria**:
- [ ] Editor path returns `editor_connected: true`, open scenes, active scene, game running state
- [ ] Both editor and headless paths include `recent_log` (last 50 lines of godot.log)
- [ ] Rust handler parses `recent_log` into structured `errors` and `warnings`
- [ ] Headless path returns `editor_connected: false`, autoloads + log
- [ ] All paths are project-relative (no `res://` prefix)
- [ ] Autoloads dictionary matches `project_reload` format

---

### Unit 6: `editor_status` Rust Types & Handler

**File**: `crates/director/src/mcp/project.rs` (params), `crates/director/src/responses.rs` (response)

```rust
// --- project.rs ---

/// Parameters for `editor_status`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct EditorStatusParams {
    /// Absolute path to the Godot project directory (must contain project.godot).
    pub project_path: String,
}

// --- responses.rs ---

/// Response for editor_status.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EditorStatusResponse {
    /// Whether the Godot editor is running and connected.
    pub editor_connected: bool,

    /// Currently active scene in the editor (project-relative path, empty if none).
    pub active_scene: String,

    /// All scenes currently open in editor tabs.
    pub open_scenes: Vec<String>,

    /// Whether the game is currently running (F5 pressed).
    pub game_running: bool,

    /// Registered autoload singletons (name → script path).
    pub autoloads: serde_json::Value,

    /// Recent lines from godot.log (last 50 lines, includes errors/warnings/print output).
    pub recent_log: Vec<String>,

    /// Structured errors parsed from the log.
    pub errors: Vec<crate::diagnostics::GodotDiagnostic>,

    /// Structured warnings parsed from the log.
    pub warnings: Vec<crate::diagnostics::GodotDiagnostic>,
}
```

**File**: `crates/director/src/mcp/mod.rs`

```rust
#[tool(
    name = "editor_status",
    description = "Get a snapshot of the Godot editor's current state — which scenes are \
        open, which is active, whether the game is running, registered autoloads, and \
        recent log output (errors, warnings, print statements from godot.log). \
        Use this to orient yourself before making changes, to check whether the editor \
        is running, or to see what errors exist. Works in headless mode too."
)]
pub async fn editor_status(
    &self,
    Parameters(params): Parameters<EditorStatusParams>,
) -> Result<String, McpError> {
    // Custom handler — needs to parse recent_log into structured diagnostics.
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "editor_status", &op_params)
        .await?;

    // Deserialize the GDScript response.
    let raw: EditorStatusRawResponse = deserialize_response(data)?;

    // Parse recent_log lines into structured diagnostics.
    let log_text = raw.recent_log.join("\n");
    let diagnostics = crate::diagnostics::parse_godot_stderr(&log_text);
    let errors: Vec<_> = diagnostics.iter().filter(|d| d.severity == "error").cloned().collect();
    let warnings: Vec<_> = diagnostics.iter().filter(|d| d.severity == "warning").cloned().collect();

    serialize_response(&EditorStatusResponse {
        editor_connected: raw.editor_connected,
        active_scene: raw.active_scene,
        open_scenes: raw.open_scenes,
        game_running: raw.game_running,
        autoloads: raw.autoloads,
        recent_log: raw.recent_log,
        errors,
        warnings,
    })
}
```

The handler is custom (no `director_tool!`) because it needs to post-process the
`recent_log` field through `parse_godot_stderr` to produce structured diagnostics.

An intermediate `EditorStatusRawResponse` struct (private, in `responses.rs`) mirrors
the GDScript response shape without the `errors`/`warnings` fields:

```rust
/// Raw response from GDScript — deserialized before Rust-side log parsing.
#[derive(Debug, Deserialize)]
pub(crate) struct EditorStatusRawResponse {
    pub editor_connected: bool,
    pub active_scene: String,
    pub open_scenes: Vec<String>,
    pub game_running: bool,
    pub autoloads: serde_json::Value,
    pub recent_log: Vec<String>,
}
```

**Implementation Notes**:
- NOT using `director_tool!` — needs post-processing of `recent_log`.
- When the editor is running, the backend routes to the editor plugin, which calls
  `_editor_status()` in `editor_ops.gd` and returns the rich response with log.
- When the editor is not running, it falls back to daemon/one-shot, which calls
  `op_editor_status()` in `project_ops.gd` and returns the basic response with log.
- The same `parse_godot_stderr` function from `diagnostics.rs` is reused.
- `recent_log` is passed through as raw lines so agents can see the full output.
- `errors` and `warnings` are the structured parse of those same lines.

**Acceptance Criteria**:
- [ ] Editor path returns rich data via `_editor_status()`
- [ ] Headless fallback returns basic data via `op_editor_status()`
- [ ] Both paths include `recent_log` from godot.log
- [ ] Rust handler parses `recent_log` into structured `errors` and `warnings`

---

### Unit 7: Dispatch Table Updates

Update all 6 GDScript dispatch tables. Changes:
- Remove `"filesystem_scan"` entries
- Add `"project_reload"` entries pointing to `ProjectOps.op_project_reload(params)`
- Add `"editor_status"` entries pointing to `ProjectOps.op_editor_status(params)`

**Files to update**:
1. `addons/director/daemon.gd` — `_dispatch()`
2. `addons/director/editor_ops.gd` — `_dispatch_headless()` (note: `editor_status` is
   handled in `dispatch()` directly for the editor path, but needs a headless entry too
   for the fallthrough case)
3. `addons/director/operations.gd` — one-shot `match`
4. `addons/director/ops/meta_ops.gd` — `_dispatch_single()` for batch
5. `addons/director/mock_editor_server.gd` — `_dispatch()`

**Special case for `editor_ops.gd`**:
- `editor_status` is intercepted in `dispatch()` before the `SCENE_OPS` check (Unit 5)
- It must ALSO be in `_dispatch_headless()` as a fallback for when the editor dispatches
  it headlessly (which shouldn't happen given the intercept, but completeness)

**Acceptance Criteria**:
- [ ] No remaining references to `"filesystem_scan"` in any dispatch table
- [ ] `"project_reload"` in all 5 dispatch tables
- [ ] `"editor_status"` in all 5 dispatch tables + special handling in `editor_ops.dispatch()`

---

### Unit 8: Remove Old Types, Wire New Types

**Files**:
- `crates/director/src/mcp/project.rs` — remove `FilesystemScanParams`, add `ProjectReloadParams`, `EditorStatusParams`
- `crates/director/src/responses.rs` — remove `FilesystemScanResponse`, add `ProjectReloadResponse`, `EditorStatusResponse`
- `crates/director/src/server.rs` — update `attach_output_schema` calls
- `crates/director/src/mcp/mod.rs` — update imports, remove `filesystem_scan` handler, add new handlers
- `crates/director/src/lib.rs` — add `pub mod diagnostics;`

**Acceptance Criteria**:
- [ ] `FilesystemScanParams` and `FilesystemScanResponse` removed
- [ ] `ProjectReloadParams`, `ProjectReloadResponse`, `EditorStatusParams`, `EditorStatusResponse` added
- [ ] Output schemas attached for both new tools
- [ ] `diagnostics` module exposed from lib.rs
- [ ] Compiles cleanly with `cargo build -p director`

---

## Implementation Order

1. **Unit 1** — `diagnostics.rs` (stderr parser, no dependencies)
2. **Unit 2** — `run_validation` in `oneshot.rs` (depends on nothing new)
3. **Unit 3** — GDScript `op_project_reload` and `op_editor_status` in `project_ops.gd`
4. **Unit 5** — `_editor_status()` in `editor_ops.gd`
5. **Unit 7** — dispatch table updates (all 6 files)
6. **Unit 8** — Rust types: remove old, add new params/responses
7. **Unit 4** — `project_reload` Rust handler (depends on Units 1, 2, 8)
8. **Unit 6** — `editor_status` Rust handler (depends on Unit 8)

---

## Testing

### Unit Tests: `crates/director/src/diagnostics.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_script_error() {
        let stderr = r#"ERROR: res://scenes/game/grid/grid.gd:70 - Parse Error: Identifier "GameState" not declared in the current scope."#;
        let diags = parse_godot_stderr(stderr);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].file, "scenes/game/grid/grid.gd");
        assert_eq!(diags[0].line, 70);
        assert_eq!(diags[0].severity, "error");
        assert!(diags[0].message.contains("GameState"));
    }

    #[test]
    fn parse_warning() {
        let stderr = "WARNING: res://scripts/old.gd:5 - Unused variable 'x'.";
        let diags = parse_godot_stderr(stderr);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, "warning");
    }

    #[test]
    fn parse_gdscript_cpp_summary_line() {
        let stderr = r#"ERROR: modules/gdscript/gdscript.cpp:2907 - Failed to load script "res://debug/test_grid.gd" with error "Parse error"."#;
        let diags = parse_godot_stderr(stderr);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].file, "debug/test_grid.gd");
    }

    #[test]
    fn deduplicates_repeated_errors() {
        let stderr = "\
ERROR: res://foo.gd:10 - Parse Error: bad.\n\
ERROR: res://foo.gd:10 - Parse Error: bad.\n\
ERROR: res://foo.gd:10 - Parse Error: bad.";
        let diags = parse_godot_stderr(stderr);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn empty_stderr_returns_empty() {
        let diags = parse_godot_stderr("");
        assert!(diags.is_empty());
    }

    #[test]
    fn ignores_non_error_lines() {
        let stderr = "Godot Engine v4.6.1.stable.official\n\
[Stage] TCP server listening on 127.0.0.1:9077\n\
[Stage] TCP server stopped";
        let diags = parse_godot_stderr(stderr);
        assert!(diags.is_empty());
    }
}
```

### Unit Tests: `crates/director/src/mcp/mod.rs`

Add to existing `mod tests`:

```rust
#[test]
fn project_reload_params_no_nulls() {
    let params = ProjectReloadParams {
        project_path: "/proj".into(),
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_no_nulls(&json);
}

#[test]
fn editor_status_params_no_nulls() {
    let params = EditorStatusParams {
        project_path: "/proj".into(),
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_no_nulls(&json);
}
```

### E2E Tests: `tests/director-tests/`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn project_reload_returns_diagnostics() {
    // Run project_reload against the test project
    // Verify scripts_checked > 0
    // Verify autoloads is a dict
    // Verify errors/warnings are arrays
}

#[test]
#[ignore = "requires Godot binary"]
fn editor_status_headless_returns_basic_data() {
    // Run editor_status in headless mode
    // Verify editor_connected == false
    // Verify autoloads is a dict
    // Verify open_scenes is empty array
}
```

---

## Verification Checklist

```bash
# Build
cargo build -p director

# Unit tests
cargo test -p director

# Lint
cargo clippy -p director
cargo fmt --check

# E2E (requires theatre deploy first)
theatre deploy ~/dev/theatre/tests/godot-project
cargo test --workspace

# Regenerate docs schema
./site/scripts/generate-schema.sh
```

---

## Migration Notes

- `filesystem_scan` is fully replaced by `project_reload`. No backward compatibility shim.
- The GDScript `op_filesystem_scan` function is removed from `project_ops.gd`.
- All dispatch tables updated: `filesystem_scan` entries → `project_reload`.
- The `autoload_add` tool description previously referenced `filesystem_scan` — update to
  reference `project_reload`.
- Site docs at `site/director/scenes.md` must be updated: rename `filesystem_scan` section
  to `project_reload`, update the workflow example.
- The `director-tool-macro.md` skill mentions `filesystem_scan` — update to reference
  `project_reload`.
