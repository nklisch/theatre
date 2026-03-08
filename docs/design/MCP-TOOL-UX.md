# Design: MCP Tool Surface Improvements

## Overview

Systematic improvements to all 9 MCP tools based on real agent feedback. An agent
used our tools to debug a camera zoom issue and reported friction in 5 areas:

1. **Opaque condition format** — `query_range` condition structure required 3+ guesses
2. **Naming mismatch** — `recording_id` vs `clip_id` confusion
3. **No `moved` condition** — tried `{"type": "moved"}` but it doesn't exist
4. **No timeseries query** — had to make 10+ `snapshot_at` calls to reconstruct a path
5. **Input capture invisible** — couldn't tell if input was captured in a clip

This design addresses each issue and applies the same self-documentation principle
across all 9 tools: **an agent should never have to guess a parameter's shape**.

---

## Problem Analysis

### Root cause: JsonSchema descriptions are prose, not structural

The clips tool description says `"query_range" — search frames for spatial conditions`
and the `condition` field says `"Condition object for query_range"`. Neither shows
the actual JSON shape. The agent tried:

```json
"moved"                           // plain string
{"property_changed": "position"}  // wrong key
{"type": "moved"}                 // right shape, wrong value
{"type": "proximity", ...}        // finally correct
```

The `QueryCondition` struct uses `#[serde(rename = "type")]` for `condition_type`,
but this is invisible in the schema description. The valid values (`proximity`,
`velocity_spike`, `property_change`, `state_transition`, `signal_emitted`,
`entered_area`, `collision`) are only visible in the Rust match arm at
`clip_analysis.rs:489-503`.

### Root cause: missing condition type

The agent expected `{"type": "moved"}` because `spatial_delta` has a `moved` array.
This is a reasonable expectation — "show me frames where this node moved" is a
fundamental temporal query. The current conditions are event-based (proximity,
velocity spike, property change) but lack a basic movement condition.

### Root cause: no trajectory/timeseries capability

The agent wanted "Camera3D's position at every Nth frame across this range" and had
to make 10+ individual `snapshot_at` calls. This is the most expensive gap — it
wastes tokens and tool calls on a pattern that should be a single query.

---

## Implementation Units

### Unit 1: Enriched `condition` field description with inline schema

**File**: `crates/spectator-server/src/mcp/clips.rs`

Replace the condition field's doc comment with a self-documenting description that
shows every valid condition shape:

```rust
/// Spatial/temporal condition for query_range. JSON object with "type" key.
///
/// Valid types and their fields:
///   {"type": "moved", "threshold": 0.5}
///     — Frames where node moved more than threshold units (default 0.01).
///   {"type": "proximity", "target": "walls/*", "threshold": 0.5}
///     — Frames where distance to target < threshold.
///   {"type": "velocity_spike", "threshold": 5.0}
///     — Frames where speed changed by more than threshold between frames.
///   {"type": "property_change", "property": "health"}
///     — Frames where the named state property changed value.
///   {"type": "state_transition", "property": "alert_level"}
///     — Alias for property_change.
///   {"type": "signal_emitted", "signal": "health_changed"}
///     — Frames where the named signal was emitted (or any signal if omitted).
///   {"type": "entered_area"}
///     — Frames where node entered an area.
///   {"type": "collision"}
///     — Frames where node had a collision event.
#[schemars(description = "Condition for query_range. Object with \"type\" key. Types: \"moved\" (threshold), \"proximity\" (target, threshold), \"velocity_spike\" (threshold), \"property_change\" (property), \"state_transition\" (property), \"signal_emitted\" (signal), \"entered_area\", \"collision\". Example: {\"type\": \"proximity\", \"target\": \"walls/*\", \"threshold\": 0.5}")]
pub condition: Option<serde_json::Value>,
```

**Implementation Notes**:
- The `#[schemars(description = "...")]` attribute overrides the doc comment in
  the generated JsonSchema. Agents see the schemars text. Humans see the doc comment.
- Keep the schemars one-liner concise but include a concrete example.
- The doc comment above is for Rust developers reading the source.

