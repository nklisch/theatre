# Design: Milestone 8 — Recording Analysis

## Overview

M8 delivers the analysis side of the recording system: the agent can scrub through recorded timelines, query across frame ranges with spatial conditions, diff frames, search for events, and receive system-generated markers. This completes the collaborative debugging workflow: human records, agent analyzes.

**Depends on:** M7 (recording capture, SQLite storage, StageRecorder, FrameEntityData format)

**Exit Criteria:** The full workflow from UX.md works: human records, marks a bug with F9, stops recording. Agent queries markers, snapshots the frame before the bug, runs a proximity query to find when the enemy breached the wall, diffs the before/after frames, adds its own marker with the root cause. Agent reports findings with frame references.

---

## Architecture Decision: Server Reads SQLite Directly

**Decision:** The MCP server opens recording SQLite files directly (read-only) for all analysis queries. No TCP round-trips to the addon for frame scanning.

**Rationale:**
- SPEC.md explicitly assigns "Recording analysis (query_range, diff)" to stage-server
- Scanning 1800 frames of MessagePack over TCP would be prohibitively slow
- Spatial condition evaluation (proximity, velocity spike) requires Rust spatial logic from stage-core
- The server and addon run on the same machine — SQLite files are directly accessible
- WAL mode (set during M7 capture) allows concurrent reads while recording is active

**Consequence:** The server needs to know the actual filesystem path to recordings (not the `user://` Godot path). A new TCP method `recording_resolve_path` returns the globalized storage path, cached in `SessionState` on first use.

---

## Architecture Decision: FrameEntityData in stage-protocol

**Decision:** Move `FrameEntityData` from `stage-godot/src/recorder.rs` (private) to `stage-protocol/src/recording.rs` (public). Both crates depend on stage-protocol.

**Rationale:**
- `FrameEntityData` is a wire format — the binary schema of frame BLOBs stored in SQLite
- Both stage-godot (writer) and stage-server (reader) must agree on the format
- stage-protocol is the shared types crate; stage-godot already depends on it
- Moving it ensures format changes are caught at compile time across both crates

---

## Current State Analysis

### What M7 provides:
1. **StageRecorder** — captures frames to SQLite via `capture_frame()` → MessagePack → `flush_to_db()`
2. **SQLite schema** — 4 tables: `recording`, `frames`, `events`, `markers` with indexes
3. **Recording MCP tool** — 7 actions: start, stop, status, list, delete, markers, add_marker
4. **FrameEntityData** — compact struct serialized as MessagePack per frame (path, class, position, rotation_deg, velocity, groups, visible, state)
5. **TCP methods** — `recording_start`, `recording_stop`, `recording_status`, `recording_list`, `recording_delete`, `recording_marker`, `recording_markers`

### What M8 adds:
1. **4 new recording actions**: `snapshot_at`, `query_range`, `diff_frames`, `find_event`
2. **Recording analysis engine** in stage-server (SQLite reads, MessagePack deserialization, spatial condition evaluation)
3. **System marker generation** (velocity spike, property threshold detection during query_range scans)
4. **Storage path resolution** (addon TCP method to globalize `user://` paths)
5. **Token budget enforcement** on analysis query results

---

## Implementation Units

### Unit 1: FrameEntityData in stage-protocol

**File**: `crates/stage-protocol/src/recording.rs` (new)

```rust
use serde::{Deserialize, Serialize};

/// Compact entity snapshot stored as MessagePack in recording frame BLOBs.
/// This is the wire format agreed upon by stage-godot (writer) and
/// stage-server (reader). Changes here require coordinated updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameEntityData {
    pub path: String,
    pub class: String,
    pub position: Vec<f64>,
    pub rotation_deg: Vec<f64>,
    pub velocity: Vec<f64>,
    pub groups: Vec<String>,
    pub visible: bool,
    pub state: serde_json::Map<String, serde_json::Value>,
}
```

**File**: `crates/stage-protocol/src/lib.rs` — add `pub mod recording;`

**File**: `crates/stage-godot/src/recorder.rs` — replace private `FrameEntityData` with `use stage_protocol::recording::FrameEntityData;`

**Acceptance Criteria:**
- [ ] `FrameEntityData` is defined in stage-protocol with `pub` visibility
- [ ] stage-godot imports from stage-protocol (no local copy)
- [ ] stage-server can import `stage_protocol::recording::FrameEntityData`
- [ ] Existing M7 tests in recorder.rs still pass with the import change
- [ ] `cargo test --workspace` passes

---

### Unit 2: Storage Path Resolution

**File**: `crates/stage-godot/src/recording_handler.rs` — add handler

```rust
/// Handle "recording_resolve_path" — returns the globalized filesystem path
/// for the recording storage directory.
fn handle_resolve_path(
    _params: &Value,
) -> Result<Value, (String, String)> {
    let storage = "user://stage_recordings/";
    let globalized = crate::recorder::globalize_path(storage);
    Ok(serde_json::json!({ "path": globalized }))
}
```

**File**: `crates/stage-godot/src/recorder.rs` — make `globalize_path` pub(crate)

```rust
pub(crate) fn globalize_path(godot_path: &str) -> String {
    // ... existing implementation
}
```

**File**: `crates/stage-server/src/tcp.rs` — add cached storage path to SessionState

```rust
pub struct SessionState {
    // ... existing fields ...
    /// Cached filesystem path to recording storage (resolved from addon).
    pub recording_storage_path: Option<String>,
}
```

**File**: `crates/stage-server/src/recording_analysis.rs` — path resolution helper

```rust
/// Resolve the recording storage path, caching the result in SessionState.
/// Queries the addon once via TCP, then uses the cached value.
pub async fn resolve_storage_path(
    state: &Arc<Mutex<SessionState>>,
) -> Result<String, McpError> {
    {
        let s = state.lock().await;
        if let Some(ref path) = s.recording_storage_path {
            return Ok(path.clone());
        }
    }
    let data = query_addon(state, "recording_resolve_path", json!({})).await?;
    let path = data["path"]
        .as_str()
        .ok_or_else(|| McpError::internal_error("Invalid storage path response".into(), None))?
        .to_string();
    {
        let mut s = state.lock().await;
        s.recording_storage_path = Some(path.clone());
    }
    Ok(path)
}

/// Open a recording's SQLite database read-only.
pub fn open_recording_db(storage_path: &str, recording_id: &str) -> Result<Connection, McpError> {
    let db_path = format!("{}/{}.sqlite", storage_path, recording_id);
    Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| {
        McpError::invalid_params(
            format!("Recording '{recording_id}' not found or unreadable: {e}"),
            None,
        )
    })
}
```

**Implementation Notes:**
- The addon dispatch in `tcp_server.rs` must route `recording_resolve_path` to the recording handler
- Storage path is cached per session; cleared on reconnect (SessionState reset)

**Acceptance Criteria:**
- [ ] `recording_resolve_path` TCP method returns the globalized filesystem path
- [ ] Server caches the path in SessionState after first resolution
- [ ] `open_recording_db` returns a read-only SQLite connection
- [ ] Returns `McpError::invalid_params` with recording_id for missing files

---

### Unit 3: Recording Analysis Engine

**File**: `crates/stage-server/src/recording_analysis.rs` (new)

This is the core analysis module. It reads SQLite and evaluates conditions in Rust.

#### 3a: Frame Deserialization

