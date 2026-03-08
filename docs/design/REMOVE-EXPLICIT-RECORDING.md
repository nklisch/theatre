# Design: Remove Explicit Recording, Keep Dashcam Only

## Overview

Remove the explicit recording subsystem (M7 start/stop recording + M8 analysis)
while keeping the dashcam (M11) as the sole capture mechanism. The explicit
recording feature adds significant complexity — separate start/stop lifecycle,
SQLite management in the server, analysis queries — but dashcam already covers
the primary use case: automatic context capture around interesting events.

**What's removed:**
- Explicit recording start/stop/status lifecycle
- M8 recording analysis (snapshot_at, query_range, diff_frames, find_event)
- Server-side `recording_analysis.rs` (entire 1437-line module)
- Recording-specific UI (record/stop buttons, recording indicator dot, F12 keybind)
- `rusqlite` and `rmp-serde` dependencies from `spectator-server`
- `recording_storage_path` from `SessionState`
- Recording project settings (storage_path, max_frames, capture_interval)

**What's kept:**
- Dashcam ring buffer, state machine, clip saving (all in `recorder.rs`)
- `add_marker` action (routes to dashcam trigger)
- `dashcam_status` and `flush_dashcam` MCP actions
- `list` and `delete` actions (for dashcam clips)
- `markers` action (for dashcam clips)
- SQLite in `spectator-godot` (dashcam writes clips to SQLite)
- `FrameEntityData` in `spectator-protocol` (dashcam still uses it)
- Dashcam config in TOML and `SessionConfig`

**What's simplified:**
- `SpectatorRecorder` drops all explicit recording state and methods
- The `recording` MCP tool is renamed to `dashcam` with a smaller action set
- The MCP `RecordingParams` struct shrinks dramatically (no analysis fields)
- `recording_handler.rs` drops 7 TCP methods, keeps 5
- Dock UI drops record/stop/marker buttons and recording stats
- runtime.gd drops recording overlay, F12 keybind, recording signal handlers
- Activity summary drops explicit recording entries

## Implementation Units

### Unit 1: Strip Explicit Recording from SpectatorRecorder

**File**: `crates/spectator-godot/src/recorder.rs`

Remove all explicit recording state and methods. Keep dashcam state machine,
ring buffer, clip saving, and SQLite write logic.

**Remove from struct fields:**
```rust
// DELETE these fields from SpectatorRecorder:
recording: bool,
recording_id: String,
recording_name: String,
started_at_frame: u64,
started_at_ms: u64,
frames_captured: u32,
frame_counter: u32,
capture_interval: u32,
max_frames: u32,
frame_buffer: Vec<CapturedFrame>,
event_buffer: Vec<CapturedEvent>,
marker_buffer: Vec<CapturedMarker>,
flush_counter: u32,
db: Option<Connection>,
storage_path: String,
```

**Remove these buffer types:**
```rust
// DELETE CapturedEvent (only used by explicit recording)
struct CapturedEvent { ... }
// DELETE CapturedMarker (only used by explicit recording)
struct CapturedMarker { ... }
// KEEP CapturedFrame (used by dashcam ring buffer)
```

**Remove exported GDExtension methods:**
```rust
// DELETE all #[func] methods for explicit recording:
fn start_recording(name, storage_path, capture_interval, max_frames) -> String
fn stop_recording() -> Dictionary
fn is_recording() -> bool
fn get_recording_id() -> GString
fn get_recording_name() -> GString
fn get_frames_captured() -> u32
fn get_elapsed_ms() -> u64
fn get_buffer_size_kb() -> u32
fn get_recording_status() -> Dictionary
```

**Remove signals:**
```rust
// DELETE these signals:
#[signal] fn recording_started(recording_id: GString, name: GString);
#[signal] fn recording_stopped(recording_id: GString, frames: u32);
// KEEP these signals:
#[signal] fn marker_added(frame: u64, source: GString, label: GString);
#[signal] fn dashcam_clip_saved(recording_id: GString, tier: GString, frames: u32);
#[signal] fn dashcam_clip_started(trigger_frame: u64, tier: GString);
```