**Acceptance Criteria**:
- [ ] Generated JsonSchema for `condition` field includes all 8 condition types
      (7 existing + new `moved`)
- [ ] Description includes at least one concrete JSON example
- [ ] Description lists required fields per condition type

---

### Unit 2: Add `moved` condition type to query_range

**File**: `crates/spectator-server/src/clip_analysis.rs`

Add a new condition type `"moved"` that detects frames where the target node's
position changed by more than a threshold compared to the previous frame.

```rust
/// In QueryCondition, `threshold` is reused:
/// - For "moved": minimum displacement in world units (default 0.01)

// Add to evaluate_condition match:
"moved" => evaluate_moved(frame, time_ms, node, entities, prev_entities, condition),

/// Detect frames where a node moved more than `threshold` units since the
/// previous frame. Default threshold: 0.01 (same as delta engine's movement
/// suppression threshold from SPEC.md).
fn evaluate_moved(
    frame: u64,
    time_ms: u64,
    node: &str,
    entities: &[FrameEntityData],
    prev_entities: &Option<Vec<FrameEntityData>>,
    condition: &QueryCondition,
) -> Option<RangeMatch> {
    let prev = prev_entities.as_ref()?;
    let threshold = condition.threshold.unwrap_or(0.01);

    let cur = find_entity(node, entities)?;
    let old = find_entity(node, prev)?;

    let dx: f64 = cur.position.iter()
        .zip(old.position.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        .sqrt();

    if dx >= threshold {
        Some(RangeMatch {
            frame,
            time_ms,
            distance: Some(dx),
            node_pos: Some(cur.position.clone()),
            node_velocity: if cur.velocity.iter().any(|v| *v != 0.0) {
                Some(cur.velocity.clone())
            } else {
                None
            },
            note: None,
        })
    } else {
        None
    }
}
```

**Implementation Notes**:
- Reuses the same `prev_entities` pattern as `evaluate_velocity_spike` and
  `evaluate_property_change` — no new frame-reading infrastructure needed.
- `find_entity` helper already exists (used by proximity, velocity_spike, etc.).
  If it doesn't exist as a standalone function, extract from the existing pattern
  where entities are searched by path/glob.
- The `distance` field in `RangeMatch` reports the displacement magnitude, which
  is natural for a movement query (matches the `distance` field semantics in
  proximity results).
- System markers are NOT auto-inserted for `moved` — it's too noisy. Only
  `velocity_spike`, `proximity`, and `collision` get auto-markers (per existing
  logic at line 450-451).

**Acceptance Criteria**:
- [ ] `{"type": "moved"}` returns frames where node displaced > 0.01 units
- [ ] `{"type": "moved", "threshold": 1.0}` uses custom threshold
- [ ] Returns empty results for stationary nodes
- [ ] `distance` field in each result shows displacement magnitude
- [ ] `node_pos` field shows position at that frame
- [ ] Unit test: node moves 5 units across 3 frames, `moved` with threshold 1.0
      returns those frames

---

### Unit 3: Add `trajectory` action to clips tool

**File**: `crates/spectator-server/src/mcp/clips.rs` (params) +
`crates/spectator-server/src/clip_analysis.rs` (implementation)

New action `trajectory` returns a compact timeseries of a node's position (and
optionally other properties) sampled at regular intervals across a frame range.
This replaces the 10+ `snapshot_at` calls pattern.

**Parameter additions to `ClipsParams`**:

```rust
/// Properties to sample in trajectory. Default: ["position"].
/// Options: "position", "rotation_deg", "velocity", "speed", or any state property name.
#[schemars(description = "Properties to sample in trajectory. Default: [\"position\"]. Options: position, rotation_deg, velocity, speed, or any state property name.")]
pub properties: Option<Vec<String>>,

/// Sample every Nth frame in trajectory. Default: 1 (every frame).
#[schemars(description = "Sample every Nth frame for trajectory. Default: 1.")]
pub sample_interval: Option<u64>,
```

**New analysis function**:

```rust
/// Sample a node's properties at regular intervals across a frame range.
/// Returns a compact timeseries suitable for understanding motion or state
/// evolution without per-frame tool calls.
pub fn trajectory(
    db: &Connection,
    node: &str,
    from_frame: u64,
    to_frame: u64,
    properties: &[String],
    sample_interval: u64,
    budget_limit: u32,
) -> Result<serde_json::Value, McpError>
```

**Response shape**:

```json
{
  "node": "Camera3D",
  "from_frame": 100,
  "to_frame": 300,
  "sample_interval": 10,
  "samples": [
    {"frame": 100, "time_ms": 1667, "position": [0, 60, 60]},
    {"frame": 110, "time_ms": 1833, "position": [0, 54, 54]},
    {"frame": 120, "time_ms": 2000, "position": [0, 48, 48]},
    ...
    {"frame": 300, "time_ms": 5000, "position": [-3.49, 2.60, 15.10]}
  ],
  "total_frames_in_range": 200,
  "samples_returned": 21
}
```

When additional properties are requested:

```json
{
  "samples": [
    {
      "frame": 100,
      "time_ms": 1667,
      "position": [0, 60, 60],
      "velocity": [0, -1.2, -1.2],
      "speed": 1.697
    }
  ]
}
```

**Implementation Notes**:
- Query frames with `WHERE frame BETWEEN ?1 AND ?2 AND (frame - ?1) % ?3 = 0`
  for efficient sampling. If SQLite modulo on frame numbers is unreliable (frames
  may not be contiguous), fall back to iterating and skipping in Rust.
- `"position"` → `entity.position`
- `"rotation_deg"` → `entity.rotation_deg`
- `"velocity"` → `entity.velocity`
- `"speed"` → magnitude of velocity vector
- Any other string → looked up in `entity.state` map
- Budget enforcement: stop emitting samples when estimated token usage reaches
  `budget_limit`. Each sample is ~30-50 tokens depending on properties.
- Node matching uses the same glob pattern logic as `query_range` (supports
  `"Camera3D"` exact match or `"enemies/*"` glob).

**Acceptance Criteria**:
- [ ] `trajectory` with `node`, `from_frame`, `to_frame` returns position timeseries
- [ ] `sample_interval: 10` skips to every 10th frame
- [ ] `properties: ["position", "velocity", "speed"]` includes all three
- [ ] `properties: ["health"]` includes state property values
- [ ] Budget truncation stops sampling when limit reached
- [ ] Unit test: 100 frames, sample_interval 10, returns 10 samples with correct
      frame numbers and positions

---

### Unit 4: Enrich clip list metadata for agent context

**File**: `crates/spectator-godot/src/recorder.rs` (write wall-clock time + build list)
**File**: `crates/spectator-godot/src/recording_handler.rs` (list response building)

Three problems with the current clip list:

1. **No wall-clock timestamp**: `created_at_ms` is engine time (physics frames),
   not a real timestamp. When the user says "the clip I just made" or "the one
   from earlier today", the agent can't correlate without wall-clock time.
2. **No marker preview**: `markers_count: 4` tells you markers exist but not what
   they say. The first human marker label is the most useful signal — it's
   typically why the clip was saved ("clipped through wall!", "zoom bug repro").
3. **No capture config visibility**: the agent can't tell if input was captured.

**Current list entry** (from `recording_handler.rs:46-63`):
```json
{
  "clip_id": "clip_abc12345",
  "name": "dashcam_1741456800",
  "frames_captured": 340,
  "duration_ms": 5667,
  "frame_range": [2800, 3140],
  "markers_count": 4,
  "size_kb": 420,
  "created_at_ms": 46667,
  "dashcam": true,
  "tier": "deliberate"
}
```

**New list entry**:
```json
{
  "clip_id": "clip_abc12345",
  "name": "dashcam_1741456800",
  "frames_captured": 340,
  "duration_ms": 5667,
  "frame_range": [2800, 3140],
  "markers_count": 4,
  "size_kb": 420,
  "created_at": "2026-03-08T14:20:00Z",
  "trigger_label": "clipped through wall!",
  "dashcam": true,
  "tier": "deliberate",
  "capture": {
    "include_input": false,
    "include_signals": true,
    "capture_interval": 1
  }
}
```

