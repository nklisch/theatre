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

**What stays (unchanged in behavior):**
- Dashcam ring buffer, state machine, clip saving
- All dashcam config (TOML, SessionConfig, apply_dashcam_config)
- `add_marker` action (triggers dashcam clip)
- `list`, `delete`, `markers` actions (query saved clips)
- `snapshot_at`, `query_range`, `diff_frames`, `find_event` (analysis on clips)
- `recording_analysis.rs` (entire module stays)
- `rusqlite` + `rmp-serde` in spectator-server (needed for analysis)
- Storage path resolution in SessionState (needed by analysis)
- SQLite schema, FrameEntityData, MessagePack serialization
- `dashcam_clip_saved`, `dashcam_clip_started`, `marker_added` signals
- F9 keybind (marker / flush dashcam)
- Dashcam label overlay, in-game marker button

---

## Naming Decisions

### MCP Tool: `recording` → `clips`

The tool name `recording` implies a manual lifecycle — agents will attempt
`action: "start"` as their first call. The tool is really about **clips**:
saved segments of gameplay that were captured automatically. Renaming to `clips`
makes the action names read naturally:

- `clips(action: "add_marker")` — mark this moment
- `clips(action: "save")` — save the buffer now
- `clips(action: "list")` — what clips exist?
- `clips(action: "snapshot_at")` — show frame N of a clip

### Action Renames

| Old | New | Why |
|---|---|---|
| `"flush"` / `"flush_dashcam"` | `"save"` | "Flush" is an implementation detail. "Save" is what the agent wants. |
| `"status"` (explicit rec.) | removed | No explicit recording to check status of. |
| `"dashcam_status"` | `"status"` | Now the only status — dashcam buffer state. |
| `"start"` / `"stop"` | removed | No manual lifecycle. |

Final action set (10 → 10, different composition):
1. `"add_marker"` — mark the current moment, triggers clip capture
2. `"save"` — force-save the current buffer as a clip immediately
3. `"status"` — dashcam buffer state and config
4. `"list"` — list saved clips
5. `"delete"` — remove a clip by clip_id
6. `"markers"` — list markers in a saved clip
7. `"snapshot_at"` — spatial state at a frame in a clip
8. `"query_range"` — search frames for conditions
9. `"diff_frames"` — compare two frames
10. `"find_event"` — search events

### ID Field: `recording_id` → `clip_id`

Per contract rules (`<resource>_id`), the tool is `clips` so the resource
identifier is `clip_id`. This rename touches:

**Rust server** (recording_analysis.rs, mcp/recording.rs → mcp/clips.rs):
- `RecordingParams.recording_id` → `ClipsParams.clip_id`
- `RecordingSession.recording_id` → `ClipSession.clip_id`
- `RecordingMeta.id` field stays `id` internally, but JSON output becomes `clip_id`
- All `json!({ "recording_id": ... })` → `json!({ "clip_id": ... })`
- `SessionState.recording_storage_path` → `SessionState.clip_storage_path`

**Rust GDExtension** (recorder.rs, recording_handler.rs):
- All `dict.set("recording_id", ...)` → `dict.set("clip_id", ...)`
- All `json!({ "recording_id": ... })` → `json!({ "clip_id": ... })`
- Internal variable names: `recording_id` → `clip_id` where it represents
  a saved clip identifier
- SQLite `recording` table column stays `id` (internal storage, not wire format)

