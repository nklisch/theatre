# Design: Dashcam-Only Refactor

## Overview

The explicit recording feature (manual start/stop with a persistent SQLite
writer) duplicates what the dashcam already does more elegantly. Both produce
identical SQLite files. The only difference is lifecycle: explicit recording
requires the agent/human to predict when something interesting will happen;
dashcam captures retroactively around the moment of interest.

**The refactor:** Remove the explicit recording lifecycle (start, stop, status)
and make the dashcam ring buffer the sole capture mechanism. The analysis tools
(snapshot_at, query_range, diff_frames, find_event) stay — they operate on
saved clips, and dashcam clips are structurally identical to explicit recordings.

**What disappears:**
- `recording(action: "start")` / `"stop"` / `"status"` MCP actions
- `CaptureConfig` parameter struct (capture_interval, max_frames)
- Explicit recording state in SpectatorRecorder (recording flag, frame_buffer,
  event_buffer, marker_buffer, periodic flush_to_db, db connection)
- `recording_started` / `recording_stopped` signals
- F12 keybind (toggle recording)
- Record/Stop buttons in editor dock
- Recording indicator (red dot overlay)
- `spectator/recording/*` project settings (storage_path, max_frames,
  capture_interval, show_recording_indicator, record_key)
- `spectator:recording` debugger message
- CapturedEvent, CapturedMarker buffer types

**What stays (unchanged):**
- Dashcam ring buffer, state machine, clip saving
- All dashcam config (TOML, SessionConfig, apply_dashcam_config)
- `dashcam_status`, `flush_dashcam` actions
- `add_marker` action (triggers dashcam clip)
- `list`, `delete`, `markers` actions (query saved clips)
- `snapshot_at`, `query_range`, `diff_frames`, `find_event` (analysis on clips)
- `recording_analysis.rs` (entire module stays)
- `rusqlite` + `rmp-serde` in spectator-server (needed for analysis)
- `recording_storage_path` in SessionState (needed by analysis)
- SQLite schema, FrameEntityData, MessagePack serialization
- `dashcam_clip_saved`, `dashcam_clip_started`, `marker_added` signals
- F9 keybind (marker / flush dashcam)
- Dashcam label overlay
- In-game marker button

**What gets renamed:**
- MCP tool `recording` → `recording` (keep name — clips are still "recordings",
  the tool still manages recordings). The name is fine; what changes is the
  action set.

## Implementation Units

### Unit 1: Remove Explicit Recording Actions from MCP Tool

**File**: `crates/spectator-server/src/mcp/recording.rs`

Remove `start`, `stop`, `status` actions and `CaptureConfig`. The `recording_name`
param is also removed (dashcam auto-names clips).

**Current RecordingParams (remove marked fields):**
```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordingParams {
    pub action: String,
    pub recording_name: Option<String>,    // DELETE — dashcam auto-names
    pub capture: Option<CaptureConfig>,     // DELETE — no explicit recording
    pub recording_id: Option<String>,       // KEEP
    pub marker_label: Option<String>,       // KEEP
    pub marker_frame: Option<u64>,          // KEEP
    pub token_budget: Option<u32>,          // KEEP
    // M8 analysis fields — all KEEP
    pub at_frame: Option<u64>,
    pub at_time_ms: Option<u64>,
    pub detail: Option<String>,
    pub from_frame: Option<u64>,
    pub to_frame: Option<u64>,
    pub node: Option<String>,
    pub condition: Option<serde_json::Value>,
    pub event_type: Option<String>,
    pub event_filter: Option<String>,
    pub frame_a: Option<u64>,
    pub frame_b: Option<u64>,
}
```