**Changes**:

#### 4a. Add `created_at` as ISO 8601 wall-clock timestamp

**File**: `crates/spectator-godot/src/recorder.rs`

Add a `created_at_unix_ms` column to the recording table schema, populated with
`current_time_ms()` (which is already wall-clock) at clip save time.

```sql
-- Add to CREATE TABLE recording:
created_at_unix_ms INTEGER
```

```rust
// In flush_dashcam_to_disk, add to INSERT:
let wall_clock_ms = current_time_ms();
// ... params: [..., wall_clock_ms]
```

In `list_recordings`, read `created_at_unix_ms` from the DB and format as ISO 8601:

```rust
// In list_recordings:
let created_unix_ms: i64 = row.get(N)?;  // new column
// ...
dict.set("created_at_unix_ms", created_unix_ms);
```

In `recording_handler.rs`, convert to ISO 8601 string:

```rust
// Format unix ms as ISO 8601 for the JSON response
fn unix_ms_to_iso8601(ms: i64) -> String {
    let secs = ms / 1000;
    let rem_ms = ms % 1000;
    // Use chrono-free approach: format as UTC manually
    // Or use the time crate if available
    // Simplest: return Unix seconds and let the consumer format
    // But ISO 8601 is more agent-friendly
    format_unix_to_iso(secs, rem_ms)
}

// In handle_list, replace created_at_ms with:
"created_at": unix_ms_to_iso8601(created_at_unix_ms),
```

**Implementation Notes**:
- The clip `name` field already contains the unix timestamp (`dashcam_{secs}`) but
  it's not structured. We need a dedicated field.
- `current_time_ms()` already exists in recorder.rs and returns wall-clock Unix ms.
- For existing clips that don't have `created_at_unix_ms`, fall back to the file's
  filesystem modification time (`std::fs::metadata(&path).modified()`), which
  `list_recordings` already reads for sorting.
- Replace the confusing `created_at_ms` field (engine time) with `created_at`
  (ISO 8601 string). This is a breaking change to the list response but the old
  field was misleading and nobody should be relying on engine-time for calendar
  correlation.
- For the ISO 8601 formatting without pulling in chrono: use a small helper that
  computes UTC from Unix seconds. The `time` crate is lightweight but may not be
  in the dependency tree — check first. If not available, a manual formatter for
  `YYYY-MM-DDTHH:MM:SSZ` from Unix seconds is ~15 lines.

#### 4b. Add `trigger_label` — the first human/agent marker label

**File**: `crates/spectator-godot/src/recorder.rs`

When building the list, query the first marker in the clip that was set by a human
or agent (not system). This is typically the reason the clip was saved.

```rust
// In list_recordings, after opening the DB:
let trigger_label: Option<String> = db
    .query_row(
        "SELECT label FROM markers WHERE source IN ('human', 'agent') \
         ORDER BY frame ASC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .ok();
```

```rust
// In the dict:
if let Some(label) = trigger_label {
    dict.set("trigger_label", GString::from(&label));
}
```

In `recording_handler.rs`, forward it:

```rust
// Only include if present:
"trigger_label": dict.get("trigger_label").map(|v| v.to_string()),
```

**Implementation Notes**:
- `trigger_label` is the first non-system marker's label. For dashcam clips
  triggered by `add_marker`, this is the label the agent or human provided.
- If the clip was triggered by a system event (velocity spike) with no human
  markers, `trigger_label` is omitted (the agent can call `markers` for details).
- This answers "what is this clip about?" at a glance without calling `markers`.

#### 4c. Add `capture` block with input/signal/interval info

Surface the `capture_config` JSON as a structured `capture` block.

```rust
// In recording_handler.rs handle_list, parse capture_config:
let capture_block = capture_config.as_deref()
    .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
    .map(|v| json!({
        "include_input": v.get("include_input").and_then(|b| b.as_bool()).unwrap_or(false),
        "include_signals": v.get("include_signals").and_then(|b| b.as_bool()).unwrap_or(true),
        "capture_interval": v.get("capture_interval").and_then(|n| n.as_u64()).unwrap_or(1),
    }));

// Include in response:
"capture": capture_block,
```