```rust
use rusqlite::Connection;
use stage_protocol::recording::FrameEntityData;

/// Read and deserialize a single frame's entity data from SQLite.
pub fn read_frame(db: &Connection, frame: u64) -> Result<Vec<FrameEntityData>, McpError> {
    let data: Vec<u8> = db
        .query_row(
            "SELECT data FROM frames WHERE frame = ?1",
            rusqlite::params![frame],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => McpError::invalid_params(
                format!("Frame {frame} not found in recording"),
                None,
            ),
            other => McpError::internal_error(format!("SQLite error: {other}"), None),
        })?;
    rmp_serde::from_slice(&data).map_err(|e| {
        McpError::internal_error(format!("MessagePack decode error at frame {frame}: {e}"), None)
    })
}

/// Read frame data by timestamp (finds nearest frame).
pub fn read_frame_at_time(db: &Connection, time_ms: u64) -> Result<(u64, Vec<FrameEntityData>), McpError> {
    let (frame, data): (u64, Vec<u8>) = db
        .query_row(
            "SELECT frame, data FROM frames ORDER BY ABS(timestamp_ms - ?1) LIMIT 1",
            rusqlite::params![time_ms],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| McpError::internal_error(format!("SQLite error: {e}"), None))?;
    let entities = rmp_serde::from_slice(&data).map_err(|e| {
        McpError::internal_error(format!("MessagePack decode error: {e}"), None)
    })?;
    Ok((frame, entities))
}

/// Get recording metadata from the recording table.
pub fn read_recording_meta(db: &Connection) -> Result<RecordingMeta, McpError> {
    db.query_row(
        "SELECT id, name, started_at_frame, ended_at_frame, started_at_ms, ended_at_ms, \
         scene_dimensions, physics_ticks_per_sec FROM recording LIMIT 1",
        [],
        |row| {
            Ok(RecordingMeta {
                id: row.get(0)?,
                name: row.get(1)?,
                started_at_frame: row.get(2)?,
                ended_at_frame: row.get(3)?,
                started_at_ms: row.get(4)?,
                ended_at_ms: row.get(5)?,
                scene_dimensions: row.get::<_, Option<i64>>(6)?.unwrap_or(3) as u32,
                physics_ticks_per_sec: row.get::<_, Option<i64>>(7)?.unwrap_or(60) as u32,
            })
        },
    )
    .map_err(|e| McpError::internal_error(format!("Failed to read recording metadata: {e}"), None))
}

pub struct RecordingMeta {
    pub id: String,
    pub name: String,
    pub started_at_frame: i64,
    pub ended_at_frame: Option<i64>,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub scene_dimensions: u32,
    pub physics_ticks_per_sec: u32,
}
```

#### 3b: snapshot_at

```rust
use serde_json::json;
use stage_core::{bearing, types::Position3};

/// Reconstruct spatial state at a specific recorded frame.
/// Returns the same shape as spatial_snapshot (standard detail).
pub fn snapshot_at(
    db: &Connection,
    frame: u64,
    detail: &str,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<serde_json::Value, McpError> {
    let entities = read_frame(db, frame)?;
    let timestamp_ms: u64 = db
        .query_row(
            "SELECT timestamp_ms FROM frames WHERE frame = ?1",
            rusqlite::params![frame],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Build perspective from first camera-like entity or default to origin
    let perspective_pos: Position3 = [0.0, 0.0, 0.0];
    let perspective = bearing::perspective_from_yaw(perspective_pos, 0.0);

    let mut entity_list: Vec<serde_json::Value> = entities
        .iter()
        .map(|e| {
            let pos: Position3 = stage_core::types::vec_to_array3(&e.position);
            let rel = bearing::relative_position(&perspective, pos, !e.visible);
            let mut entry = json!({
                "path": e.path,
                "class": e.class,
                "abs": e.position,
                "groups": e.groups,
                "visible": e.visible,
            });
            if detail != "summary" {
                entry["rel"] = json!({
                    "dist": rel.dist,
                    "bearing": rel.bearing,
                    "bearing_deg": rel.bearing_deg,
                });
                entry["rot_y"] = json!(e.rotation_deg.get(1).copied().unwrap_or(0.0));
                let vel_mag: f64 = e.velocity.iter().map(|v| v * v).sum::<f64>().sqrt();
                if vel_mag > 0.01 {
                    entry["velocity"] = json!(e.velocity);
                }
                if !e.state.is_empty() {
                    entry["state"] = json!(e.state);
                }
            }
            if detail == "full" {
                entry["rotation_deg"] = json!(e.rotation_deg);
            }
            (rel.dist, entry)
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|(_dist, entry)| entry) // already sorted below
        .collect();

    // Sort by distance nearest-first
    // (re-do to sort properly)
    let mut with_dist: Vec<(f64, serde_json::Value)> = entities
        .iter()
        .zip(entity_list.drain(..))
        .map(|(e, entry)| {
            let pos: Position3 = stage_core::types::vec_to_array3(&e.position);
            let dist = bearing::distance(perspective_pos, pos);
            (dist, entry)
        })
        .collect();
    with_dist.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // Apply token budget (nearest-first truncation)
    let entities_json: Vec<serde_json::Value> = with_dist.into_iter().map(|(_, e)| e).collect();
    let total = entities_json.len();
    let truncated_entities = budget_truncate(&entities_json, budget_limit);
    let showing = truncated_entities.len();

    let mut response = json!({
        "frame": frame,
        "timestamp_ms": timestamp_ms,
        "source": "recording",
        "entities": truncated_entities,
        "total_entities": total,
    });

    if showing < total {
        response["pagination"] = json!({
            "truncated": true,
            "showing": showing,
            "total": total,
        });
    }

    Ok(response)
}

/// Truncate entity list to fit within token budget (nearest-first ordering preserved).
fn budget_truncate(entities: &[serde_json::Value], budget_limit: u32) -> Vec<serde_json::Value> {
    let mut result = Vec::new();
    let mut bytes_used: usize = 100; // overhead for frame metadata
    for entity in entities {
        let entity_bytes = serde_json::to_vec(entity).unwrap_or_default().len();
        let entity_tokens = stage_core::budget::estimate_tokens(entity_bytes);
        if stage_core::budget::estimate_tokens(bytes_used) + entity_tokens > budget_limit {
            break;
        }
        bytes_used += entity_bytes;
        result.push(entity.clone());
    }
    if result.is_empty() && !entities.is_empty() {
        // Always include at least one entity
        result.push(entities[0].clone());
    }
    result
}
```

**Implementation Notes:**
- `snapshot_at` returns the same shape as `spatial_snapshot` responses per CONTRACT.md
- Perspective defaults to origin with north facing (no live camera for recorded data)
- Token budget applied via nearest-first truncation, same as live snapshots
- `detail` parameter controls how much per-entity data is included

#### 3c: query_range