**New RecordingParams:**
```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordingParams {
    /// Action to perform.
    /// "add_marker" — drop a marker; triggers dashcam clip capture.
    /// "flush" — force-save the dashcam buffer as a clip immediately.
    /// "status" — return dashcam buffer state and config.
    /// "list" — list saved clips.
    /// "delete" — remove a clip by recording_id.
    /// "markers" — list markers in a saved clip.
    /// "snapshot_at" — reconstruct spatial state at a frame in a saved clip.
    /// "query_range" — search a clip's frames for spatial conditions.
    /// "diff_frames" — compare two frames in a clip.
    /// "find_event" — search events in a clip.
    pub action: String,

    /// Clip to query (markers, delete, analysis). Uses most recent if omitted.
    pub recording_id: Option<String>,

    /// Marker label (add_marker, flush).
    pub marker_label: Option<String>,

    /// Frame to attach marker to (add_marker). Defaults to current frame.
    pub marker_frame: Option<u64>,

    /// Soft token budget for the response.
    pub token_budget: Option<u32>,

    // --- Analysis fields (operate on saved clips) ---

    /// Frame number for snapshot_at. Mutually exclusive with at_time_ms.
    pub at_frame: Option<u64>,

    /// Timestamp (ms) for snapshot_at. Finds nearest frame.
    pub at_time_ms: Option<u64>,

    /// Detail level for snapshot_at: "summary", "standard", "full".
    pub detail: Option<String>,

    /// Start of frame range for query_range and find_event.
    pub from_frame: Option<u64>,

    /// End of frame range for query_range and find_event.
    pub to_frame: Option<u64>,

    /// Node path for query_range.
    pub node: Option<String>,

    /// Spatial/temporal condition for query_range.
    pub condition: Option<serde_json::Value>,

    /// Event type for find_event.
    pub event_type: Option<String>,

    /// Event filter string for find_event (substring match on event data).
    pub event_filter: Option<String>,

    /// Frame A for diff_frames.
    pub frame_a: Option<u64>,

    /// Frame B for diff_frames.
    pub frame_b: Option<u64>,
}
```

**Delete**: `CaptureConfig` struct entirely.

**Update handler dispatch:**
```rust
pub async fn handle_recording(
    params: RecordingParams,
    state: &Arc<Mutex<SessionState>>,
) -> Result<String, McpError> {
    let config = crate::tcp::get_config(state).await;
    let hard_cap = config.token_hard_cap;
    let budget_limit = resolve_budget(params.token_budget, 1500, hard_cap);

    match params.action.as_str() {
        // Dashcam lifecycle
        "add_marker" => handle_add_marker(&params, state, budget_limit, hard_cap).await,
        "flush" => handle_flush(&params, state, budget_limit, hard_cap).await,
        "status" => query_and_finalize(state, "dashcam_status", json!({}), budget_limit, hard_cap).await,

        // Clip management
        "list" => query_and_finalize(state, "recording_list", json!({}), budget_limit, hard_cap).await,
        "delete" => handle_delete(&params, state, budget_limit, hard_cap).await,
        "markers" => handle_markers(&params, state, budget_limit, hard_cap).await,

        // Analysis (on saved clips)
        "snapshot_at" => handle_snapshot_at(&params, state, budget_limit, hard_cap).await,
        "query_range" => handle_query_range(&params, state, budget_limit, hard_cap).await,
        "diff_frames" => handle_diff_frames(&params, state, budget_limit, hard_cap).await,
        "find_event" => handle_find_event(&params, state, budget_limit, hard_cap).await,

        other => Err(McpError::invalid_params(
            format!(
                "Unknown recording action: '{other}'. Valid: add_marker, flush, status, \
                 list, delete, markers, snapshot_at, query_range, diff_frames, find_event"
            ),
            None,
        )),
    }
}
```

**Rename `handle_flush_dashcam` → `handle_flush`** (it was already calling
`dashcam_flush`). The `"flush_dashcam"` action string becomes just `"flush"`.

**Delete functions:** `handle_start` (was building `recording_start` query).

**Update tool description in `mcp/mod.rs`:**
```rust
#[tool(description = "Manage the always-on dashcam and analyze saved clips. \
    Dashcam: 'add_marker' (drop marker, triggers clip capture), \
    'flush' (force-save buffer as clip), 'status' (buffer state and config). \
    Clips: 'list' (saved clips), 'delete' (remove by recording_id), \
    'markers' (list markers in a clip). \
    Analysis: 'snapshot_at' (spatial state at a frame), 'query_range' \
    (search frames for conditions), 'diff_frames' (compare two frames), \
    'find_event' (search events). \
    Analysis defaults to most recent clip if recording_id is omitted.")]
pub async fn recording(
    &self,
    Parameters(params): Parameters<recording::RecordingParams>,
) -> Result<String, McpError> { ... }
```