**Acceptance Criteria**:
- [ ] `clips(action: "list")` entries include `created_at` as ISO 8601 string
- [ ] `created_at` reflects wall-clock time, not engine time
- [ ] Existing clips without `created_at_unix_ms` column fall back to file mtime
- [ ] Old `created_at_ms` field (engine time) is removed
- [ ] `trigger_label` shows the first human/agent marker label when present
- [ ] `trigger_label` is omitted when no human/agent markers exist
- [ ] `capture.include_input` boolean is present
- [ ] `capture.include_signals` and `capture.capture_interval` are present
- [ ] Missing or NULL `capture_config` → no `capture` field (no error)

---

### Unit 5: Enrich tool descriptions across all 9 tools

**File**: `crates/spectator-server/src/mcp/mod.rs` (tool descriptions)

The tool description is the first thing an agent reads. Current descriptions list
actions but don't show parameter shapes for non-obvious inputs. Apply the same
self-documentation principle used in Unit 1 to all tools where parameter format
isn't obvious from the field name.

**clips tool** — add condition format hint and trajectory action:

```
Capture and analyze gameplay clips. Actions:
- 'add_marker' — mark a moment (triggers clip save). Optional: marker_label.
- 'save' — force-save dashcam buffer. Optional: marker_label.
- 'status' — dashcam state.
- 'list' — saved clips with metadata.
- 'delete' — remove clip. Requires: clip_id.
- 'markers' — list markers in a clip.
- 'snapshot_at' — state at a frame. Requires: at_frame or at_time_ms.
- 'trajectory' — position/property timeseries across frame range. Requires: node, from_frame, to_frame. Optional: properties (default ["position"]), sample_interval.
- 'query_range' — search frames matching a condition. Requires: node, from_frame, to_frame, condition. Condition example: {"type": "proximity", "target": "walls/*", "threshold": 0.5}. Types: moved, proximity, velocity_spike, property_change, state_transition, signal_emitted, entered_area, collision.
- 'diff_frames' — compare two frames. Requires: frame_a, frame_b.
- 'find_event' — search events. Requires: event_type.
Analysis defaults to most recent clip if clip_id omitted.
```

**spatial_action tool** — add per-action required params:

```
Manipulate game state for debugging. Actions and required parameters:
- 'pause' — pause/unpause. Requires: paused (bool).
- 'advance_frames' — step physics frames. Requires: frames (int).
- 'advance_time' — step seconds. Requires: seconds (float).
- 'teleport' — move node. Requires: node, position. Optional: rotation_deg.
- 'set_property' — change property. Requires: node, property, value.
- 'call_method' — call method. Requires: node, method. Optional: method_args.
- 'emit_signal' — emit signal. Requires: node, signal. Optional: args.
- 'spawn_node' — instantiate scene. Requires: scene_path, parent. Optional: name, position.
- 'remove_node' — delete node. Requires: node.
Set return_delta=true to get a spatial delta showing what changed.
```

**spatial_watch tool** — add condition format:

```
Subscribe to changes on nodes or groups. Actions:
- 'add' — subscribe. Requires: watch.node (path or "group:name"). Optional: watch.conditions (array of {property, operator, value}; operators: lt, gt, eq, changed), watch.track (array: position, state, signals, physics, all).
- 'remove' — unsubscribe. Requires: watch_id.
- 'list' — show active watches.
- 'clear' — remove all watches.
Watch triggers appear in spatial_delta responses under 'watch_triggers'.
```

**spatial_query tool** — add per-type required params:

```
Targeted spatial questions. Query types and required parameters:
- 'nearest' — K nearest nodes. Requires: from. Optional: k (default 5), groups, class_filter. Needs prior spatial_snapshot.
- 'radius' — nodes within distance. Requires: from. Optional: radius (default 20), groups, class_filter. Needs prior spatial_snapshot.
- 'area' — alias for radius.
- 'raycast' — line-of-sight check. Requires: from, to.
- 'path_distance' — navmesh distance. Requires: from, to.
- 'relationship' — mutual spatial info. Requires: from, to.
'from' and 'to' accept node path (string) or position array [x,y,z].
```