**Simplify `add_marker`:**
The current `add_marker` has two paths: explicit recording vs dashcam. Remove
the explicit recording path. It now always routes to dashcam:
```rust
#[func]
pub fn add_marker(&mut self, source: GString, label: GString) {
    // Always route to dashcam trigger
    if matches!(self.dashcam_state, DashcamState::Buffering | DashcamState::PostCapture { .. }) {
        let tier = if source.to_string() == "system" {
            DashcamTier::System
        } else {
            DashcamTier::Deliberate
        };
        self.on_dashcam_marker(source.to_string(), label.to_string(), tier);
        // Emit marker_added signal
        let frame = Engine::singleton().get_physics_frames();
        self.base_mut().emit_signal("marker_added", &[
            frame.to_variant(),
            source.to_variant(),
            label.to_variant(),
        ]);
    }
}
```

**Simplify `physics_process`:**
Remove the explicit recording capture path. The physics_process only needs to
handle dashcam:
```rust
fn physics_process(&mut self, _delta: f64) {
    self.dashcam_check_force_close();

    if matches!(self.dashcam_state, DashcamState::Disabled) {
        return;
    }

    // Dashcam capture at interval
    self.frame_counter += 1;
    if self.frame_counter < self.dashcam_config.capture_interval {
        return;
    }
    self.frame_counter = 0;

    if let Some(captured) = self.do_capture() {
        self.dashcam_ingest(captured);
    }
}
```

**Remove periodic SQLite flush logic** (`flush_to_db`, the 60-frame flush timer).
Dashcam only writes to SQLite when a clip is saved (`flush_dashcam_clip_internal`).

**Keep all dashcam methods unchanged:**
- `set_dashcam_enabled`, `is_dashcam_active`, `get_dashcam_state`
- `get_dashcam_buffer_frames`, `get_dashcam_buffer_kb`
- `trigger_dashcam_clip`, `flush_dashcam_clip`
- `apply_dashcam_config`, `get_dashcam_config_json`
- `list_recordings`, `delete_recording`, `get_recording_markers`

**Keep `storage_path` parameter** on `list_recordings` and `delete_recording` —
dashcam clips are still stored as SQLite files.

**Acceptance Criteria:**
- [ ] `SpectatorRecorder` has no `recording: bool` or explicit recording state
- [ ] No `start_recording` / `stop_recording` methods exist
- [ ] `recording_started` / `recording_stopped` signals removed
- [ ] `physics_process` only captures for dashcam
- [ ] `add_marker` only routes to dashcam
- [ ] All dashcam methods still work (ring buffer, clip save, config)
- [ ] All existing dashcam unit tests pass
- [ ] `CapturedEvent` and `CapturedMarker` types deleted

---

### Unit 2: Simplify Recording Handler (TCP Dispatch)

**File**: `crates/spectator-godot/src/recording_handler.rs`

Remove TCP method handlers for explicit recording. Keep dashcam-related handlers.

**Remove methods:**
```rust
// DELETE handlers for:
"recording_start"
"recording_stop"
"recording_status"
```

**Keep methods (rename prefix from `recording_` to `dashcam_` where sensible):**
```rust
// KEEP as-is (these work for dashcam clips stored on disk):
"recording_list"     // lists dashcam clips — keep method name for now
"recording_delete"   // deletes a dashcam clip
"recording_marker"   // routes to add_marker → dashcam trigger
"recording_markers"  // reads markers from a saved dashcam clip's SQLite
"recording_resolve_path"  // resolves storage directory

// KEEP dashcam-specific:
"dashcam_status"
"dashcam_flush"
"dashcam_config"
```

**Update `handle_recording_query` dispatch:**
```rust
pub fn handle_recording_query(
    recorder: &mut Gd<SpectatorRecorder>,
    method: &str,
    params: &Value,
) -> Result<Value, String> {
    match method {
        "recording_list" => handle_list(recorder, params),
        "recording_delete" => handle_delete(recorder, params),
        "recording_marker" => handle_marker(recorder, params),
        "recording_markers" => handle_markers(recorder, params),
        "recording_resolve_path" => handle_resolve_path(recorder, params),
        "dashcam_status" => handle_dashcam_status(recorder, params),
        "dashcam_flush" => handle_dashcam_flush(recorder, params),
        "dashcam_config" => handle_dashcam_config(recorder, params),
        _ => Err(format!("Unknown method: {method}")),
    }
}
```