**Update tests:** Remove `recording_params_deserializes` (tests `start` action
with `capture` field). Remove `recording_params_minimal_start`. Keep all analysis
param tests. Add `recording_params_flush` test.

**Acceptance Criteria:**
- [ ] No `start`, `stop` actions in dispatch
- [ ] No `CaptureConfig` struct
- [ ] No `recording_name` param
- [ ] `flush` action works (calls `dashcam_flush` TCP method)
- [ ] `status` action works (calls `dashcam_status` TCP method)
- [ ] All 4 analysis actions still work unchanged
- [ ] Tool description updated
- [ ] Param deserialization tests updated

---

### Unit 2: Update Activity Summaries

**File**: `crates/spectator-server/src/activity.rs`

Remove summaries for `start`, `stop`, `status` (old meanings). Rename the
`"status"` summary to reflect dashcam. Replace `"flush_dashcam"` with `"flush"`.

```rust
pub fn recording_summary(params: &RecordingParams) -> String {
    match params.action.as_str() {
        "add_marker" => {
            let label = params.marker_label.as_deref().unwrap_or("(no label)");
            format!("Marker: {label}")
        }
        "flush" => {
            let label = params.marker_label.as_deref().unwrap_or("agent flush");
            format!("Flushed dashcam: {label}")
        }
        "status" => "Dashcam status".into(),
        "list" => "Listing clips".into(),
        "delete" => {
            let id = params.recording_id.as_deref().unwrap_or("?");
            format!("Deleted clip {id}")
        }
        "markers" => {
            let id = params.recording_id.as_deref().unwrap_or("current");
            format!("Listing markers for {id}")
        }
        // Analysis summaries — keep unchanged
        "snapshot_at" => { /* existing code */ }
        "query_range" => { /* existing code */ }
        "diff_frames" => { /* existing code */ }
        "find_event" => { /* existing code */ }
        other => format!("Recording: {other}"),
    }
}
```

**Acceptance Criteria:**
- [ ] No `"start"` / `"stop"` match arms
- [ ] `"status"` says "Dashcam status" not "Checking recording status"
- [ ] `"flush"` replaces `"flush_dashcam"` / `"dashcam_status"`
- [ ] Analysis summaries unchanged

---

### Unit 3: Remove Explicit Recording from TCP Handler

**File**: `crates/spectator-godot/src/recording_handler.rs`

Remove `recording_start`, `recording_stop`, `recording_status` TCP method handlers.

**Updated dispatch:**
```rust
pub fn handle_recording_query(
    recorder: &mut Gd<SpectatorRecorder>,
    method: &str,
    params: &Value,
) -> Result<Value, String> {
    match method {
        // Clip management (work on saved SQLite files)
        "recording_list" => handle_list(recorder, params),
        "recording_delete" => handle_delete(recorder, params),
        "recording_marker" => handle_marker(recorder, params),
        "recording_markers" => handle_markers(recorder, params),
        "recording_resolve_path" => handle_resolve_path(recorder, params),
        // Dashcam control
        "dashcam_status" => handle_dashcam_status(recorder, params),
        "dashcam_flush" => handle_dashcam_flush(recorder, params),
        "dashcam_config" => handle_dashcam_config(recorder, params),
        _ => Err(format!("Unknown recording method: {method}")),
    }
}
```

**Simplify `handle_marker`:**
Remove the `is_recording` branch. Marker always routes to dashcam:
```rust
fn handle_marker(recorder: &mut Gd<SpectatorRecorder>, params: &Value) -> Result<Value, String> {
    let source = params["source"].as_str().unwrap_or("agent");
    let label = params["label"].as_str().unwrap_or("");

    // Always add_marker — routes to dashcam internally
    recorder.bind_mut().add_marker(source.into(), label.into());

    let dashcam_active = recorder.bind().is_dashcam_active();
    Ok(json!({
        "ok": true,
        "frame": current_physics_frame_via_engine(),
        "source": source,
        "label": label,
        "dashcam_triggered": dashcam_active,
    }))
}
```