**Implementation Notes**:
- The `#[tool(description = "...")]` attribute is a single string. Use the
  structured action-list format above instead of the current run-on sentence.
- rmcp renders tool descriptions as-is in the MCP schema. Newlines within the
  string are preserved in the JSON schema `description` field.
- Keep total description length reasonable — agents see this in every
  `tools/list` response.

**Acceptance Criteria**:
- [ ] All 4 action-based tools (clips, spatial_action, spatial_watch, scene_tree)
      list per-action required parameters in their description
- [ ] clips description includes condition type list with example
- [ ] clips description includes trajectory action
- [ ] spatial_query description lists per-type required parameters
- [ ] No tool description relies on agents guessing parameter shapes

---

### Unit 6: Enrich field-level JsonSchema descriptions

**File**: `crates/spectator-server/src/mcp/clips.rs`
**File**: `crates/spectator-server/src/mcp/action.rs`
**File**: `crates/spectator-server/src/mcp/query.rs`
**File**: `crates/spectator-server/src/mcp/watch.rs`
**File**: `crates/spectator-server/src/mcp/snapshot.rs`

Add `#[schemars(description = "...")]` to fields where the type alone doesn't
communicate valid values, format, or defaults.

**Priority fields** (these caused actual agent confusion or have non-obvious formats):

```rust
// clips.rs — action field
#[schemars(description = "Action: add_marker, save, status, list, delete, markers, snapshot_at, trajectory, query_range, diff_frames, find_event")]
pub action: String,

// clips.rs — clip_id field
#[schemars(description = "Clip to operate on (from list response). Defaults to most recent clip if omitted.")]
pub clip_id: Option<String>,

// action.rs — action field
#[schemars(description = "Action: pause, advance_frames, advance_time, teleport, set_property, call_method, emit_signal, spawn_node, remove_node")]
pub action: String,

// query.rs — from field
#[schemars(description = "Origin: node path string (e.g. \"player\") or position array [x,y,z]")]
pub from: serde_json::Value,

// query.rs — to field
#[schemars(description = "Target: node path string or position array [x,y,z]. Required for raycast, path_distance, relationship.")]
pub to: Option<serde_json::Value>,

// watch.rs — action field
#[schemars(description = "Action: add, remove, list, clear")]
pub action: String,

// watch.rs — WatchConditionInput.operator
#[schemars(description = "Comparison operator: lt (less than), gt (greater than), eq (equals), changed (any change)")]
pub operator: String,

// snapshot.rs — perspective
#[schemars(description = "Where to look from: \"camera\" (active camera, default), \"node\" (requires focal_node), \"point\" (requires focal_point)")]
pub perspective: String,

// snapshot.rs — detail
#[schemars(description = "Detail tier: \"summary\" (~200 tokens, clusters only), \"standard\" (~400-800 tokens, per-entity), \"full\" (~1000+ tokens, transforms/physics/children)")]
pub detail: String,
```

**Implementation Notes**:
- Fields that already have good doc comments but no `#[schemars]` attribute get
  the attribute added. Doc comments are for Rust developers; schemars descriptions
  are for agents reading the JSON schema.
- For enum-like string fields, always list valid values in the schemars description.
- For fields with defaults, state the default in the description.

**Acceptance Criteria**:
- [ ] Every string field that accepts an enumerated set of values has all valid
      values listed in its `#[schemars(description)]`
- [ ] Every field with a default value states the default in its description
- [ ] `condition` field description includes all condition types with required fields
- [ ] `from`/`to` fields describe both node path and array formats
- [ ] `perspective` field describes what each option requires

---

### Unit 7: Improve error messages for invalid conditions

**File**: `crates/spectator-server/src/clip_analysis.rs`

Currently, an unknown condition type silently returns `None` (no match), which
produces an empty results array. The agent gets `"results": []` with no indication
that their condition type was invalid.