**Simplify `handle_marker`:**
Remove the `is_recording` branch. Marker always goes to dashcam:
```rust
fn handle_marker(recorder: &mut Gd<SpectatorRecorder>, params: &Value) -> Result<Value, String> {
    let source = params["source"].as_str().unwrap_or("agent");
    let label = params["label"].as_str().unwrap_or("");
    recorder.bind_mut().add_marker(source.into(), label.into());
    // Response includes dashcam trigger info
    let r = recorder.bind();
    Ok(json!({
        "ok": true,
        "frame": Engine::singleton().get_physics_frames(),
        "source": source,
        "label": label,
        "dashcam_triggered": r.is_dashcam_active(),
    }))
}
```

**Acceptance Criteria:**
- [ ] `recording_start`, `recording_stop`, `recording_status` methods return error or don't exist
- [ ] Remaining methods dispatch correctly
- [ ] Marker handler only routes to dashcam

---

### Unit 3: Simplify TCP Server Query Routing

**File**: `crates/spectator-godot/src/tcp_server.rs`

The existing routing (`method.starts_with("recording_") || method.starts_with("dashcam_")`)
still works. No structural change needed here — just verify that removed methods
produce a clean error from `recording_handler.rs` returning `Err`.

**Acceptance Criteria:**
- [ ] Unknown recording methods return a clean TCP error response
- [ ] Dashcam methods continue to route correctly

---

### Unit 4: Rename MCP Tool and Slim Parameters

**File**: `crates/spectator-server/src/mcp/recording.rs` → rename to `crates/spectator-server/src/mcp/dashcam.rs`

Rename the MCP tool from `recording` to `dashcam`. Dramatically reduce the
parameter struct by removing all M8 analysis fields and explicit recording fields.

**New parameter struct:**
```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DashcamParams {
    /// Action to perform.
    /// "status" — return dashcam ring buffer state and config.
    /// "flush" — force-save the ring buffer as a clip.
    /// "add_marker" — add an agent marker (triggers dashcam clip capture).
    /// "list" — list saved dashcam clips.
    /// "delete" — remove a clip by recording_id.
    /// "markers" — list markers in a saved clip.
    pub action: String,

    /// Recording to query (markers, delete). Uses most recent if omitted.
    pub recording_id: Option<String>,

    /// Marker label (add_marker, flush only).
    pub marker_label: Option<String>,

    /// Soft token budget for the response.
    pub token_budget: Option<u32>,
}
```

**DELETE entirely:**
- `CaptureConfig` struct
- All M8 analysis fields: `at_frame`, `at_time_ms`, `detail`, `from_frame`,
  `to_frame`, `node`, `condition`, `event_type`, `event_filter`, `frame_a`, `frame_b`
- `recording_name` field
- `capture` field

**New handler:**
```rust
pub async fn handle_dashcam(
    params: DashcamParams,
    state: &Arc<Mutex<SessionState>>,
) -> Result<String, McpError> {
    let config = crate::tcp::get_config(state).await;
    let hard_cap = config.token_hard_cap;
    let budget_limit = resolve_budget(params.token_budget, 1500, hard_cap);

    match params.action.as_str() {
        "status" => query_and_finalize(state, "dashcam_status", json!({}), budget_limit, hard_cap).await,
        "flush" => handle_flush(&params, state, budget_limit, hard_cap).await,
        "add_marker" => handle_add_marker(&params, state, budget_limit, hard_cap).await,
        "list" => query_and_finalize(state, "recording_list", json!({}), budget_limit, hard_cap).await,
        "delete" => handle_delete(&params, state, budget_limit, hard_cap).await,
        "markers" => handle_markers(&params, state, budget_limit, hard_cap).await,
        other => Err(McpError::invalid_params(
            format!("Unknown dashcam action: '{other}'. Valid: status, flush, add_marker, list, delete, markers"),
            None,
        )),
    }
}
```