**GDScript** (runtime.gd):
- Signal parameter names stay as-is (GDScript side doesn't parse the string)

**Tests**: All assertions checking for `"recording_id"` → `"clip_id"`

**Contract rules** (`.claude/rules/contracts.md`):
- Update examples: `recording_id` → `clip_id`

**Saved clip ID prefix**: `dash_` → `clip_` (clips are the only kind now,
the `dash_` prefix is meaningless without explicit recordings to contrast).

### Module/File Renames

| Old | New |
|---|---|
| `mcp/recording.rs` | `mcp/clips.rs` |
| `RecordingParams` | `ClipsParams` |
| `recording_analysis.rs` | `clip_analysis.rs` |
| `RecordingSession` | `ClipSession` |
| `RecordingMeta` | `ClipMeta` |
| `recording_summary()` | `clips_summary()` |

### TOML Section: `[recording]` → `[dashcam]`

The TOML section configures the dashcam mechanism (pre-window, post-window,
byte cap). `[dashcam]` is the right name. Drop the `dashcam_` prefix from
field names since they're already scoped:

```toml
[dashcam]
enabled = true
pre_window_system_sec = 30
```

### Tool Description — Write for the Agent

Describe the workflow, not the mechanism:

```
Capture and analyze gameplay clips. Clips are saved automatically when you
mark an interesting moment with 'add_marker'. Use 'save' to capture the
buffer immediately without a marker. Analyze saved clips with 'snapshot_at'
(spatial state at a frame), 'query_range' (search for conditions),
'diff_frames' (compare two frames), 'find_event' (search events). Manage
clips with 'list', 'delete', 'markers'. Check dashcam buffer with 'status'.
Analysis defaults to the most recent clip if clip_id is omitted.
```

### Doc Updates — Guidance

When updating docs (SPEC.md, VISION.md, etc.):

- **Describe the system from the agent's perspective.** "Clips are captured
  automatically around important moments" — not "a ring buffer captures frames
  and transitions through a state machine."
- **Lead with what you can do, not how it works.** The dashcam is invisible
  infrastructure. Agents see: mark → clip appears → analyze clip.
- **Use "clip" consistently.** Not "recording", not "dashcam clip", just "clip".
  The only place "dashcam" appears is in `status` responses and config.
- **The tool description in the MCP schema is the most important doc.** It's
  what every agent reads on every invocation. Keep it under 3 sentences for
  the overview, then list actions concisely.
- **Action docstrings on the params struct** are the second most important —
  agents see these in the JSON schema. Each `///` comment should say what the
  action does in 5-10 words, not explain when to use it.

---

## Implementation Units

### Unit 1: Rename and Slim MCP Tool

**Files**: `crates/spectator-server/src/mcp/recording.rs` → rename to `crates/spectator-server/src/mcp/clips.rs`

Rename the tool, remove explicit recording actions, slim params.

**New params struct:**
```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClipsParams {
    /// Action to perform.
    /// "add_marker" — mark the current moment, triggers clip capture.
    /// "save" — force-save the dashcam buffer as a clip.
    /// "status" — dashcam buffer state and config.
    /// "list" — list saved clips.
    /// "delete" — remove a clip by clip_id.
    /// "markers" — list markers in a saved clip.
    /// "snapshot_at" — spatial state at a frame in a clip.
    /// "query_range" — search frames for spatial conditions.
    /// "diff_frames" — compare two frames in a clip.
    /// "find_event" — search events in a clip.
    pub action: String,

    /// Clip to operate on. Uses most recent if omitted.
    pub clip_id: Option<String>,

    /// Marker label (add_marker, save).
    pub marker_label: Option<String>,

    /// Frame to attach marker to (add_marker). Defaults to current.
    pub marker_frame: Option<u64>,

    /// Soft token budget.
    pub token_budget: Option<u32>,

    // --- Analysis fields ---

    /// Frame number for snapshot_at.
    pub at_frame: Option<u64>,

    /// Timestamp (ms) for snapshot_at. Finds nearest frame.
    pub at_time_ms: Option<u64>,

    /// Detail level for snapshot_at: "summary", "standard", "full".
    pub detail: Option<String>,

    /// Start of frame range for query_range / find_event.
    pub from_frame: Option<u64>,

    /// End of frame range for query_range / find_event.
    pub to_frame: Option<u64>,

    /// Node path for query_range.
    pub node: Option<String>,

    /// Condition object for query_range.
    pub condition: Option<serde_json::Value>,

    /// Event type for find_event.
    pub event_type: Option<String>,

    /// Event filter for find_event (substring match).
    pub event_filter: Option<String>,

    /// Frame A for diff_frames.
    pub frame_a: Option<u64>,

    /// Frame B for diff_frames.
    pub frame_b: Option<u64>,
}
```

**Deleted**: `CaptureConfig`, `recording_name` field.

**Handler dispatch:**
```rust
pub async fn handle_clips(
    params: ClipsParams,
    state: &Arc<Mutex<SessionState>>,
) -> Result<String, McpError> {
    let config = crate::tcp::get_config(state).await;
    let hard_cap = config.token_hard_cap;
    let budget_limit = resolve_budget(params.token_budget, 1500, hard_cap);

    match params.action.as_str() {
        "add_marker" => handle_add_marker(&params, state, budget_limit, hard_cap).await,
        "save" => handle_save(&params, state, budget_limit, hard_cap).await,
        "status" => query_and_finalize(state, "dashcam_status", json!({}), budget_limit, hard_cap).await,
        "list" => query_and_finalize(state, "recording_list", json!({}), budget_limit, hard_cap).await,
        "delete" => handle_delete(&params, state, budget_limit, hard_cap).await,
        "markers" => handle_markers(&params, state, budget_limit, hard_cap).await,
        "snapshot_at" => handle_snapshot_at(&params, state, budget_limit, hard_cap).await,
        "query_range" => handle_query_range(&params, state, budget_limit, hard_cap).await,
        "diff_frames" => handle_diff_frames(&params, state, budget_limit, hard_cap).await,
        "find_event" => handle_find_event(&params, state, budget_limit, hard_cap).await,
        other => Err(McpError::invalid_params(
            format!("Unknown clips action: '{other}'. Valid: add_marker, save, status, \
                     list, delete, markers, snapshot_at, query_range, diff_frames, find_event"),
            None,
        )),
    }
}
```

**`handle_save`** replaces `handle_flush_dashcam`:
```rust
async fn handle_save(
    params: &ClipsParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let query = json!({
        "marker_label": params.marker_label.as_deref().unwrap_or("agent save"),
    });
    let data = query_addon(state, "dashcam_flush", query).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}
```

Note: the TCP method name stays `dashcam_flush` — internal plumbing doesn't
need to match the MCP action name. The MCP layer translates.

**`handle_delete`** uses `clip_id` instead of `recording_id`:
```rust
async fn handle_delete(params: &ClipsParams, ...) -> Result<String, McpError> {
    let id = require_param!(params.clip_id.as_deref(), "clip_id is required for delete");
    let data = query_addon(state, "recording_delete", json!({ "clip_id": id })).await?;
    // ...
}
```

**Acceptance Criteria:**
- [ ] Tool named `clips` in MCP schema
- [ ] Params struct is `ClipsParams` with `clip_id` field
- [ ] No `start`, `stop` actions
- [ ] No `CaptureConfig`
- [ ] `save` action calls `dashcam_flush` TCP method
- [ ] `status` action calls `dashcam_status` TCP method
- [ ] All analysis actions use `clip_id`
- [ ] Tool description is workflow-focused

---

### Unit 2: Rename recording_analysis → clip_analysis

**File**: `crates/spectator-server/src/recording_analysis.rs` → `crates/spectator-server/src/clip_analysis.rs`

Rename types and update all JSON output field names:

```rust
// Renames:
RecordingSession → ClipSession
RecordingMeta → ClipMeta
resolve_storage_path → resolve_clip_storage_path
open_recording_db → open_clip_db
most_recent_recording → most_recent_clip

// JSON output renames:
"recording_id" → "clip_id"
"recording_context" → "clip_context"  (the context block in analysis responses)
```

**`ClipMeta.to_context()`**:
```rust
pub fn to_context(&self) -> serde_json::Value {
    json!({
        "clip_id": self.id,
        "name": self.name,
        "frame_range": [self.started_at_frame, self.ended_at_frame],
        "dimensions": match self.scene_dimensions { 2 => "2d", 3 => "3d", _ => "mixed" },
    })
}
```

**`ClipSession::open()`**: takes `clip_id: Option<&str>` instead of
`recording_id: Option<&str>`.

**`SessionState` rename:**
```rust
pub clip_storage_path: Option<String>,  // was: recording_storage_path
```

**Acceptance Criteria:**
- [ ] File renamed to `clip_analysis.rs`
- [ ] All types renamed
- [ ] All JSON output uses `clip_id` not `recording_id`
- [ ] `SessionState` field renamed
- [ ] `mod clip_analysis` in lib.rs/main.rs

---

### Unit 3: Update Activity Summaries

**File**: `crates/spectator-server/src/activity.rs`

Rename function, update to use `ClipsParams`, remove old actions, add `save`:

```rust
use crate::mcp::clips::ClipsParams;

pub fn clips_summary(params: &ClipsParams) -> String {
    match params.action.as_str() {
        "add_marker" => {
            let label = params.marker_label.as_deref().unwrap_or("(no label)");
            format!("Marker: {label}")
        }
        "save" => {
            let label = params.marker_label.as_deref().unwrap_or("agent save");
            format!("Saved clip: {label}")
        }
        "status" => "Dashcam status".into(),
        "list" => "Listing clips".into(),
        "delete" => {
            let id = params.clip_id.as_deref().unwrap_or("?");
            format!("Deleted clip {id}")
        }
        "markers" => {
            let id = params.clip_id.as_deref().unwrap_or("latest");
            format!("Markers for {id}")
        }
        // Analysis — keep existing logic, just update recording_id refs
        "snapshot_at" => { ... }   // use params.clip_id
        "query_range" => { ... }
        "diff_frames" => { ... }
        "find_event" => { ... }
        other => format!("Clips: {other}"),
    }
}
```

**Acceptance Criteria:**
- [ ] Function is `clips_summary` taking `ClipsParams`
- [ ] No `"start"` / `"stop"` match arms
- [ ] `"save"` replaces `"flush"` / `"flush_dashcam"`
- [ ] Uses `clip_id` not `recording_id`

---

### Unit 4: Update MCP Tool Router

**File**: `crates/spectator-server/src/mcp/mod.rs`

```rust
// mod recording;  →
pub mod clips;

// In #[tool_router] impl:
#[tool(description = "Capture and analyze gameplay clips. Clips are saved \
    automatically when you mark a moment with 'add_marker'. Use 'save' to \
    capture the buffer immediately. Analyze saved clips with 'snapshot_at', \
    'query_range', 'diff_frames', 'find_event'. Manage with 'list', 'delete', \
    'markers'. Check dashcam buffer with 'status'. Analysis defaults to the \
    most recent clip if clip_id is omitted.")]
pub async fn clips(
    &self,
    Parameters(params): Parameters<clips::ClipsParams>,
) -> Result<String, McpError> {
    let summary = crate::activity::clips_summary(&params);
    let result = clips::handle_clips(params, &self.state).await;
    self.log_activity("query", &summary, "clips").await;
    result
}
```

**Acceptance Criteria:**
- [ ] Tool is `clips` not `recording`
- [ ] Description is workflow-focused
- [ ] Activity logging uses `clips_summary`

---

### Unit 5: Strip Explicit Recording from SpectatorRecorder

**File**: `crates/spectator-godot/src/recorder.rs`

Remove all explicit recording state, methods, signals. Keep dashcam and shared
infrastructure (list, delete, get_markers).

**Remove from struct:**
```rust
recording: bool,
recording_id: String,
recording_name: String,
started_at_frame: u64,
started_at_ms: u64,
frames_captured: u32,
capture_interval: u32,
max_frames: u32,
frame_buffer: Vec<CapturedFrame>,
event_buffer: Vec<CapturedEvent>,
marker_buffer: Vec<CapturedMarker>,
flush_counter: u32,
db: Option<Connection>,
storage_path: String,
```

**Keep** `frame_counter: u32` (dashcam uses for capture interval).

**Delete types**: `CapturedEvent`, `CapturedMarker`.

**Delete signals**: `recording_started`, `recording_stopped`.

**Delete methods**: `is_recording`, `get_recording_id`, `get_recording_name`,
`get_frames_captured`, `get_elapsed_ms`, `get_buffer_size_kb`,
`get_recording_status`, `start_recording`, `stop_recording`.

**Delete internals**: `flush_to_db`, `create_db`.

**Simplify `add_marker`**: Remove `if self.recording` branch. Always route
to dashcam.

**Simplify `physics_process`**: Remove explicit recording path. Only dashcam.

**Update `list_recordings` JSON output**: `"recording_id"` → `"clip_id"` in
dict entries.

**Update `delete_recording`**: Parameter name stays (it's the GDExtension
method signature), but could rename to `delete_clip` for consistency. The
TCP handler calls it, so both must match.

**Update `get_recording_markers`** → rename to `get_clip_markers` (or keep
and just update JSON output fields).

**Update `flush_dashcam_clip_internal`**: Change clip ID prefix from
`"dash_"` to `"clip_"`.

**Update all unit tests**: Remove explicit recording tests. Keep dashcam tests.

**Acceptance Criteria:**
- [ ] No explicit recording state or methods
- [ ] `add_marker` only routes to dashcam
- [ ] `physics_process` only captures for dashcam
- [ ] All JSON output uses `clip_id`
- [ ] Clip IDs prefixed `clip_` not `dash_`
- [ ] All dashcam unit tests pass

---

### Unit 6: Update TCP Recording Handler

**File**: `crates/spectator-godot/src/recording_handler.rs`

Remove `recording_start`, `recording_stop`, `recording_status` handlers.
Update all JSON response fields from `recording_id` to `clip_id`.

**Updated dispatch:**
```rust
pub fn handle_recording_query(...) -> Result<Value, String> {
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

**`handle_marker`**: Remove `is_recording` branch. Always route to dashcam.

**All response JSON**: `"recording_id"` → `"clip_id"`.

**`handle_delete`**: Read `"clip_id"` from params (not `"recording_id"`).

**`handle_markers`**: Read `"clip_id"` from params.

**Acceptance Criteria:**
- [ ] No `recording_start`, `recording_stop`, `recording_status` handlers
- [ ] All JSON uses `clip_id`
- [ ] Marker handler has no `is_recording` branch

---

### Unit 7: Simplify GDScript Runtime

**File**: `addons/spectator/runtime.gd`

**Remove:**
- `_recording_dot` field + overlay creation
- `_record_keycode` field + F12 handling in `_shortcut_input()`
- `_toggle_recording()`, `_set_recording_indicator()`
- `_on_recording_started()`, `_on_recording_stopped()`
- Signal connections for `recording_started`, `recording_stopped`
- `spectator:recording` debugger message in `_push_status_to_editor()`
- `"start_recording"` / `"stop_recording"` in `_on_debugger_command()`
- `recorder.stop_recording()` in `_exit_tree()`

**Simplify `_drop_marker()`**: Remove `is_recording()` branch.

**Simplify `_on_marker_added()`**: Remove recording dot color flash.

**Acceptance Criteria:**
- [ ] No recording dot, toggle, indicator, F12
- [ ] No recording signal connections
- [ ] No debugger recording message

---

### Unit 8: Simplify Editor Dock

**Files**: `addons/spectator/dock.gd` + `addons/spectator/dock.tscn`

**Remove**: `record_btn`, `stop_btn`, `marker_btn`, `recording_stats`,
`_recording_active`, `receive_recording()`, all recording button handlers,
`_update_recording_controls()`, `_format_elapsed()`.

**Remove from tscn**: `RecordBtn`, `StopBtn`, `MarkerBtn`, `RecordingStats`.

**Keep**: Status section, activity log.

**Acceptance Criteria:**
- [ ] No recording buttons or stats in dock
- [ ] Status and activity still work

---

### Unit 9: Simplify Debugger Plugin + Plugin Settings

**File**: `addons/spectator/debugger_plugin.gd`

Remove `spectator:recording` handler, `send_command()`.

**File**: `addons/spectator/plugin.gd`

Remove `_dock._debugger_plugin` wiring (dock is read-only now).

Remove settings:
- `spectator/recording/storage_path`
- `spectator/recording/max_frames`
- `spectator/recording/capture_interval`
- `spectator/display/show_recording_indicator`
- `spectator/shortcuts/record_key`

**Acceptance Criteria:**
- [ ] No recording debugger messages
- [ ] No send_command
- [ ] No recording project settings

---

### Unit 10: Rename TOML Config Section

**File**: `crates/spectator-server/src/config.rs`

`RecordingConfig` → `DashcamTomlConfig`, section `[recording]` → `[dashcam]`,
drop `dashcam_` prefix from field names.

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

Update `toml_to_session_config` mapping + all tests.

**Acceptance Criteria:**
- [ ] TOML section `[dashcam]` with short field names
- [ ] All config tests pass

---

### Unit 11: Update Tests — Wire Tests

**File**: `tests/wire-tests/src/test_recording.rs` → rename to `test_clips.rs`

Remove explicit recording tests. Add dashcam-focused tests. All assertions
use `clip_id` not `recording_id`.

**Acceptance Criteria:**
- [ ] No explicit recording lifecycle tests
- [ ] Dashcam tests use `clip_id`

---

### Unit 12: Update Tests — Server Scenarios + E2E

**Files**: `crates/spectator-server/tests/scenarios.rs`, `e2e_journeys.rs`

- DELETE `journey_recording_lifecycle()` and
  `test_recording_lifecycle_ids_consistent()`
- UPDATE `journey_dashcam_agent_workflow()`: action `"flush_dashcam"` → `"save"`,
  `"dashcam_status"` → `"status"`, tool name `"recording"` → `"clips"`,
  all `"recording_id"` assertions → `"clip_id"`, clip prefix `"dash_"` → `"clip_"`

**Acceptance Criteria:**
- [ ] No explicit recording tests
- [ ] All tests use `clips` tool name and `clip_id` field

---

### Unit 13: Update Tests — GDScript

**Files**: `test_keybinds.gd`, `test_dock.gd`, `test_extension.gd`,
`test_runtime_wiring.gd`, `test_signals.gd`

- DELETE tests referencing `start_recording`, `stop_recording`, `is_recording`,
  `recording_started`, `recording_stopped`, record/stop buttons
- UPDATE signal lists in test_extension.gd
- UPDATE test_runtime_wiring.gd debugger command tests

**Acceptance Criteria:**
- [ ] No references to explicit recording API
- [ ] Remaining tests pass

---

### Unit 14: Update Contract Rules + Docs

**Files:**

**`.claude/rules/contracts.md`**: Update ID field examples:
```
clip_id        ✓    id      ✗
watch_id       ✓    id      ✗
session_id     ✓    id      ✗
marker_id      ✓    id      ✗
```
Update all prose mentioning `recording_id`.

**`docs/SPEC.md`**:
- Rename tool in tool table: `recording` → `clips`
- Remove `recording_start`, `recording_stop` from addon query methods table
- Update recording system section → "Clip Capture" section describing
  dashcam-only architecture
- Update default budgets table: `recording` → `clips`
- Use "clip" consistently, not "recording"
- Tool description: workflow-focused, not mechanism-focused
- ASCII diagram: "Recording Mgmt" → "Clip Analysis"

**`docs/VISION.md`**:
- "Record and analyze" bullet → "The dashcam captures context automatically
  around interesting moments; the agent scrubs through saved clips to diagnose
  what went wrong"

**Delete**: `docs/design/M7-RECORDING-CAPTURE.md` (explicit recording design)

**Keep**: `docs/design/M8-RECORDING-ANALYSIS.md` (analysis still applies)

**Update CLAUDE.md**: "9 MCP tools" — update tool list to show `clips` not
`recording`.

**`.agents/skills/spectator/SKILL.md`**: Update any references to `recording`
tool → `clips`, `recording_id` → `clip_id`.

**Acceptance Criteria:**
- [ ] Contract rules show `clip_id` not `recording_id`
- [ ] SPEC.md uses "clips" consistently
- [ ] No docs reference "start recording" / "stop recording"
- [ ] M7 deleted
- [ ] CLAUDE.md updated

---

## Implementation Order

1. **Unit 1**: MCP tool rename + slim (server, compiles independently)
2. **Unit 2**: Rename recording_analysis → clip_analysis
3. **Unit 3**: Activity summaries
4. **Unit 4**: MCP tool router
5. **Unit 10**: TOML config rename
6. **Unit 5**: SpectatorRecorder — strip explicit recording (biggest unit)
7. **Unit 6**: TCP recording handler
8. **Unit 7**: runtime.gd
9. **Unit 8**: dock
10. **Unit 9**: debugger plugin + plugin settings
11. **Unit 11**: Wire tests
12. **Unit 12**: Server tests
13. **Unit 13**: GDScript tests
14. **Unit 14**: Contract rules + docs

## Testing

### Verification Commands
```bash
cargo build --workspace
cargo clippy --workspace
cargo fmt --check
spectator-deploy ~/dev/spectator/tests/godot-project
cargo test --workspace
godot --headless --quit --path ~/dev/spectator/tests/godot-project 2>&1
```

### End-to-End Workflow
1. Game starts → dashcam buffers automatically
2. `clips(action: "status")` → `{ "state": "buffering", ... }`
3. `clips(action: "add_marker", marker_label: "bug here")` → clip capture triggered
4. `clips(action: "save")` → `{ "clip_id": "clip_...", ... }`
5. `clips(action: "list")` → array with saved clip
6. `clips(action: "snapshot_at", at_frame: N, clip_id: "clip_...")` → spatial state
7. `clips(action: "query_range", ...)` → matching frames
8. `clips(action: "diff_frames", ...)` → frame comparison
9. `clips(action: "delete", clip_id: "clip_...")` → `{ "clip_id": "clip_..." }`
10. F9 in-game → clip saved, toast shown

## Verification Checklist
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo clippy --workspace` clean
- [ ] `cargo test --workspace` all pass
- [ ] MCP schema shows `clips` tool with 10 actions
- [ ] All wire format uses `clip_id`, never `recording_id`
- [ ] Clip IDs prefixed `clip_` not `dash_` or `rec_`
- [ ] No `start_recording`/`stop_recording`/`is_recording` anywhere
- [ ] No `recording_started`/`recording_stopped` signals
- [ ] No `CaptureConfig` struct
- [ ] No record/stop buttons in dock, no F12 keybind
- [ ] No `spectator/recording/*` project settings
- [ ] `spectator.toml` `[dashcam]` section works
- [ ] Analysis tools work on saved clips
- [ ] Tool description reads as agent workflow, not implementation details