```rust
// Current (line 503):
_ => None,

// New:
other => {
    return Err(McpError::invalid_params(
        format!(
            "Unknown condition type '{}'. Valid types: moved, proximity, \
             velocity_spike, property_change, state_transition, signal_emitted, \
             entered_area, collision",
            other
        ),
        None,
    ));
}
```

This requires changing `evaluate_condition` to return `Result<Option<RangeMatch>, McpError>`
and propagating the error up through `query_range`.

**Current signature**:
```rust
fn evaluate_condition(
    db: &Connection,
    frame: u64,
    time_ms: u64,
    node: &str,
    entities: &[FrameEntityData],
    condition: &QueryCondition,
    prev_entities: &Option<Vec<FrameEntityData>>,
) -> Option<RangeMatch>
```

**New signature**:
```rust
fn evaluate_condition(
    db: &Connection,
    frame: u64,
    time_ms: u64,
    node: &str,
    entities: &[FrameEntityData],
    condition: &QueryCondition,
    prev_entities: &Option<Vec<FrameEntityData>>,
) -> Result<Option<RangeMatch>, McpError>
```

**Callers in `query_range`** need to change from:
```rust
if let Some(m) = evaluate_condition(...) {
```
to:
```rust
if let Some(m) = evaluate_condition(...)? {
```

**Implementation Notes**:
- This is a refactor of internal functions — no MCP API change.
- All existing condition evaluators return `Ok(Some(...))` or `Ok(None)`.
- Only the `_` catch-all returns `Err(...)`.

**Acceptance Criteria**:
- [ ] `{"type": "moved"}` works (new condition, not error)
- [ ] `{"type": "foo"}` returns `McpError::invalid_params` listing valid types
- [ ] Error message includes the invalid type name and all valid type names
- [ ] Existing condition types still work unchanged

---

### Unit 8: Add `moved` condition to CONTRACT.md and SPEC.md

**File**: `docs/CONTRACT.md`
**File**: `docs/SPEC.md`

Update the contract and spec documentation to reflect the new `moved` condition
type and the `trajectory` action.

**CONTRACT.md** — update the `condition` parameter section in Tool 9 (recording/clips):

Add `"moved"` to the condition type enum:
```jsonc
"type": {
    "enum": ["proximity", "property_change", "signal_emitted",
             "entered_area", "velocity_spike", "state_transition",
             "collision", "moved"]
}
```

Add `"trajectory"` to the action enum and add response example:
```jsonc
// Response — trajectory
{
  "node": "Camera3D",
  "from_frame": 100,
  "to_frame": 300,
  "sample_interval": 10,
  "samples": [
    {"frame": 100, "time_ms": 1667, "position": [0, 60, 60]},
    {"frame": 110, "time_ms": 1833, "position": [0, 54, 54]}
  ],
  "total_frames_in_range": 200,
  "samples_returned": 21,
  "budget": { /* ... */ }
}
```

**Acceptance Criteria**:
- [ ] CONTRACT.md lists `moved` in condition type enum
- [ ] CONTRACT.md lists `trajectory` in action enum
- [ ] CONTRACT.md has trajectory response example
- [ ] SPEC.md mentions `moved` condition in the query_range description

---

## Implementation Order

1. **Unit 7** — Error messages for invalid conditions (enables faster iteration)
2. **Unit 2** — Add `moved` condition type
3. **Unit 3** — Add `trajectory` action
4. **Unit 4** — Add `include_input` to list response
5. **Unit 1** — Enrich `condition` field description
6. **Unit 6** — Enrich field-level JsonSchema descriptions across all tools
7. **Unit 5** — Enrich tool descriptions across all 9 tools
8. **Unit 8** — Update CONTRACT.md and SPEC.md

Rationale: Fix error feedback first (Unit 7), then add missing capabilities
(Units 2-4), then improve documentation (Units 1, 5-6, 8). Documentation units
come last because they reference the new capabilities.

---

## Testing

### Unit Tests: `crates/spectator-server/src/clip_analysis.rs`