```rust
/// Query condition for range search.
#[derive(Debug, Deserialize)]
pub struct QueryCondition {
    /// Condition type.
    #[serde(rename = "type")]
    pub condition_type: String, // proximity, property_change, signal_emitted, entered_area, velocity_spike, state_transition
    /// Target node for proximity conditions.
    pub target: Option<String>,
    /// Distance or velocity threshold.
    pub threshold: Option<f64>,
    /// Property name for property_change / state_transition.
    pub property: Option<String>,
    /// Signal name for signal_emitted.
    pub signal: Option<String>,
}

/// A single matching result from query_range.
#[derive(Debug, Serialize)]
pub struct RangeMatch {
    pub frame: u64,
    pub time_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_pos: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_velocity: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Search across a frame range for frames matching a spatial condition.
pub fn query_range(
    db: &Connection,
    node: &str,
    from_frame: u64,
    to_frame: u64,
    condition: &QueryCondition,
    budget_limit: u32,
) -> Result<serde_json::Value, McpError> {
    // 1. Select frames in range
    let mut stmt = db
        .prepare("SELECT frame, timestamp_ms, data FROM frames WHERE frame BETWEEN ?1 AND ?2 ORDER BY frame")
        .map_err(|e| McpError::internal_error(format!("SQLite error: {e}"), None))?;

    let rows = stmt
        .query_map(rusqlite::params![from_frame, to_frame], |row| {
            Ok((
                row.get::<_, u64>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        })
        .map_err(|e| McpError::internal_error(format!("SQLite error: {e}"), None))?;

    let mut matches: Vec<RangeMatch> = Vec::new();
    let mut total_frames: u64 = 0;
    let mut prev_entities: Option<Vec<FrameEntityData>> = None;

    // Tracking for annotations
    let mut first_breach_frame: Option<u64> = None;
    let mut deepest_value: Option<f64> = None;
    let mut deepest_frame: Option<u64> = None;

    for row_result in rows {
        let (frame, time_ms, data) = row_result
            .map_err(|e| McpError::internal_error(format!("SQLite error: {e}"), None))?;
        total_frames += 1;

        let entities: Vec<FrameEntityData> = rmp_serde::from_slice(&data).map_err(|e| {
            McpError::internal_error(format!("MessagePack decode error at frame {frame}: {e}"), None)
        })?;

        if let Some(range_match) = evaluate_condition(
            frame, time_ms, node, &entities, &condition, &prev_entities,
        ) {
            // Track first_breach and deepest_penetration for proximity
            if condition.condition_type == "proximity" {
                if let Some(dist) = range_match.distance {
                    if first_breach_frame.is_none() {
                        first_breach_frame = Some(frame);
                    }
                    if deepest_value.is_none() || dist < deepest_value.unwrap() {
                        deepest_value = Some(dist);
                        deepest_frame = Some(frame);
                    }
                }
            }
            matches.push(range_match);
        }

        prev_entities = Some(entities);
    }

    // Annotate first_breach and deepest_penetration
    if let Some(first_frame) = first_breach_frame {
        for m in &mut matches {
            if m.frame == first_frame && m.note.is_none() {
                m.note = Some("first_breach".into());
            }
        }
    }
    if let Some(deep_frame) = deepest_frame {
        for m in &mut matches {
            if m.frame == deep_frame {
                m.note = Some("deepest_penetration".into());
            }
        }
    }

    // Token budget: truncate matches if too many
    let showing = budget_truncate_count(&matches, budget_limit);
    let total_matching = matches.len();
    matches.truncate(showing);

    Ok(json!({
        "query": condition.condition_type,
        "node": node,
        "target": condition.target,
        "threshold": condition.threshold,
        "results": matches,
        "total_frames_in_range": total_frames,
        "frames_matching": total_matching,
    }))
}

/// Evaluate a single condition against a frame's entity data.
/// Returns Some(RangeMatch) if the condition is met, None otherwise.
fn evaluate_condition(
    frame: u64,
    time_ms: u64,
    node: &str,
    entities: &[FrameEntityData],
    condition: &QueryCondition,
    prev_entities: &Option<Vec<FrameEntityData>>,
) -> Option<RangeMatch> {
    match condition.condition_type.as_str() {
        "proximity" => evaluate_proximity(frame, time_ms, node, entities, condition),
        "velocity_spike" => evaluate_velocity_spike(frame, time_ms, node, entities, prev_entities, condition),
        "property_change" => evaluate_property_change(frame, time_ms, node, entities, prev_entities, condition),
        "signal_emitted" => None, // Handled via events table, not frame data
        "state_transition" => evaluate_state_transition(frame, time_ms, node, entities, prev_entities, condition),
        _ => None,
    }
}
```

**Condition evaluator functions:**

```rust
fn evaluate_proximity(
    frame: u64,
    time_ms: u64,
    node: &str,
    entities: &[FrameEntityData],
    condition: &QueryCondition,
) -> Option<RangeMatch> {
    let threshold = condition.threshold.unwrap_or(1.0);
    let target_pattern = condition.target.as_deref()?;

    let node_entity = entities.iter().find(|e| e.path == node)?;
    let node_pos = stage_core::types::vec_to_array3(&node_entity.position);

    // Find closest matching target (supports glob-style trailing wildcard)
    let mut min_dist = f64::MAX;
    for entity in entities {
        if !path_matches(&entity.path, target_pattern) {
            continue;
        }
        let target_pos = stage_core::types::vec_to_array3(&entity.position);
        let dist = stage_core::bearing::distance(node_pos, target_pos);
        if dist < min_dist {
            min_dist = dist;
        }
    }

    if min_dist <= threshold {
        Some(RangeMatch {
            frame,
            time_ms,
            distance: Some(min_dist),
            node_pos: Some(node_entity.position.clone()),
            node_velocity: Some(node_entity.velocity.clone()),
            note: None,
        })
    } else {
        None
    }
}

fn evaluate_velocity_spike(
    frame: u64,
    time_ms: u64,
    node: &str,
    entities: &[FrameEntityData],
    prev_entities: &Option<Vec<FrameEntityData>>,
    condition: &QueryCondition,
) -> Option<RangeMatch> {
    let threshold = condition.threshold.unwrap_or(5.0);
    let prev = prev_entities.as_ref()?;

    let curr = entities.iter().find(|e| e.path == node)?;
    let prev_entity = prev.iter().find(|e| e.path == node)?;

    let curr_speed: f64 = curr.velocity.iter().map(|v| v * v).sum::<f64>().sqrt();
    let prev_speed: f64 = prev_entity.velocity.iter().map(|v| v * v).sum::<f64>().sqrt();
    let delta_speed = (curr_speed - prev_speed).abs();

    if delta_speed >= threshold {
        Some(RangeMatch {
            frame,
            time_ms,
            distance: None,
            node_pos: Some(curr.position.clone()),
            node_velocity: Some(curr.velocity.clone()),
            note: Some(format!("velocity: {prev_speed:.1} -> {curr_speed:.1}")),
        })
    } else {
        None
    }
}

fn evaluate_property_change(
    frame: u64,
    time_ms: u64,
    node: &str,
    entities: &[FrameEntityData],
    prev_entities: &Option<Vec<FrameEntityData>>,
    condition: &QueryCondition,
) -> Option<RangeMatch> {
    let property = condition.property.as_deref()?;
    let prev = prev_entities.as_ref()?;

    let curr = entities.iter().find(|e| e.path == node)?;
    let prev_entity = prev.iter().find(|e| e.path == node)?;

    let curr_val = curr.state.get(property)?;
    let prev_val = prev_entity.state.get(property)?;

    if !stage_core::delta::values_equal(prev_val, curr_val) {
        Some(RangeMatch {
            frame,
            time_ms,
            distance: None,
            node_pos: Some(curr.position.clone()),
            node_velocity: None,
            note: Some(format!("{property}: {prev_val} -> {curr_val}")),
        })
    } else {
        None
    }
}

fn evaluate_state_transition(
    frame: u64,
    time_ms: u64,
    node: &str,
    entities: &[FrameEntityData],
    prev_entities: &Option<Vec<FrameEntityData>>,
    condition: &QueryCondition,
) -> Option<RangeMatch> {
    // Same as property_change but checks for string state transitions specifically
    evaluate_property_change(frame, time_ms, node, entities, prev_entities, condition)
}

/// Simple glob matching for target patterns like "walls/*".
fn path_matches(path: &str, pattern: &str) -> bool {
    if pattern.ends_with("/*") {
        let prefix = &pattern[..pattern.len() - 2];
        path.starts_with(prefix) && path.len() > prefix.len() && path.as_bytes()[prefix.len()] == b'/'
    } else if pattern.ends_with('*') {
        path.starts_with(&pattern[..pattern.len() - 1])
    } else {
        path == pattern
    }
}

/// Count how many results fit in the token budget.
fn budget_truncate_count(matches: &[RangeMatch], budget_limit: u32) -> usize {
    let mut bytes = 100; // overhead
    for (i, m) in matches.iter().enumerate() {
        let entry_bytes = serde_json::to_vec(m).unwrap_or_default().len();
        bytes += entry_bytes;
        if stage_core::budget::estimate_tokens(bytes) > budget_limit {
            return i.max(1); // at least 1 result
        }
    }
    matches.len()
}
```

#### 3d: diff_frames