**Delete functions:** `handle_start`, `handle_stop`, `handle_status` (the old
explicit-recording-specific handlers).

**Acceptance Criteria:**
- [ ] No `recording_start`, `recording_stop`, `recording_status` in dispatch
- [ ] `handle_marker` has no `is_recording` branch
- [ ] Remaining handlers compile and work

---

### Unit 4: Strip Explicit Recording from SpectatorRecorder

**File**: `crates/spectator-godot/src/recorder.rs`

This is the largest unit. Remove all explicit recording state, methods, and
signals. Keep dashcam and shared infrastructure.

**Remove from struct fields:**
```rust
// DELETE:
recording: bool,
recording_id: String,
recording_name: String,
started_at_frame: u64,
started_at_ms: u64,
frames_captured: u32,
frame_counter: u32,        // KEEP but rename — dashcam also uses interval counting
capture_interval: u32,      // DELETE — dashcam has its own in DashcamConfig
max_frames: u32,
frame_buffer: Vec<CapturedFrame>,
event_buffer: Vec<CapturedEvent>,
marker_buffer: Vec<CapturedMarker>,
flush_counter: u32,
db: Option<Connection>,
storage_path: String,
```

**Delete types:**
```rust
// DELETE:
struct CapturedEvent { frame, event_type, node_path, data }
struct CapturedMarker { frame, timestamp_ms, source, label }
// KEEP:
struct CapturedFrame { frame, timestamp_ms, data }  // used by dashcam ring buffer
```

**Delete signals:**
```rust
// DELETE:
#[signal] fn recording_started(recording_id: GString, name: GString);
#[signal] fn recording_stopped(recording_id: GString, frames: u32);
// KEEP:
#[signal] fn marker_added(frame: u64, source: GString, label: GString);
#[signal] fn dashcam_clip_saved(recording_id: GString, tier: GString, frames: u32);
#[signal] fn dashcam_clip_started(trigger_frame: u64, tier: GString);
```

**Delete exported methods:**
```rust
// DELETE all of these:
fn is_recording() -> bool
fn get_recording_id() -> GString
fn get_recording_name() -> GString
fn get_frames_captured() -> u32
fn get_elapsed_ms() -> u64
fn get_buffer_size_kb() -> u32
fn get_recording_status() -> Dictionary
fn start_recording(name, storage_path, capture_interval, max_frames) -> GString
fn stop_recording() -> Dictionary
```

**Simplify `add_marker`:**
Currently has two branches (explicit recording vs dashcam). Remove the explicit
branch — markers always route to dashcam:
```rust
#[func]
pub fn add_marker(&mut self, source: GString, label: GString) {
    let frame = current_physics_frame();
    let timestamp_ms = current_time_ms();

    self.on_dashcam_marker(
        &source.to_string(), &label.to_string(), frame, timestamp_ms,
    );

    self.base_mut().emit_signal(
        "marker_added",
        &[frame.to_variant(), source.to_variant(), label.to_variant()],
    );
}
```