**New tests for `moved` condition**:
```rust
#[test]
fn test_moved_condition_detects_displacement() {
    // Setup: 3 frames where Camera3D moves [0,60,60] → [0,55,55] → [0,48,48]
    // Condition: {"type": "moved", "threshold": 1.0}
    // Expected: 2 results (frame 2 moved 7.07 units, frame 3 moved 9.9 units)
}

#[test]
fn test_moved_condition_respects_threshold() {
    // Setup: 3 frames, node moves 0.005 units per frame
    // Condition: {"type": "moved"} (default threshold 0.01)
    // Expected: 0 results (below threshold)
}

#[test]
fn test_moved_condition_default_threshold() {
    // Verify default threshold is 0.01 when threshold field omitted
}

#[test]
fn test_moved_condition_stationary_node() {
    // Setup: 3 frames, node doesn't move
    // Expected: 0 results
}
```

**New tests for `trajectory`**:
```rust
#[test]
fn test_trajectory_basic() {
    // Setup: 10 frames with known positions
    // Call trajectory(node, 1, 10, ["position"], 1)
    // Expected: 10 samples with correct positions
}

#[test]
fn test_trajectory_sample_interval() {
    // Setup: 100 frames
    // Call trajectory(node, 1, 100, ["position"], 10)
    // Expected: 10 samples at frames 1, 11, 21, ..., 91
}

#[test]
fn test_trajectory_multiple_properties() {
    // Setup: frames with position, velocity, and "health" state
    // Call trajectory(node, 1, 5, ["position", "velocity", "health"], 1)
    // Expected: samples include all three properties
}

#[test]
fn test_trajectory_budget_truncation() {
    // Setup: 1000 frames
    // Call with budget_limit = 100 (~2-3 samples worth)
    // Expected: truncated with samples_returned < total
}
```

**New tests for error reporting**:
```rust
#[test]
fn test_invalid_condition_type_returns_error() {
    // Condition: {"type": "foo"}
    // Expected: McpError::invalid_params with valid types listed
}

#[test]
fn test_moved_is_valid_condition_type() {
    // Condition: {"type": "moved"}
    // Expected: Ok (not an error), returns results array
}
```

**Tests for clip list metadata** (in `clips.rs` or `recording_handler` tests):
```rust
#[test]
fn test_list_includes_created_at_iso8601() {
    // Setup: clip with created_at_unix_ms in recording table
    // Expected: list entry has "created_at" as ISO 8601 string
}

#[test]
fn test_list_includes_trigger_label() {
    // Setup: clip with human marker "zoom bug repro"
    // Expected: list entry has trigger_label: "zoom bug repro"
}

#[test]
fn test_list_omits_trigger_label_when_no_human_markers() {
    // Setup: clip with only system markers
    // Expected: list entry has no trigger_label field
}

#[test]
fn test_list_includes_capture_config() {
    // Setup: clip with capture_config JSON in recording table
    // Expected: list entry has capture.include_input field
}

#[test]
fn test_list_fallback_mtime_for_old_clips() {
    // Setup: clip without created_at_unix_ms column
    // Expected: created_at falls back to file modification time
}
```

### Integration Tests

No new integration test files needed. Existing E2E journey tests cover the clips
tool flow. The new `moved` condition and `trajectory` action will be exercised
through the existing test infrastructure once deployed.

---

## Verification Checklist

```bash
# Build
cargo build --workspace

# All tests pass
spectator-deploy ~/dev/spectator/tests/godot-project
cargo test --workspace

# Lint
cargo clippy --workspace
cargo fmt --check

# Verify JsonSchema output includes new descriptions
# (manual: run server, call tools/list, inspect schema)
```

### Manual Verification

1. Start a Godot project with Spectator, connect an MCP client
2. Call `clips(action: "list")` → verify `capture` block in response
3. Call `clips(action: "query_range", condition: {"type": "foo"})` → verify
   helpful error message listing valid types
4. Call `clips(action: "query_range", condition: {"type": "moved"})` → verify
   movement detection works
5. Call `clips(action: "trajectory", node: "Camera3D", from_frame: X, to_frame: Y)`
   → verify compact timeseries response
6. Inspect `tools/list` JSON schema → verify enriched descriptions on all fields