**Acceptance Criteria:**
- [ ] MCP tool named `dashcam` (not `recording`)
- [ ] Only 6 actions: status, flush, add_marker, list, delete, markers
- [ ] No M8 analysis fields in parameter struct
- [ ] No CaptureConfig struct
- [ ] All 6 actions work end-to-end

---

### Unit 5: Delete recording_analysis.rs

**File**: `crates/spectator-server/src/recording_analysis.rs` — DELETE entire file

This 1437-line module handles all M8 analysis operations (snapshot_at,
query_range, diff_frames, find_event). With explicit recording removed,
there's no stored recording to analyze — dashcam clips are captured and saved
but not analyzed server-side.

**Also remove from `crates/spectator-server/src/main.rs` or `lib.rs`:**
```rust
// DELETE: mod recording_analysis;
```

**Acceptance Criteria:**
- [ ] `recording_analysis.rs` deleted
- [ ] No references to `recording_analysis` in codebase
- [ ] `rusqlite` and `rmp-serde` removed from `spectator-server/Cargo.toml`

---

### Unit 6: Remove rusqlite/rmp-serde from spectator-server

**File**: `crates/spectator-server/Cargo.toml`

Remove `rusqlite` and `rmp-serde` dependencies. These were only used by
`recording_analysis.rs` for reading SQLite recordings and deserializing
MessagePack frames. Dashcam clips are written by the GDExtension
(`spectator-godot`), not read by the server.

**Also remove from `SessionState`:**
```rust
// DELETE from SessionState:
pub recording_storage_path: Option<String>,
```

**Acceptance Criteria:**
- [ ] `rusqlite` not in spectator-server Cargo.toml
- [ ] `rmp-serde` not in spectator-server Cargo.toml
- [ ] `recording_storage_path` removed from SessionState
- [ ] `cargo build -p spectator-server` succeeds

---

### Unit 7: Update MCP Tool Router

**File**: `crates/spectator-server/src/mcp/mod.rs`

Rename the tool registration from `recording` to `dashcam`.

```rust
// BEFORE:
pub mod recording;

// AFTER:
pub mod dashcam;
```

Update the `#[tool_router]` block:
```rust
/// Dashcam: always-on ring buffer that captures clips around interesting events.
#[tool(description = "Dashcam: always-on capture buffer. \
    'status' — ring buffer state and config. \
    'flush' — force-save current buffer as a clip. \
    'add_marker' — drop an agent marker (triggers clip capture). \
    'list' — list saved clips. \
    'delete' — remove a clip by recording_id. \
    'markers' — list markers in a saved clip.")]
pub async fn dashcam(
    &self,
    Parameters(params): Parameters<dashcam::DashcamParams>,
) -> Result<String, McpError> {
    let summary = crate::activity::dashcam_summary(&params);
    let result = dashcam::handle_dashcam(params, &self.state).await;
    self.log_activity("query", &summary, "dashcam").await;
    result
}
```

**Acceptance Criteria:**
- [ ] Tool named `dashcam` in MCP schema
- [ ] Old `recording` tool name no longer exists
- [ ] Tool description mentions only dashcam actions

---

### Unit 8: Update Activity Summaries

**File**: `crates/spectator-server/src/activity.rs`

Replace `recording_summary` with `dashcam_summary`. Remove all explicit
recording and M8 analysis entries.

```rust
use crate::mcp::dashcam::DashcamParams;

pub fn dashcam_summary(params: &DashcamParams) -> String {
    match params.action.as_str() {
        "status" => "Dashcam status".into(),
        "flush" => {
            let label = params.marker_label.as_deref().unwrap_or("agent flush");
            format!("Flushed dashcam clip: {label}")
        }
        "add_marker" => {
            let label = params.marker_label.as_deref().unwrap_or("(no label)");
            format!("Dashcam marker: {label}")
        }
        "list" => "Listing dashcam clips".into(),
        "delete" => {
            let id = params.recording_id.as_deref().unwrap_or("?");
            format!("Deleted clip {id}")
        }
        "markers" => {
            let id = params.recording_id.as_deref().unwrap_or("latest");
            format!("Listing markers for clip {id}")
        }
        other => format!("Dashcam: {other}"),
    }
}
```