**Simplify `physics_process`:**
Remove the explicit recording capture path entirely. Only dashcam needs frames:
```rust
fn physics_process(&mut self, _delta: f64) {
    self.dashcam_check_force_close();

    if matches!(self.dashcam_state, DashcamState::Disabled) {
        return;
    }

    // Interval counting for dashcam capture
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

**Delete internal methods:**
```rust
// DELETE:
fn flush_to_db(&mut self)  // only used by explicit recording periodic flush
// KEEP:
fn create_db(&mut self, path: &str)  // used by flush_dashcam_clip_internal
fn do_capture(&mut self)  // used by dashcam
// KEEP all dashcam internals unchanged
```

Wait — `create_db` is only called from `start_recording`. The dashcam clip
flush (`flush_dashcam_clip_internal`) creates its own SQLite inline. So
`create_db` can be deleted too.

**Update `init`:** Remove all explicit recording field initializations.

**Update unit tests:** Remove tests that exercise `start_recording`/`stop_recording`.
Keep all dashcam tests (ring buffer, config, merge, force-close, etc.).

**Acceptance Criteria:**
- [ ] No `recording: bool` or explicit recording state in struct
- [ ] No `start_recording` / `stop_recording` / `is_recording` methods
- [ ] No `recording_started` / `recording_stopped` signals
- [ ] `add_marker` only routes to dashcam
- [ ] `physics_process` only captures for dashcam
- [ ] `CapturedEvent` and `CapturedMarker` types deleted
- [ ] `flush_to_db` and `create_db` deleted
- [ ] All dashcam unit tests pass
- [ ] `list_recordings`, `delete_recording`, `get_recording_markers` unchanged

---

### Unit 5: Simplify GDScript Runtime

**File**: `addons/spectator/runtime.gd`

**Remove:**
- `_recording_dot: ColorRect` field and its creation in `_setup_overlay()`
- `_record_keycode: int` field and F12 keybind handling in `_shortcut_input()`
- `_toggle_recording()` function
- `_set_recording_indicator()` function
- `_on_recording_started()` signal handler
- `_on_recording_stopped()` signal handler
- Signal connections: `recorder.recording_started.connect(...)`,
  `recorder.recording_stopped.connect(...)`
- `spectator:recording` debugger message from `_push_status_to_editor()`
- `"start_recording"` and `"stop_recording"` cases in `_on_debugger_command()`
- `recorder.stop_recording()` call in `_exit_tree()` (recording no longer exists)

**Keep:**
- `_marker_keycode` / F9 keybind handling
- `_dashcam_label` overlay
- `_marker_btn` (in-game marker button)
- `_on_marker_added()` — keep but simplify: no recording dot color flash
  (recording dot is gone), just show toast
- `_on_dashcam_clip_saved()` / `_on_dashcam_clip_started()` handlers
- `_pause_keycode` / F11 keybind
- `_update_dashcam_label()`
- Toast system, overlay, pause label

**Simplify `_drop_marker()`:**
Currently checks `is_recording()` first. Remove that branch — always route to
dashcam:
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
```gdscript
func _on_debugger_command(message: String, data: Array) -> bool:
    if message != "spectator:command" or data.is_empty():
        return false
    match data[0]:
        "add_marker": _drop_marker()
    return true
```

**Simplify `_on_marker_added`:**
Remove the recording dot color flash (no recording dot anymore):
```gdscript
func _on_marker_added(_frame: int, source: String, label: String) -> void:
    var text := "Marker: %s" % label if not label.is_empty() else "Marker added"
    if source != "human":
        text = "[%s] %s" % [source, text]
    _show_toast(text)
```

**Simplify `_exit_tree`:**
```gdscript
func _exit_tree() -> void:
    if tcp_server:
        tcp_server.stop()