```rust
/// Compare spatial state between two recorded frames.
pub fn diff_frames(
    db: &Connection,
    frame_a: u64,
    frame_b: u64,
    budget_limit: u32,
) -> Result<serde_json::Value, McpError> {
    let entities_a = read_frame(db, frame_a)?;
    let entities_b = read_frame(db, frame_b)?;

    let ts_a: u64 = db
        .query_row("SELECT timestamp_ms FROM frames WHERE frame = ?1", rusqlite::params![frame_a], |r| r.get(0))
        .unwrap_or(0);
    let ts_b: u64 = db
        .query_row("SELECT timestamp_ms FROM frames WHERE frame = ?1", rusqlite::params![frame_b], |r| r.get(0))
        .unwrap_or(0);

    // Build lookup maps
    let map_a: HashMap<&str, &FrameEntityData> =
        entities_a.iter().map(|e| (e.path.as_str(), e)).collect();
    let map_b: HashMap<&str, &FrameEntityData> =
        entities_b.iter().map(|e| (e.path.as_str(), e)).collect();

    let mut changes: Vec<serde_json::Value> = Vec::new();
    let mut unchanged_count: usize = 0;

    // Compare entities present in both frames
    for (path, b_entity) in &map_b {
        if let Some(a_entity) = map_a.get(path) {
            let mut entry = json!({ "path": path });
            let mut has_change = false;

            // Position change
            let pos_a = stage_core::types::vec_to_array3(&a_entity.position);
            let pos_b = stage_core::types::vec_to_array3(&b_entity.position);
            let dist = stage_core::bearing::distance(pos_a, pos_b);
            if dist > stage_core::delta::POSITION_THRESHOLD {
                entry["position"] = json!({ "a": a_entity.position, "b": b_entity.position });
                entry["delta_pos"] = json!([
                    pos_b[0] - pos_a[0],
                    pos_b[1] - pos_a[1],
                    pos_b[2] - pos_a[2],
                ]);
                has_change = true;
            }

            // State changes
            let mut state_changes = serde_json::Map::new();
            for (key, val_b) in &b_entity.state {
                match a_entity.state.get(key) {
                    Some(val_a) if !stage_core::delta::values_equal(val_a, val_b) => {
                        state_changes.insert(key.clone(), json!({ "a": val_a, "b": val_b }));
                        has_change = true;
                    }
                    None => {
                        state_changes.insert(key.clone(), json!({ "a": null, "b": val_b }));
                        has_change = true;
                    }
                    _ => {}
                }
            }
            for (key, val_a) in &a_entity.state {
                if !b_entity.state.contains_key(key) {
                    state_changes.insert(key.clone(), json!({ "a": val_a, "b": null }));
                    has_change = true;
                }
            }
            if !state_changes.is_empty() {
                entry["state"] = json!(state_changes);
            }

            if has_change {
                changes.push(entry);
            } else {
                unchanged_count += 1;
            }
        }
        // Entities only in B are implicitly "entered" — not shown in diff per CONTRACT
    }

    // Query markers between frames
    let markers = query_markers_between(db, frame_a, frame_b)?;

    Ok(json!({
        "frame_a": frame_a,
        "frame_b": frame_b,
        "dt_ms": ts_b.saturating_sub(ts_a),
        "changes": changes,
        "nodes_unchanged": unchanged_count,
        "markers_between": markers,
    }))
}

/// Query markers between two frames (inclusive of both endpoints).
fn query_markers_between(
    db: &Connection,
    frame_a: u64,
    frame_b: u64,
) -> Result<Vec<serde_json::Value>, McpError> {
    let mut stmt = db
        .prepare("SELECT frame, timestamp_ms, source, label FROM markers WHERE frame BETWEEN ?1 AND ?2 ORDER BY frame")
        .map_err(|e| McpError::internal_error(format!("SQLite error: {e}"), None))?;

    let rows = stmt
        .query_map(rusqlite::params![frame_a, frame_b], |row| {
            Ok(json!({
                "frame": row.get::<_, i64>(0)?,
                "source": row.get::<_, String>(2)?,
                "label": row.get::<_, String>(3)?,
            }))
        })
        .map_err(|e| McpError::internal_error(format!("SQLite error: {e}"), None))?;

    let markers: Vec<serde_json::Value> = rows.flatten().collect();
    Ok(markers)
}
```

#### 3e: find_event

```rust
/// Search the recording timeline for specific event types.
pub fn find_event(
    db: &Connection,
    recording_id: &str,
    event_type: &str,
    event_filter: Option<&str>,
    node: Option<&str>,
    from_frame: Option<u64>,
    to_frame: Option<u64>,
    budget_limit: u32,
) -> Result<serde_json::Value, McpError> {
    // For "marker" event type, search markers table
    if event_type == "marker" {
        return find_markers(db, recording_id, event_filter, from_frame, to_frame);
    }

    // For "signal" and other event types stored in events table
    let mut sql = String::from(
        "SELECT frame, event_type, node_path, data FROM events WHERE event_type = ?1"
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(event_type.to_string())];
    let mut param_idx = 2;

    if let Some(node_path) = node {
        sql.push_str(&format!(" AND node_path = ?{param_idx}"));
        params.push(Box::new(node_path.to_string()));
        param_idx += 1;
    }

    if let Some(from) = from_frame {
        sql.push_str(&format!(" AND frame >= ?{param_idx}"));
        params.push(Box::new(from as i64));
        param_idx += 1;
    }

    if let Some(to) = to_frame {
        sql.push_str(&format!(" AND frame <= ?{param_idx}"));
        params.push(Box::new(to as i64));
        // param_idx += 1; // last param
    }

    sql.push_str(" ORDER BY frame");

    let mut stmt = db.prepare(&sql)
        .map_err(|e| McpError::internal_error(format!("SQLite error: {e}"), None))?;

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        let frame: i64 = row.get(0)?;
        let evt_type: String = row.get(1)?;
        let node_path: String = row.get(2)?;
        let data_str: String = row.get(3)?;
        let data: serde_json::Value = serde_json::from_str(&data_str).unwrap_or(json!(null));
        Ok(json!({
            "frame": frame,
            "event_type": evt_type,
            "node": node_path,
            "data": data,
        }))
    }).map_err(|e| McpError::internal_error(format!("SQLite error: {e}"), None))?;

    let mut events: Vec<serde_json::Value> = Vec::new();
    for row in rows.flatten() {
        // Apply event_filter if present (substring match on signal name or data)
        if let Some(filter) = event_filter {
            let row_str = row.to_string();
            if !row_str.contains(filter) {
                continue;
            }
        }
        events.push(row);
    }

    // Token budget truncation
    let total = events.len();
    let showing = budget_truncate_count_json(&events, budget_limit);
    events.truncate(showing);

    Ok(json!({
        "recording_id": recording_id,
        "event_type": event_type,
        "filter": event_filter,
        "events": events,
        "total_events": total,
        "showing": showing,
    }))
}

fn find_markers(
    db: &Connection,
    recording_id: &str,
    label_filter: Option<&str>,
    from_frame: Option<u64>,
    to_frame: Option<u64>,
) -> Result<serde_json::Value, McpError> {
    let mut sql = String::from("SELECT frame, timestamp_ms, source, label FROM markers WHERE 1=1");
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1;

    if let Some(from) = from_frame {
        sql.push_str(&format!(" AND frame >= ?{idx}"));
        params.push(Box::new(from as i64));
        idx += 1;
    }
    if let Some(to) = to_frame {
        sql.push_str(&format!(" AND frame <= ?{idx}"));
        params.push(Box::new(to as i64));
        idx += 1;
    }
    if let Some(filter) = label_filter {
        sql.push_str(&format!(" AND label LIKE ?{idx}"));
        params.push(Box::new(format!("%{filter}%")));
    }
    sql.push_str(" ORDER BY frame");

    let mut stmt = db.prepare(&sql)
        .map_err(|e| McpError::internal_error(format!("SQLite error: {e}"), None))?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(json!({
            "frame": row.get::<_, i64>(0)?,
            "time_ms": row.get::<_, i64>(1)?,
            "source": row.get::<_, String>(2)?,
            "label": row.get::<_, String>(3)?,
        }))
    }).map_err(|e| McpError::internal_error(format!("SQLite error: {e}"), None))?;

    let events: Vec<serde_json::Value> = rows.flatten().collect();

    Ok(json!({
        "recording_id": recording_id,
        "event_type": "marker",
        "events": events,
    }))
}

fn budget_truncate_count_json(items: &[serde_json::Value], budget_limit: u32) -> usize {
    let mut bytes = 100;
    for (i, item) in items.iter().enumerate() {
        bytes += serde_json::to_vec(item).unwrap_or_default().len();
        if stage_core::budget::estimate_tokens(bytes) > budget_limit {
            return i.max(1);
        }
    }
    items.len()
}
```