**Acceptance Criteria:**
- [ ] No references to `RecordingParams` in activity.rs
- [ ] All 6 dashcam actions have summaries
- [ ] No M8 analysis summaries (snapshot_at, query_range, etc.)

---

### Unit 9: Update TOML Config

**File**: `crates/spectator-server/src/config.rs`

Rename `RecordingConfig` to `DashcamConfig` for clarity. The TOML section
stays as `[recording]` or can be renamed to `[dashcam]` (breaking change is
fine — no users).

```rust
#[derive(Debug, Default, Deserialize)]
pub struct SpectatorToml {
    pub connection: Option<ConnectionConfig>,
    pub tracking: Option<TrackingConfig>,
    pub dashcam: Option<DashcamTomlConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub struct DashcamTomlConfig {
    pub enabled: Option<bool>,
    pub capture_interval: Option<u32>,
    pub pre_window_system_sec: Option<u32>,
    pub pre_window_deliberate_sec: Option<u32>,
    pub post_window_system_sec: Option<u32>,
    pub post_window_deliberate_sec: Option<u32>,
    pub max_window_sec: Option<u32>,
    pub min_after_sec: Option<u32>,
    pub system_min_interval_sec: Option<u32>,
    pub byte_cap_mb: Option<u32>,
}
```

Remove the `dashcam_` prefix from field names since they're already scoped
under `[dashcam]`:

```toml
# Before:
[recording]
dashcam_enabled = true
dashcam_pre_window_system_sec = 30

# After:
[dashcam]
enabled = true
pre_window_system_sec = 30
```

The mapping function `toml_to_session_config` updates to read from
`toml.dashcam` instead of `toml.recording`.