```

**Acceptance Criteria:**
- [ ] No `_recording_dot`, `_toggle_recording()`, `_set_recording_indicator()`
- [ ] No F12 keybind handling
- [ ] No `recording_started`/`recording_stopped` signal connections
- [ ] No `spectator:recording` debugger message
- [ ] F9 still works for dashcam marker/flush
- [ ] Dashcam label still works
- [ ] In-game marker button still works

---

### Unit 6: Simplify Editor Dock

**Files**: `addons/spectator/dock.gd` + `addons/spectator/dock.tscn`

**Remove from dock.gd:**
- `record_btn`, `stop_btn`, `marker_btn` `@onready` vars
- `recording_stats` `@onready` var
- `_recording_active` state variable
- `receive_recording()` method
- `_on_record_pressed()`, `_on_stop_pressed()`, `_on_marker_pressed()` handlers
- `_update_recording_controls()` method
- `_format_elapsed()` helper
- Button connections in `_ready()`

**Remove from dock.tscn:**
- `RecordBtn`, `StopBtn`, `MarkerBtn` nodes
- `RecordingStats` label node

**Keep unchanged:**
- Status section (dot, label, port, tracking, watches, frame)
- Activity log section (list, scroll, collapse button)
- `receive_status()` and `receive_activity()` methods

**Acceptance Criteria:**
- [ ] No record/stop/marker buttons in dock
- [ ] No recording stats
- [ ] No `receive_recording()` method
- [ ] Status and activity sections work

---

### Unit 7: Simplify Debugger Plugin

**File**: `addons/spectator/debugger_plugin.gd`

**Remove:**
- `"spectator:recording"` case in `_capture()` — no longer pushed by runtime
- `send_command()` method — dock no longer sends start/stop/marker commands
  (dock is now read-only)

**Resulting file:**
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

**Update dock.gd:** Remove `_debugger_plugin` field (nothing to send anymore).
**Update plugin.gd:** Remove `_dock._debugger_plugin = _debugger_plugin` wiring.

**Acceptance Criteria:**
- [ ] No `spectator:recording` handler
- [ ] No `send_command()` method
- [ ] Status and activity relay still works

---

### Unit 8: Simplify Plugin Settings

**File**: `addons/spectator/plugin.gd`

**Remove settings:**
```gdscript
# DELETE:
_add_setting("spectator/recording/storage_path", ...)
_add_setting("spectator/recording/max_frames", ...)
_add_setting("spectator/recording/capture_interval", ...)
_add_setting("spectator/display/show_recording_indicator", ...)
_add_setting("spectator/shortcuts/record_key", ...)
```

**Keep:**
```gdscript
_add_setting("spectator/shortcuts/marker_key", ...)   # F9 for dashcam
_add_setting("spectator/shortcuts/pause_key", ...)     # F11 for pause
_add_setting("spectator/display/show_agent_notifications", ...)
# All connection and tracking settings
```

**Acceptance Criteria:**
- [ ] No `spectator/recording/*` settings
- [ ] No `spectator/display/show_recording_indicator` setting
- [ ] No `spectator/shortcuts/record_key` setting
- [ ] Remaining settings registered correctly

---

### Unit 9: Rename TOML Config Section

**File**: `crates/spectator-server/src/config.rs`

Rename `RecordingConfig` → `DashcamTomlConfig` and the TOML section from
`[recording]` to `[dashcam]`. Drop the `dashcam_` prefix from field names
since they're already scoped under `[dashcam]`.

```rust
#[derive(Debug, Default, Deserialize)]
pub struct SpectatorToml {
    pub connection: Option<ConnectionConfig>,
    pub tracking: Option<TrackingConfig>,
    pub dashcam: Option<DashcamTomlConfig>,  // was: recording: Option<RecordingConfig>
}

#[derive(Debug, Default, Deserialize)]
pub struct DashcamTomlConfig {
    pub enabled: Option<bool>,                      // was: dashcam_enabled
    pub capture_interval: Option<u32>,              // was: dashcam_capture_interval
    pub pre_window_system_sec: Option<u32>,         // was: dashcam_pre_window_system_sec
    pub pre_window_deliberate_sec: Option<u32>,     // etc.
    pub post_window_system_sec: Option<u32>,
    pub post_window_deliberate_sec: Option<u32>,
    pub max_window_sec: Option<u32>,
    pub min_after_sec: Option<u32>,
    pub system_min_interval_sec: Option<u32>,
    pub byte_cap_mb: Option<u32>,
}
```

TOML file changes:
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

Update `toml_to_session_config` to read from `toml.dashcam` instead of
`toml.recording`, mapping shortened field names to the existing
`config.dashcam_*` SessionConfig fields.

Update all TOML config tests.

**Acceptance Criteria:**
- [ ] TOML section is `[dashcam]`
- [ ] Field names drop `dashcam_` prefix within section
- [ ] Mapping to SessionConfig fields still correct
- [ ] All config tests pass

---

### Unit 10: Update Tests — GDScript

**Files:**
- `tests/godot-project/tests/test_keybinds.gd`
- `tests/godot-project/tests/test_dock.gd`
- `tests/godot-project/tests/test_extension.gd`
- `tests/godot-project/tests/test_runtime_wiring.gd`
- `tests/godot-project/tests/test_signals.gd`

**test_keybinds.gd:**
- DELETE `test_f12_toggles_recording()`
- DELETE `test_f9_adds_marker_when_recording()` — rewrite as
  `test_f9_triggers_dashcam_marker()` (verifies `marker_added` signal fires
  via dashcam path)
- UPDATE `test_default_keycode_fields_match_expected()` — remove
  `_record_keycode` check, keep `_marker_keycode` and `_pause_keycode`

**test_dock.gd:**
- DELETE `test_dock_receive_recording_updates_controls()`
- DELETE `test_dock_receive_recording_stopped_updates_controls()`
- DELETE `test_dock_record_button_sends_command()`
- DELETE `test_dock_stop_button_sends_command()`
- DELETE `test_dock_marker_button_sends_command()`
- KEEP `test_dock_has_no_recorder_field()` (still true)
- KEEP all status/activity tests

**test_extension.gd:**
- UPDATE `test_recorder_has_signals()` — remove `recording_started`,
  `recording_stopped` from expected signal list; keep `marker_added`,
  `dashcam_clip_saved`, `dashcam_clip_started`

**test_runtime_wiring.gd:**
- DELETE `test_debugger_command_start_recording()`
- DELETE `test_debugger_command_stop_recording()`
- KEEP `test_debugger_command_add_marker()`

**test_signals.gd:**
- DELETE `test_recorder_emits_recording_started()`
- DELETE `test_recorder_emits_recording_stopped()`
- KEEP `test_recorder_emits_marker_added()`

**Acceptance Criteria:**
- [ ] No tests reference `start_recording`, `stop_recording`, `is_recording`
- [ ] No tests reference `recording_started`, `recording_stopped` signals
- [ ] No tests reference record/stop buttons in dock
- [ ] Remaining tests pass

---

### Unit 11: Update Tests — Wire Tests

**File**: `tests/wire-tests/src/test_recording.rs`

Rename to `test_dashcam.rs` (or keep name, doesn't matter).

**Remove tests:**
- `recording_start_status_stop_lifecycle`
- `recording_status_when_not_recording`
- `recording_stop_when_not_recording_returns_error_or_ok`
- `recording_add_marker_while_recording` — rewrite for dashcam context

**Add/update tests:**
```rust
#[test]
#[ignore] // requires Godot
fn dashcam_status_returns_buffering() {
    // Connect, query dashcam_status, verify state="buffering"
}

#[test]
#[ignore]
fn marker_triggers_dashcam_clip() {
    // Connect, send recording_marker with source="agent",
    // verify dashcam_triggered=true in response
}

#[test]
#[ignore]
fn dashcam_flush_returns_recording_id() {
    // Connect, advance frames so buffer has content,
    // flush, verify recording_id starts with "dash_"
}
```

**Acceptance Criteria:**
- [ ] No explicit recording lifecycle tests
- [ ] Dashcam tests cover status, marker trigger, flush
- [ ] Wire tests pass

---

### Unit 12: Update Tests — Server Scenarios and E2E

**Files:**
- `crates/spectator-server/tests/scenarios.rs`
- `crates/spectator-server/tests/e2e_journeys.rs`

**scenarios.rs:**
- DELETE `test_recording_lifecycle_ids_consistent()`
- DELETE or simplify `test_dashcam_independent_of_explicit_recording()` — no
  explicit recording to be independent from. Could test dashcam runs when
  no other tool is active.
- KEEP `test_dashcam_clips_in_recording_list()` (rename if desired)

**e2e_journeys.rs:**
- DELETE `journey_recording_lifecycle()` entirely
- UPDATE `journey_dashcam_agent_workflow()` — change action strings:
  `"flush_dashcam"` → `"flush"`, `"dashcam_status"` → `"status"`. Keep the
  workflow: advance frames → add_marker → verify clip → flush → verify list.

**Acceptance Criteria:**
- [ ] No explicit recording journey
- [ ] Dashcam journey updated to new action names
- [ ] `cargo test --workspace` passes

---

### Unit 13: Clean Up Docs

**Files to update:**
- `docs/SPEC.md` — Remove recording-specific rows from addon query methods
  table (`recording_start`, `recording_stop`), update tool descriptions,
  update recording system section to describe dashcam-only architecture
- `docs/VISION.md` — Adjust "Record and analyze" bullet: humans don't need
  to explicitly start recording; dashcam captures context automatically

**Files to delete:**
- `docs/design/M7-RECORDING-CAPTURE.md` — explicit recording design, superseded
- `docs/design/M8-RECORDING-ANALYSIS.md` — analysis design still applies but
  the doc references explicit recording heavily; keep and add a note, or delete
  since analysis code stays unchanged

**Update CLAUDE.md memory:** "9 MCP tools" stays accurate (tool is still named
`recording`, just with fewer actions).

**Acceptance Criteria:**
- [ ] SPEC.md reflects dashcam-only capture
- [ ] No docs reference "start recording" / "stop recording" as agent actions
- [ ] M7 design doc deleted

---

## Implementation Order

1. **Unit 1**: MCP tool — remove start/stop/status actions, CaptureConfig (server compiles independently)
2. **Unit 2**: Activity summaries — remove old recording entries
3. **Unit 9**: TOML config — rename section (server-only, independent)
4. **Unit 4**: SpectatorRecorder — strip explicit recording (biggest unit, GDExtension)
5. **Unit 3**: TCP recording handler — remove start/stop/status dispatchers
6. **Unit 5**: runtime.gd — remove recording UI
7. **Unit 6**: dock — remove recording controls
8. **Unit 7**: debugger plugin — remove recording message + send_command
9. **Unit 8**: plugin settings — remove recording settings
10. **Unit 10**: GDScript tests
11. **Unit 11**: Wire tests
12. **Unit 12**: Server scenario/E2E tests
13. **Unit 13**: Docs

**Rationale:** Server-side first (units 1-2, 9) — self-contained, fast
compilation feedback. Then GDExtension (units 4, 3) — the core refactor. Then
GDScript addon (units 5-8) — follows GDExtension changes. Tests after all
implementation (units 10-12). Docs last.

## Testing

### Verification Commands
```bash
# Build everything
cargo build --workspace
cargo clippy --workspace
cargo fmt --check

# Deploy GDExtension to test project
spectator-deploy ~/dev/spectator/tests/godot-project

# Run ALL tests
cargo test --workspace

# Verify Godot loads cleanly
godot --headless --quit --path ~/dev/spectator/tests/godot-project 2>&1
```

### What Must Still Work End-to-End
1. Godot game starts → dashcam begins buffering automatically
2. Agent calls `recording(action: "status")` → sees `state: "buffering"`
3. Agent calls `recording(action: "add_marker", marker_label: "bug here")` → triggers clip
4. Agent calls `recording(action: "flush")` → clip saved immediately
5. Agent calls `recording(action: "list")` → sees saved clip
6. Agent calls `recording(action: "snapshot_at", at_frame: N)` → spatial state reconstructed
7. Agent calls `recording(action: "query_range", ...)` → condition matches found
8. Agent calls `recording(action: "diff_frames", ...)` → frame comparison works
9. Human presses F9 → dashcam clip saved
10. Dock shows status + activity (no recording buttons)

## Verification Checklist
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo clippy --workspace` clean
- [ ] `cargo test --workspace` all pass
- [ ] No `start_recording`, `stop_recording`, `is_recording` in any Rust or GDScript code
- [ ] No `recording_started`, `recording_stopped` signals
- [ ] No `CaptureConfig` struct
- [ ] No record/stop buttons in dock
- [ ] No F12 keybind
- [ ] No `spectator/recording/*` project settings
- [ ] MCP schema shows recording tool with 10 actions (not 14)
- [ ] Analysis tools work on dashcam clips
- [ ] Dashcam auto-starts and buffers
- [ ] `spectator.toml` `[dashcam]` section parsed correctly