**Implementation Notes:**
- `values_equal` from `stage_core::delta` needs to be made `pub` (currently `pub(crate)`)
- `signal_emitted` condition uses the events table, not frame data scanning
- `entered_area` is evaluated via events table (`area_enter` events)
- `velocity_spike` requires two consecutive frames (prev + curr)
- Glob matching for target patterns uses simple prefix matching (sufficient for `walls/*` patterns)

**Acceptance Criteria:**
- [ ] `snapshot_at` returns spatial state at a frame matching CONTRACT.md `snapshot_at` response shape
- [ ] `snapshot_at` supports `at_frame` and `at_time_ms` (nearest frame lookup)
- [ ] `query_range` scans frame range and evaluates proximity conditions correctly
- [ ] `query_range` annotates `first_breach` and `deepest_penetration` on proximity results
- [ ] `query_range` evaluates velocity_spike, property_change, state_transition conditions
- [ ] `diff_frames` returns position changes with old/new + delta_pos
- [ ] `diff_frames` returns state changes with old/new values
- [ ] `diff_frames` includes markers between the two frames
- [ ] `find_event` searches events table by type with optional node and frame range filters
- [ ] `find_event` supports "marker" event type (searches markers table)
- [ ] All analysis functions enforce token budget via truncation
- [ ] `path_matches` correctly handles `walls/*` and exact match patterns

---

### Unit 4: Extend RecordingParams for Analysis Actions

**File**: `crates/stage-server/src/mcp/recording.rs`

Add new fields to `RecordingParams` for the 4 analysis actions:

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordingParams {
    /// Action to perform.
    /// Capture: "start", "stop", "status", "list", "delete", "markers", "add_marker".
    /// Analysis: "snapshot_at", "query_range", "diff_frames", "find_event".
    pub action: String,

    // --- existing M7 fields (unchanged) ---
    pub recording_name: Option<String>,
    pub capture: Option<CaptureConfig>,
    pub recording_id: Option<String>,
    pub marker_label: Option<String>,
    pub marker_frame: Option<u64>,
    pub token_budget: Option<u32>,

    // --- M8 analysis fields ---

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

    /// Event type for find_event: "signal", "property_change", "collision",
    /// "area_enter", "area_exit", "node_added", "node_removed", "marker", "input".
    pub event_type: Option<String>,

    /// Event filter string for find_event (substring match on event data).
    pub event_filter: Option<String>,

    /// Frame A for diff_frames.
    pub frame_a: Option<u64>,

    /// Frame B for diff_frames.
    pub frame_b: Option<u64>,
}
```

**Implementation Notes:**
- `condition` is `Option<serde_json::Value>` (not a typed struct) because JsonSchema derives don't handle the nested enum well. Deserialized into `QueryCondition` inside the handler.
- All new fields are `Option` — they're only relevant for their respective actions.

**Acceptance Criteria:**
- [ ] New params fields deserialize correctly from JSON
- [ ] Existing M7 params remain unchanged and backward-compatible
- [ ] `serde_json::from_value` works for all action combinations
- [ ] JsonSchema generates correct schema for the MCP tool description

---

### Unit 5: Recording Handler — Analysis Action Routing

**File**: `crates/stage-server/src/mcp/recording.rs`

Add 4 new action handlers that use the analysis engine:

```rust
pub async fn handle_recording(
    params: RecordingParams,
    state: &Arc<Mutex<SessionState>>,
) -> Result<String, McpError> {
    let config = crate::tcp::get_config(state).await;
    let hard_cap = config.token_hard_cap;
    let budget_limit = resolve_budget(params.token_budget, 1500, hard_cap);

    match params.action.as_str() {
        // --- M7 capture actions (unchanged) ---
        "start" => handle_start(&params, state, budget_limit, hard_cap).await,
        "stop" => handle_stop(state, budget_limit, hard_cap).await,
        "status" => handle_status(state, budget_limit, hard_cap).await,
        "list" => handle_list(state, budget_limit, hard_cap).await,
        "delete" => handle_delete(&params, state, budget_limit, hard_cap).await,
        "markers" => handle_markers(&params, state, budget_limit, hard_cap).await,
        "add_marker" => handle_add_marker(&params, state, budget_limit, hard_cap).await,

        // --- M8 analysis actions (new) ---
        "snapshot_at" => handle_snapshot_at(&params, state, budget_limit, hard_cap).await,
        "query_range" => handle_query_range(&params, state, budget_limit, hard_cap).await,
        "diff_frames" => handle_diff_frames(&params, state, budget_limit, hard_cap).await,
        "find_event" => handle_find_event(&params, state, budget_limit, hard_cap).await,

        other => Err(McpError::invalid_params(
            format!(
                "Unknown recording action: '{other}'. Valid: start, stop, status, list, \
                 delete, markers, add_marker, snapshot_at, query_range, diff_frames, find_event"
            ),
            None,
        )),
    }
}