**Acceptance Criteria:**
- [ ] TOML section is `[dashcam]`, not `[recording]`
- [ ] Field names drop `dashcam_` prefix within the section
- [ ] All existing TOML config tests updated and passing
- [ ] `SessionConfig` field names stay unchanged (they're used elsewhere)

---

### Unit 10: Simplify GDScript Runtime

**File**: `addons/spectator/runtime.gd`

Remove explicit recording UI, keybinds, and signal handlers.

**Remove:**
- `_recording_dot` (red recording indicator ColorRect)
- `_record_keycode` / F12 keybind handling
- `_toggle_recording()` function
- `_set_recording_indicator()` function
- `_on_recording_started()` signal handler
- `_on_recording_stopped()` signal handler
- Recording status push in `_push_status_to_editor()` (the `spectator:recording` message)
- Signal connections: `recorder.recording_started.connect(...)`, `recorder.recording_stopped.connect(...)`

**Keep:**
- `_marker_keycode` / F9 keybind → dashcam marker/flush
- `_dashcam_label` overlay
- `_marker_btn` (in-game marker button)
- `_on_marker_added()` signal handler
- `_on_dashcam_clip_saved()` signal handler
- `_on_dashcam_clip_started()` signal handler
- `_pause_keycode` / F11 keybind
- `_update_dashcam_label()`
- Toast system

**Simplify `_drop_marker()`:**
```gdscript
func _drop_marker() -> void:
    if not recorder:
        return
    if recorder.is_dashcam_active():
        var clip_id: String = recorder.flush_dashcam_clip("human")
        if not clip_id.is_empty():
            _show_toast("Dashcam clip saved")
    else:
        recorder.add_marker("human", "")
```

**Simplify `_on_debugger_command`:**
Remove `start_recording` and `stop_recording` commands:
```gdscript
func _on_debugger_command(message: String, data: Array) -> bool:
    if message != "spectator:command" or data.is_empty():
        return false
    match data[0]:
        "add_marker": _drop_marker()
    return true
```

**Simplify `_push_status_to_editor`:**
Remove `spectator:recording` message entirely.

**Acceptance Criteria:**
- [ ] No `_recording_dot`, no `_toggle_recording()`, no `_set_recording_indicator()`
- [ ] No F12 keybind handling
- [ ] No `recording_started`/`recording_stopped` signal connections
- [ ] No `spectator:recording` debugger message
- [ ] F9 marker key still works (routes to dashcam)
- [ ] Dashcam label overlay still works

---

### Unit 11: Simplify Editor Dock

**File**: `addons/spectator/dock.gd` + `addons/spectator/dock.tscn`

Remove recording controls from the dock.

**Remove from dock.gd:**
- `record_btn`, `stop_btn`, `marker_btn` references
- `recording_stats` label
- `_recording_active` state variable
- `receive_recording()` method
- `_on_record_pressed()`, `_on_stop_pressed()`, `_on_marker_pressed()` handlers
- `_update_recording_controls()` method
- `_format_elapsed()` helper

**Remove from dock.tscn:**
- `RecordBtn`, `StopBtn`, `MarkerBtn` nodes
- `RecordingStats` label node

**Keep:**
- Status section (status dot, status label, port, tracking, watches, frame)
- Activity log section (list, scroll, collapse)
- All `receive_status` and `receive_activity` methods

**Acceptance Criteria:**
- [ ] No record/stop/marker buttons in dock
- [ ] No recording stats display
- [ ] `receive_recording` method removed
- [ ] Status and activity sections still work

---

### Unit 12: Simplify Editor Debugger Plugin

**File**: `addons/spectator/debugger_plugin.gd`

Remove the `spectator:recording` message handler.

**Remove:**
- The `"spectator:recording"` case in the capture handler
- The `send_command` method signature stays (used for `add_marker` still? — check
  if dock still sends commands). Actually, with record/stop/marker buttons removed
  from dock, the dock no longer sends commands. But keep `send_command` in case
  it's needed for future dock features, or remove it if the dock has no buttons.

**Decision:** Remove `send_command` if no dock buttons remain that use it.
The dock only displays status and activity now — it's read-only.

**Updated debugger_plugin.gd:**
```gdscript
@tool
extends EditorDebuggerPlugin

var _dock

func _has_capture(prefix: String) -> bool:
    return prefix == "spectator"

func _capture(message: StringName, data: Array, _session_id: int) -> bool:
    if not _dock:
        return false
    match message:
        &"spectator:status":
            _dock.receive_status(data[0], data[1], data[2], data[3], data[4], data[5])
        &"spectator:activity":
            _dock.receive_activity(data[0], data[1], data[2], data[3])
    return true
```

**Acceptance Criteria:**
- [ ] No `spectator:recording` handler
- [ ] `send_command` removed (or kept with comment if needed later)
- [ ] Status and activity messages still relay correctly

---

### Unit 13: Simplify Plugin Settings

**File**: `addons/spectator/plugin.gd`

Remove recording-specific project settings.

**Remove settings:**
```gdscript
# DELETE:
_add_setting("spectator/recording/storage_path", ...)
_add_setting("spectator/recording/max_frames", ...)
_add_setting("spectator/recording/capture_interval", ...)
_add_setting("spectator/display/show_recording_indicator", ...)
_add_setting("spectator/shortcuts/record_key", ...)
```

**Keep settings:**
```gdscript
# KEEP:
_add_setting("spectator/shortcuts/marker_key", ...)  # F9 for dashcam
_add_setting("spectator/shortcuts/pause_key", ...)    # F11 for pause
_add_setting("spectator/display/show_agent_notifications", ...)
# All connection and tracking settings
```

**Acceptance Criteria:**
- [ ] No `spectator/recording/*` settings
- [ ] No `spectator/display/show_recording_indicator` setting
- [ ] No `spectator/shortcuts/record_key` setting
- [ ] Remaining settings still registered correctly

---

### Unit 14: Update Tests — Wire Tests

**File**: `tests/wire-tests/src/test_recording.rs`

Rewrite or remove explicit recording tests. Keep/add dashcam tests.

**Remove tests:**
```rust
// DELETE:
fn recording_start_status_stop_lifecycle()
fn recording_status_when_not_recording()
fn recording_stop_when_not_recording_returns_error_or_ok()
fn recording_add_marker_while_recording()  // rewrite for dashcam
```

**Add/update tests:**
```rust
#[test]
#[ignore] // requires Godot
fn dashcam_status_returns_state() {
    // Connect, query dashcam_status, verify state="buffering"
}

#[test]
#[ignore]
fn dashcam_marker_triggers_clip() {
    // Connect, send recording_marker, verify dashcam_triggered=true
}

#[test]
#[ignore]
fn dashcam_flush_saves_clip() {
    // Connect, wait for buffer to fill, flush, verify recording_id returned
}
```

Consider renaming file to `test_dashcam.rs`.

**Acceptance Criteria:**
- [ ] No explicit recording lifecycle tests
- [ ] Dashcam-specific wire tests exist
- [ ] All wire tests pass

---

### Unit 15: Update Tests — GDScript Tests

**Files:**
- `tests/godot-project/tests/test_keybinds.gd`
- `tests/godot-project/tests/test_dock.gd`
- `tests/godot-project/tests/test_extension.gd`
- `tests/godot-project/tests/test_runtime_wiring.gd`
- `tests/godot-project/tests/test_signals.gd`

**test_keybinds.gd — Remove/update:**
- `test_f12_toggles_recording()` → DELETE
- `test_f9_adds_marker_when_recording()` → REWRITE as `test_f9_triggers_dashcam_clip()`
- `test_default_keycode_fields_match_expected()` → UPDATE (no `_record_keycode`)

**test_dock.gd — Remove/update:**
- `test_dock_receive_recording_updates_controls()` → DELETE
- `test_dock_receive_recording_stopped_updates_controls()` → DELETE
- `test_dock_record_button_sends_command()` → DELETE
- `test_dock_stop_button_sends_command()` → DELETE
- `test_dock_marker_button_sends_command()` → DELETE
- `test_dock_has_no_recorder_field()` → KEEP (still true)

**test_extension.gd — Update:**
- `test_recorder_has_signals()` → UPDATE signal list (no `recording_started`/`recording_stopped`)

**test_runtime_wiring.gd — Remove/update:**
- `test_debugger_command_start_recording()` → DELETE
- `test_debugger_command_stop_recording()` → DELETE
- `test_debugger_command_add_marker()` → KEEP (still works)

**test_signals.gd — Remove/update:**
- `test_recorder_emits_recording_started()` → DELETE
- `test_recorder_emits_recording_stopped()` → DELETE
- `test_recorder_emits_marker_added()` → KEEP

**Acceptance Criteria:**
- [ ] No tests reference `start_recording`, `stop_recording`, `is_recording`
- [ ] No tests reference `recording_started`, `recording_stopped` signals
- [ ] No tests reference record button or recording stats in dock
- [ ] Remaining tests pass

---

### Unit 16: Update Server Tests — Scenarios and E2E Journeys

**Files:**
- `crates/spectator-server/tests/scenarios.rs`
- `crates/spectator-server/tests/e2e_journeys.rs`

**scenarios.rs:**
- `test_recording_lifecycle_ids_consistent()` → DELETE
- `test_dashcam_independent_of_explicit_recording()` → SIMPLIFY (no explicit recording to test independence from)
- `test_dashcam_clips_in_recording_list()` → KEEP, rename to `test_dashcam_clips_in_list()`

**e2e_journeys.rs:**
- `journey_recording_lifecycle()` → DELETE entirely
- `journey_dashcam_agent_workflow()` → KEEP, update to use `dashcam` tool name instead of `recording`

**Acceptance Criteria:**
- [ ] No explicit recording journey tests
- [ ] Dashcam journey uses `dashcam` MCP tool name
- [ ] `cargo test --workspace` passes

---

### Unit 17: Update Protocol Crate

**File**: `crates/spectator-protocol/src/recording.rs`

Keep `FrameEntityData` (dashcam still uses it for MessagePack frame serialization).
No changes needed to this file.

**Acceptance Criteria:**
- [ ] `FrameEntityData` still exists and compiles
- [ ] Roundtrip tests still pass

---

### Unit 18: Clean Up Docs and References

**Files:**
- `docs/SPEC.md` — Update recording system section, tool table, addon query methods
- `docs/VISION.md` — Update "Record and analyze" bullet point
- Delete old design docs: `docs/design/M7-RECORDING-CAPTURE.md`, `docs/design/M8-RECORDING-ANALYSIS.md`
- Update `CLAUDE.md` if it references 9 MCP tools (now 8: recording→dashcam)

**Acceptance Criteria:**
- [ ] SPEC.md reflects dashcam-only design
- [ ] No references to "start recording", "stop recording" in docs
- [ ] M7 and M8 design docs deleted
- [ ] Tool count accurate

---

## Implementation Order

1. **Unit 5**: Delete `recording_analysis.rs` (removes the biggest chunk, unblocks dep removal)
2. **Unit 6**: Remove `rusqlite`/`rmp-serde` from server Cargo.toml + `recording_storage_path` from state
3. **Unit 4**: Rename MCP tool, create `dashcam.rs`, slim params
4. **Unit 7**: Update MCP tool router (`mod.rs`)
5. **Unit 8**: Update activity summaries
6. **Unit 1**: Strip explicit recording from `SpectatorRecorder` (the largest unit)
7. **Unit 2**: Simplify `recording_handler.rs`
8. **Unit 3**: Verify TCP routing (minimal change)
9. **Unit 9**: Update TOML config
10. **Unit 10**: Simplify `runtime.gd`
11. **Unit 11**: Simplify dock UI
12. **Unit 12**: Simplify debugger plugin
13. **Unit 13**: Simplify plugin settings
14. **Unit 14**: Update wire tests
15. **Unit 15**: Update GDScript tests
16. **Unit 16**: Update server scenario/E2E tests
17. **Unit 17**: Verify protocol crate (no change expected)
18. **Unit 18**: Clean up docs

**Rationale:** Start server-side (units 5-9) because they're self-contained
deletions with clear compilation feedback. Then GDExtension (units 1-3) which
is the most complex change. Then GDScript addon (units 10-13). Tests last
(units 14-16) since they need the implementation to be stable. Docs last.

## Testing

### Unit Tests (Rust)
- `crates/spectator-godot/src/recorder.rs` — all dashcam unit tests must pass
- `crates/spectator-server/src/mcp/dashcam.rs` — param deserialization tests
- `crates/spectator-server/src/config.rs` — TOML parsing tests with `[dashcam]`
- `crates/spectator-server/src/activity.rs` — summary tests if any

### Integration Tests
- `tests/wire-tests/` — dashcam status, marker trigger, flush
- `crates/spectator-server/tests/scenarios.rs` — dashcam clip listing
- `crates/spectator-server/tests/e2e_journeys.rs` — dashcam agent workflow

### GDScript Tests
- `test_keybinds.gd` — F9 marker, F11 pause (no F12)
- `test_dock.gd` — status/activity display (no recording controls)
- `test_extension.gd` — recorder instantiation, signal list
- `test_runtime_wiring.gd` — component creation, debugger commands
- `test_signals.gd` — marker_added signal

### Full Verification
```bash
# Build
cargo build --workspace
cargo clippy --workspace
cargo fmt --check

# Deploy and test
spectator-deploy ~/dev/spectator/tests/godot-project
cargo test --workspace

# Verify Godot loads cleanly
godot --headless --quit --path ~/dev/spectator/tests/godot-project 2>&1
```

## Verification Checklist

- [ ] `cargo build --workspace` succeeds
- [ ] `cargo clippy --workspace` clean
- [ ] `cargo test --workspace` all pass
- [ ] No `rusqlite` or `rmp-serde` in `spectator-server/Cargo.toml`
- [ ] No `recording_analysis.rs` file exists
- [ ] MCP schema shows `dashcam` tool, not `recording`
- [ ] `spectator-server` binary starts without errors
- [ ] GDExtension loads in Godot without errors
- [ ] Dashcam ring buffer works (status shows "buffering")
- [ ] Agent `add_marker` triggers dashcam clip
- [ ] `flush` saves clip, appears in `list`
- [ ] F9 in-game drops marker / flushes dashcam
- [ ] Dock shows status and activity (no recording buttons)
- [ ] `spectator.toml` `[dashcam]` section parsed correctly