async fn handle_snapshot_at(
    params: &RecordingParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let storage_path = recording_analysis::resolve_storage_path(state).await?;
    let recording_id = resolve_recording_id(params, &storage_path)?;
    let db = recording_analysis::open_recording_db(&storage_path, &recording_id)?;

    let frame = if let Some(f) = params.at_frame {
        f
    } else if let Some(t) = params.at_time_ms {
        let (frame, _) = recording_analysis::read_frame_at_time(&db, t)?;
        frame
    } else {
        return Err(McpError::invalid_params(
            "snapshot_at requires at_frame or at_time_ms".into(),
            None,
        ));
    };

    let detail = params.detail.as_deref().unwrap_or("standard");
    let mut response = recording_analysis::snapshot_at(&db, frame, detail, budget_limit, hard_cap)?;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_query_range(
    params: &RecordingParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let storage_path = recording_analysis::resolve_storage_path(state).await?;
    let recording_id = resolve_recording_id(params, &storage_path)?;
    let db = recording_analysis::open_recording_db(&storage_path, &recording_id)?;

    let node = params.node.as_deref().ok_or_else(|| {
        McpError::invalid_params("query_range requires 'node' parameter".into(), None)
    })?;

    let from = params.from_frame.ok_or_else(|| {
        McpError::invalid_params("query_range requires 'from_frame'".into(), None)
    })?;
    let to = params.to_frame.ok_or_else(|| {
        McpError::invalid_params("query_range requires 'to_frame'".into(), None)
    })?;

    let condition: recording_analysis::QueryCondition = params
        .condition
        .as_ref()
        .ok_or_else(|| McpError::invalid_params("query_range requires 'condition'".into(), None))
        .and_then(|v| {
            serde_json::from_value(v.clone()).map_err(|e| {
                McpError::invalid_params(format!("Invalid condition: {e}"), None)
            })
        })?;

    let mut response = recording_analysis::query_range(
        &db, node, from, to, &condition, budget_limit,
    )?;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_diff_frames(
    params: &RecordingParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let storage_path = recording_analysis::resolve_storage_path(state).await?;
    let recording_id = resolve_recording_id(params, &storage_path)?;
    let db = recording_analysis::open_recording_db(&storage_path, &recording_id)?;

    let frame_a = params.frame_a.ok_or_else(|| {
        McpError::invalid_params("diff_frames requires 'frame_a'".into(), None)
    })?;
    let frame_b = params.frame_b.ok_or_else(|| {
        McpError::invalid_params("diff_frames requires 'frame_b'".into(), None)
    })?;

    let mut response = recording_analysis::diff_frames(&db, frame_a, frame_b, budget_limit)?;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_find_event(
    params: &RecordingParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let storage_path = recording_analysis::resolve_storage_path(state).await?;
    let recording_id = resolve_recording_id(params, &storage_path)?;
    let db = recording_analysis::open_recording_db(&storage_path, &recording_id)?;

    let event_type = params.event_type.as_deref().ok_or_else(|| {
        McpError::invalid_params("find_event requires 'event_type'".into(), None)
    })?;

    let mut response = recording_analysis::find_event(
        &db,
        &recording_id,
        event_type,
        params.event_filter.as_deref(),
        params.node.as_deref(),
        params.from_frame,
        params.to_frame,
        budget_limit,
    )?;
    finalize_response(&mut response, budget_limit, hard_cap)
}

/// Resolve recording_id: use explicit param, or find the most recent recording.
fn resolve_recording_id(params: &RecordingParams, storage_path: &str) -> Result<String, McpError> {
    if let Some(ref id) = params.recording_id {
        return Ok(id.clone());
    }
    // Find the most recent recording by file modification time
    most_recent_recording(storage_path).ok_or_else(|| {
        McpError::invalid_params(
            "No recording_id specified and no recordings found".into(),
            None,
        )
    })
}

/// Find the most recently modified .sqlite file in the storage directory.
fn most_recent_recording(storage_path: &str) -> Option<String> {
    let entries = std::fs::read_dir(storage_path).ok()?;
    let mut newest: Option<(std::time::SystemTime, String)> = None;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("sqlite") {
            continue;
        }
        let modified = entry.metadata().ok()?.modified().ok()?;
        let stem = path.file_stem()?.to_str()?.to_string();

        if newest.is_none() || modified > newest.as_ref().unwrap().0 {
            newest = Some((modified, stem));
        }
    }

    newest.map(|(_, id)| id)
}
```

**Acceptance Criteria:**
- [ ] All 4 analysis actions route correctly from the action dispatcher
- [ ] `recording_id` defaults to most recent recording when omitted
- [ ] Parameter validation produces clear error messages with required field names
- [ ] `condition` JSON is deserialized into `QueryCondition` with helpful error on malformed input
- [ ] Each handler calls `finalize_response` for budget injection
- [ ] Invalid action names list all 11 valid actions in the error message

---

### Unit 6: Activity Logging for Analysis Actions

**File**: `crates/stage-server/src/activity.rs`

Extend `recording_summary` to cover the 4 new actions:

```rust
pub fn recording_summary(params: &RecordingParams) -> String {
    match params.action.as_str() {
        // --- M7 (existing) ---
        "start" => { /* unchanged */ }
        "stop" => "Stopped recording".into(),
        "status" => "Checking recording status".into(),
        "list" => "Listing recordings".into(),
        "delete" => { /* unchanged */ }
        "markers" => { /* unchanged */ }
        "add_marker" => { /* unchanged */ }

        // --- M8 (new) ---
        "snapshot_at" => {
            let frame_info = if let Some(f) = params.at_frame {
                format!("frame {f}")
            } else if let Some(t) = params.at_time_ms {
                format!("{t}ms")
            } else {
                "?".into()
            };
            let rec = params.recording_id.as_deref().unwrap_or("latest");
            format!("Snapshot at {frame_info} in {rec}")
        }
        "query_range" => {
            let from = params.from_frame.map(|f| f.to_string()).unwrap_or("?".into());
            let to = params.to_frame.map(|f| f.to_string()).unwrap_or("?".into());
            let node = params.node.as_deref().unwrap_or("?");
            format!("Query range {from}-{to} for {node}")
        }
        "diff_frames" => {
            let a = params.frame_a.map(|f| f.to_string()).unwrap_or("?".into());
            let b = params.frame_b.map(|f| f.to_string()).unwrap_or("?".into());
            format!("Diff frames {a} vs {b}")
        }
        "find_event" => {
            let evt = params.event_type.as_deref().unwrap_or("?");
            let filter = params.event_filter.as_deref().unwrap_or("");
            if filter.is_empty() {
                format!("Find events: {evt}")
            } else {
                format!("Find events: {evt} filter={filter}")
            }
        }

        other => format!("Recording: {other}"),
    }
}
```

**Implementation Notes:**
- Activity types for analysis actions use `"recording"` entry_type (same as M7 capture actions)
- The dock shows these in the Agent Activity Feed with informational (blue) styling

**Acceptance Criteria:**
- [ ] All 4 analysis actions produce human-readable activity summaries
- [ ] Activity messages include relevant context (frame numbers, node names, recording IDs)
- [ ] Summaries are concise (fit on one dock feed line)

---

### Unit 7: Update MCP Tool Description

**File**: `crates/stage-server/src/mcp/mod.rs`

Update the `recording` tool's `#[tool(description = "...")]`:

```rust
#[tool(description = "Capture and analyze play session recordings. \
    Capture: 'start' (begin recording), 'stop' (end recording), 'status' (check state), \
    'list' (saved recordings), 'delete' (remove by recording_id), 'markers' (list markers), \
    'add_marker' (agent marker). \
    Analysis: 'snapshot_at' (spatial state at frame/time, requires at_frame or at_time_ms), \
    'query_range' (search frame range with condition, requires node + from_frame + to_frame + condition), \
    'diff_frames' (compare two frames, requires frame_a + frame_b), \
    'find_event' (search events by type, requires event_type). \
    Analysis defaults to most recent recording if recording_id is omitted.")]
```

**Acceptance Criteria:**
- [ ] Tool description lists all 11 actions with their required parameters
- [ ] Agent can discover analysis capabilities from the tool description alone

---

### Unit 8: Dependencies

**File**: `crates/stage-server/Cargo.toml`

```toml
[dependencies]
# ... existing deps ...
rusqlite = { workspace = true }
rmp-serde = { workspace = true }
```

**File**: `crates/stage-protocol/Cargo.toml`

Verify `serde` and `serde_json` workspace dependencies are present (they are — needed for `FrameEntityData`).

**Implementation Notes:**
- Both `rusqlite` and `rmp-serde` are already workspace dependencies (used by stage-godot)
- `rusqlite` bundled feature ensures SQLite is statically linked (no system dependency)

**Acceptance Criteria:**
- [ ] `cargo build -p stage-server` compiles with new dependencies
- [ ] No duplicate SQLite library versions in the dependency tree

---

### Unit 9: Make `values_equal` Public

**File**: `crates/stage-core/src/delta.rs`

Change visibility of `values_equal` from `pub(crate)` to `pub`:

```rust
/// Compare two JSON values with float thresholds.
pub fn values_equal(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    // ... unchanged implementation
}
```

Also export `POSITION_THRESHOLD` as `pub`:

```rust
pub const POSITION_THRESHOLD: f64 = 0.01;
```

**Acceptance Criteria:**
- [ ] `stage_core::delta::values_equal` is callable from stage-server
- [ ] `stage_core::delta::POSITION_THRESHOLD` is accessible from stage-server
- [ ] No changes to the function behavior

---

### Unit 10: System Marker Generation

**File**: `crates/stage-server/src/recording_analysis.rs`

System markers are generated as a side effect of `query_range` scans. When a velocity spike or property threshold crossing is detected, a system marker is inserted into the recording's SQLite database.

```rust
/// Insert a system-generated marker into the recording.
/// Opens a separate write connection (WAL mode supports concurrent read+write).
fn insert_system_marker(
    storage_path: &str,
    recording_id: &str,
    frame: u64,
    timestamp_ms: u64,
    label: &str,
) {
    let db_path = format!("{}/{}.sqlite", storage_path, recording_id);
    let db = match Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE,
    ) {
        Ok(db) => db,
        Err(e) => {
            tracing::warn!("Failed to open recording for system marker: {e}");
            return;
        }
    };

    let _ = db.execute(
        "INSERT INTO markers (frame, timestamp_ms, source, label) VALUES (?1, ?2, 'system', ?3)",
        rusqlite::params![frame as i64, timestamp_ms as i64, label],
    );
}
```

**Integration with query_range:**

During `query_range` with `velocity_spike` condition, when a spike is detected:
```rust
// Inside evaluate_velocity_spike, after detecting a spike:
// The caller (query_range) can collect detected spikes and batch-insert markers
// after the scan completes.
```

Rather than inserting during the scan (which holds a read connection), the `query_range` function collects system marker candidates and inserts them after the read is complete:

```rust
// At the end of query_range, after all frame scanning:
if condition.condition_type == "velocity_spike" || condition.condition_type == "proximity" {
    for m in &matches {
        if let Some(ref note) = m.note {
            if note.starts_with("velocity:") || note == "first_breach" || note == "deepest_penetration" {
                insert_system_marker(
                    storage_path,
                    &recording_id,
                    m.frame,
                    m.time_ms,
                    &format!("{}: {}", condition.condition_type, note),
                );
            }
        }
    }
}
```

**Implementation Notes:**
- System markers are post-hoc — generated during analysis, not during capture
- WAL mode allows the read connection (scan) and write connection (marker insert) to coexist
- Only significant findings get system markers (first_breach, deepest_penetration, velocity spikes)
- System markers are idempotent: duplicate labels at the same frame are acceptable (SQLite AUTOINCREMENT)

**Acceptance Criteria:**
- [ ] Velocity spike detection inserts system markers with `source = "system"`
- [ ] Proximity `first_breach` and `deepest_penetration` annotations become system markers
- [ ] System markers are visible via `recording(action: "markers")` after analysis
- [ ] Marker insertion failures are logged but don't fail the analysis query

---

### Unit 11: TCP Dispatch for recording_resolve_path

**File**: `crates/stage-godot/src/tcp_server.rs`

Add routing for the new TCP method in the message dispatch:

```rust
// In the method dispatch section (around line 235-250):
"recording_resolve_path" => {
    recording_handler::handle_recording_query(recorder, method, params)
}
```

**File**: `crates/stage-godot/src/recording_handler.rs`

Add the method to the dispatcher:

```rust
pub fn handle_recording_query(
    recorder: &mut Gd<StageRecorder>,
    method: &str,
    params: &Value,
) -> Result<Value, (String, String)> {
    match method {
        // ... existing M7 methods ...
        "recording_resolve_path" => handle_resolve_path(params),
        other => Err(("method_not_found".into(), format!("Unknown method: {other}"))),
    }
}
```

**Acceptance Criteria:**
- [ ] `recording_resolve_path` TCP method is dispatched correctly
- [ ] Returns the filesystem path as `{ "path": "/home/user/.local/share/godot/..." }`

---

## Implementation Order

1. **Unit 8: Dependencies** — add rusqlite, rmp-serde to stage-server Cargo.toml
2. **Unit 1: FrameEntityData** — move to stage-protocol, update imports
3. **Unit 9: values_equal visibility** — make pub for cross-crate use
4. **Unit 11: TCP dispatch** — add recording_resolve_path route in addon
5. **Unit 2: Storage path resolution** — addon handler + server caching
6. **Unit 3: Recording analysis engine** — core analysis module (snapshot_at, query_range, diff_frames, find_event)
7. **Unit 4: Extended RecordingParams** — add analysis fields
8. **Unit 5: Handler routing** — wire analysis actions to engine
9. **Unit 6: Activity logging** — extend recording_summary
10. **Unit 7: Tool description** — update MCP tool description
11. **Unit 10: System markers** — post-hoc marker insertion

**Rationale:** Dependencies and shared types first (8, 1, 9), then infrastructure (11, 2), then the core engine (3), then the MCP surface (4, 5, 6, 7), and finally the enhancement (10).

---

## Testing

### Unit Tests: `crates/stage-protocol/src/recording.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_entity_data_roundtrips_msgpack() {
        // Same as existing test in recorder.rs — validates shared type
    }
}
```

### Unit Tests: `crates/stage-server/src/recording_analysis.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    // Schema SQL (import from somewhere shared, or inline for tests)
    const SCHEMA_SQL: &str = "..."; // Same as recorder.rs

    fn test_db() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA_SQL).unwrap();
        db
    }

    fn test_entity(path: &str, pos: [f64; 3]) -> FrameEntityData {
        FrameEntityData {
            path: path.into(),
            class: "CharacterBody3D".into(),
            position: pos.to_vec(),
            rotation_deg: vec![0.0, 0.0, 0.0],
            velocity: vec![0.0, 0.0, 0.0],
            groups: vec![],
            visible: true,
            state: serde_json::Map::new(),
        }
    }

    fn test_entity_with_state(path: &str, pos: [f64; 3], state: &[(&str, serde_json::Value)]) -> FrameEntityData {
        let mut e = test_entity(path, pos);
        e.state = state.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        e
    }

    fn test_entity_with_velocity(path: &str, pos: [f64; 3], vel: [f64; 3]) -> FrameEntityData {
        let mut e = test_entity(path, pos);
        e.velocity = vel.to_vec();
        e
    }

    fn insert_frame(db: &Connection, frame: u64, ts_ms: u64, entities: &[FrameEntityData]) {
        let data = rmp_serde::to_vec(entities).unwrap();
        db.execute(
            "INSERT INTO frames (frame, timestamp_ms, data) VALUES (?1, ?2, ?3)",
            rusqlite::params![frame, ts_ms, &data],
        ).unwrap();
    }

    fn insert_recording(db: &Connection, id: &str) {
        db.execute(
            "INSERT INTO recording (id, name, started_at_frame, started_at_ms) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, "test", 100i64, 1000i64],
        ).unwrap();
    }

    // --- read_frame tests ---

    #[test]
    fn read_frame_returns_entities() {
        let db = test_db();
        let entities = vec![test_entity("enemy", [1.0, 0.0, 0.0])];
        insert_frame(&db, 100, 1667, &entities);
        let result = read_frame(&db, 100).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "enemy");
    }

    #[test]
    fn read_frame_missing_returns_error() {
        let db = test_db();
        let result = read_frame(&db, 999);
        assert!(result.is_err());
    }

    // --- snapshot_at tests ---

    #[test]
    fn snapshot_at_returns_entities_sorted_by_distance() {
        let db = test_db();
        insert_frame(&db, 100, 1000, &[
            test_entity("far", [20.0, 0.0, 0.0]),
            test_entity("near", [1.0, 0.0, 0.0]),
        ]);
        let response = snapshot_at(&db, 100, "standard", 5000, 5000).unwrap();
        let entities = response["entities"].as_array().unwrap();
        assert_eq!(entities[0]["path"], "near");
        assert_eq!(entities[1]["path"], "far");
    }

    // --- diff_frames tests ---

    #[test]
    fn diff_frames_detects_position_change() {
        let db = test_db();
        insert_frame(&db, 100, 1000, &[test_entity("enemy", [0.0, 0.0, 0.0])]);
        insert_frame(&db, 110, 1167, &[test_entity("enemy", [5.0, 0.0, 0.0])]);

        let response = diff_frames(&db, 100, 110, 5000).unwrap();
        let changes = response["changes"].as_array().unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0]["path"], "enemy");
        assert!(changes[0]["position"].is_object());
        assert!(changes[0]["delta_pos"].is_array());
    }

    #[test]
    fn diff_frames_detects_state_change() {
        let db = test_db();
        insert_frame(&db, 100, 1000, &[
            test_entity_with_state("enemy", [0.0, 0.0, 0.0], &[("health", serde_json::json!(100))]),
        ]);
        insert_frame(&db, 110, 1167, &[
            test_entity_with_state("enemy", [0.0, 0.0, 0.0], &[("health", serde_json::json!(50))]),
        ]);

        let response = diff_frames(&db, 100, 110, 5000).unwrap();
        let changes = response["changes"].as_array().unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0]["state"]["health"].is_object());
    }

    #[test]
    fn diff_frames_includes_markers_between() {
        let db = test_db();
        insert_frame(&db, 100, 1000, &[test_entity("a", [0.0, 0.0, 0.0])]);
        insert_frame(&db, 110, 1167, &[test_entity("a", [0.0, 0.0, 0.0])]);
        db.execute(
            "INSERT INTO markers (frame, timestamp_ms, source, label) VALUES (105, 1083, 'human', 'bug')",
            [],
        ).unwrap();

        let response = diff_frames(&db, 100, 110, 5000).unwrap();
        let markers = response["markers_between"].as_array().unwrap();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0]["label"], "bug");
    }

    // --- query_range tests ---

    #[test]
    fn query_range_proximity_finds_breach() {
        let db = test_db();
        insert_frame(&db, 100, 1000, &[
            test_entity("enemy", [10.0, 0.0, 0.0]),
            test_entity("wall", [0.0, 0.0, 0.0]),
        ]);
        insert_frame(&db, 101, 1017, &[
            test_entity("enemy", [0.3, 0.0, 0.0]),
            test_entity("wall", [0.0, 0.0, 0.0]),
        ]);

        let condition = QueryCondition {
            condition_type: "proximity".into(),
            target: Some("wall".into()),
            threshold: Some(0.5),
            property: None,
            signal: None,
        };

        let response = query_range(&db, "enemy", 100, 101, &condition, 5000).unwrap();
        let results = response["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["frame"], 101);
        assert_eq!(results[0]["note"], "first_breach");
    }

    #[test]
    fn query_range_velocity_spike() {
        let db = test_db();
        insert_frame(&db, 100, 1000, &[
            test_entity_with_velocity("enemy", [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        ]);
        insert_frame(&db, 101, 1017, &[
            test_entity_with_velocity("enemy", [1.0, 0.0, 0.0], [10.0, 0.0, 0.0]),
        ]);

        let condition = QueryCondition {
            condition_type: "velocity_spike".into(),
            target: None,
            threshold: Some(5.0),
            property: None,
            signal: None,
        };

        let response = query_range(&db, "enemy", 100, 101, &condition, 5000).unwrap();
        let results = response["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
    }

    // --- find_event tests ---

    #[test]
    fn find_event_searches_events_table() {
        let db = test_db();
        insert_frame(&db, 100, 1000, &[]); // FK target
        db.execute(
            "INSERT INTO events (frame, event_type, node_path, data) VALUES (100, 'signal', 'enemy', '{\"signal\":\"hit\"}')",
            [],
        ).unwrap();

        let response = find_event(&db, "rec_1", "signal", Some("hit"), None, None, None, 5000).unwrap();
        let events = response["events"].as_array().unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn find_event_marker_type_searches_markers_table() {
        let db = test_db();
        insert_frame(&db, 100, 1000, &[]); // FK target
        db.execute(
            "INSERT INTO markers (frame, timestamp_ms, source, label) VALUES (100, 1000, 'human', 'bug here')",
            [],
        ).unwrap();

        let response = find_event(&db, "rec_1", "marker", Some("bug"), None, None, None, 5000).unwrap();
        let events = response["events"].as_array().unwrap();
        assert_eq!(events.len(), 1);
    }

    // --- path_matches tests ---

    #[test]
    fn path_matches_exact() {
        assert!(path_matches("walls/segment_04", "walls/segment_04"));
        assert!(!path_matches("walls/segment_05", "walls/segment_04"));
    }

    #[test]
    fn path_matches_wildcard() {
        assert!(path_matches("walls/segment_04", "walls/*"));
        assert!(!path_matches("enemies/scout", "walls/*"));
        assert!(!path_matches("walls", "walls/*")); // "walls" itself doesn't match "walls/*"
    }

    // --- RecordingParams deserialization tests ---

    #[test]
    fn recording_params_snapshot_at() {
        let json = serde_json::json!({
            "action": "snapshot_at",
            "at_frame": 4575,
            "detail": "standard",
            "recording_id": "rec_001",
        });
        let params: RecordingParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.action, "snapshot_at");
        assert_eq!(params.at_frame, Some(4575));
    }

    #[test]
    fn recording_params_query_range() {
        let json = serde_json::json!({
            "action": "query_range",
            "from_frame": 4570,
            "to_frame": 4590,
            "node": "enemies/guard_01",
            "condition": {
                "type": "proximity",
                "target": "walls/*",
                "threshold": 0.5,
            },
        });
        let params: RecordingParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.action, "query_range");
        assert!(params.condition.is_some());
    }

    #[test]
    fn recording_params_diff_frames() {
        let json = serde_json::json!({
            "action": "diff_frames",
            "frame_a": 3010,
            "frame_b": 3020,
        });
        let params: RecordingParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.frame_a, Some(3010));
        assert_eq!(params.frame_b, Some(3020));
    }

    #[test]
    fn recording_params_find_event() {
        let json = serde_json::json!({
            "action": "find_event",
            "event_type": "signal",
            "event_filter": "health_changed",
            "recording_id": "rec_001",
        });
        let params: RecordingParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.event_type.as_deref(), Some("signal"));
        assert_eq!(params.event_filter.as_deref(), Some("health_changed"));
    }
}
```

### Tests in `crates/stage-server/src/mcp/recording.rs`

Add alongside existing tests:

```rust
// Existing tests remain unchanged. Add:

#[test]
fn recording_params_snapshot_at() { /* see above */ }

#[test]
fn recording_params_query_range() { /* see above */ }

#[test]
fn recording_params_diff_frames() { /* see above */ }

#[test]
fn recording_params_find_event() { /* see above */ }
```

---

## Verification Checklist

```bash
# Build everything
cargo build --workspace

# Run all tests
cargo test --workspace

# Lint
cargo clippy --workspace
cargo fmt --check

# Verify new module compiles
cargo build -p stage-server

# Verify stage-godot still compiles with FrameEntityData import change
cargo build -p stage-godot

# Run recording analysis tests specifically
cargo test -p stage-server recording_analysis

# Run recording param tests
cargo test -p stage-server recording_params

# Verify no SQLite library duplication
cargo tree -p stage-server | grep rusqlite
```

---

## Files Changed Summary

| File | Change Type | Description |
|------|------------|-------------|
| `crates/stage-protocol/src/recording.rs` | **new** | FrameEntityData shared type |
| `crates/stage-protocol/src/lib.rs` | edit | Add `pub mod recording` |
| `crates/stage-server/Cargo.toml` | edit | Add rusqlite, rmp-serde deps |
| `crates/stage-server/src/recording_analysis.rs` | **new** | Core analysis engine (520+ lines) |
| `crates/stage-server/src/mcp/recording.rs` | edit | Extended params + 4 new handlers |
| `crates/stage-server/src/mcp/mod.rs` | edit | Updated tool description, add mod |
| `crates/stage-server/src/main.rs` | edit | Add `mod recording_analysis` |
| `crates/stage-server/src/tcp.rs` | edit | Add `recording_storage_path` to SessionState |
| `crates/stage-server/src/activity.rs` | edit | Extend recording_summary |
| `crates/stage-core/src/delta.rs` | edit | Make `values_equal` and `POSITION_THRESHOLD` pub |
| `crates/stage-godot/src/recorder.rs` | edit | Import FrameEntityData from protocol, make globalize_path pub(crate) |
| `crates/stage-godot/src/recording_handler.rs` | edit | Add `recording_resolve_path` handler |
| `crates/stage-godot/src/tcp_server.rs` | edit | Route `recording_resolve_path` |
