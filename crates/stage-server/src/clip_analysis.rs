use std::collections::HashMap;
use std::sync::Arc;

use rmcp::model::ErrorData as McpError;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;

use stage_protocol::recording::FrameEntityData;

use crate::tcp::{SessionState, query_addon};

// ---------------------------------------------------------------------------
// Storage path resolution
// ---------------------------------------------------------------------------

/// Resolve the recording storage path with offline fallback.
///
/// Resolution order: memory cache → disk cache → TCP addon → error.
/// On successful resolution (from any source), the path is cached in memory
/// and persisted to disk so it survives server restarts.
pub async fn resolve_clip_storage_path(
    state: &Arc<Mutex<SessionState>>,
) -> Result<String, McpError> {
    // 1. Memory cache
    {
        let s = state.lock().await;
        if let Some(ref path) = s.clip_storage_path {
            return Ok(path.clone());
        }
    }

    // 2. Disk cache
    let cache_path = {
        let s = state.lock().await;
        s.project_dir.join(".stage").join("clip_storage_path")
    };
    if let Ok(cached) = std::fs::read_to_string(&cache_path) {
        let path = cached.trim().to_string();
        if !path.is_empty() && std::path::Path::new(&path).is_dir() {
            let mut s = state.lock().await;
            s.clip_storage_path = Some(path.clone());
            return Ok(path);
        }
    }

    // 3. TCP addon
    let tcp_result = query_addon(state, "recording_resolve_path", json!({})).await;
    match tcp_result {
        Ok(data) => {
            let path = data["path"]
                .as_str()
                .ok_or_else(|| {
                    McpError::internal_error("Invalid storage path response".to_string(), None)
                })?
                .to_string();
            // Cache to memory + disk
            persist_clip_storage_path(state, &cache_path, &path).await;
            Ok(path)
        }
        Err(e) => Err(McpError::internal_error(
            format!(
                "Cannot resolve clip storage path. No cached path and addon is not connected: {e}"
            ),
            None,
        )),
    }
}

/// Cache the storage path in memory and persist to disk.
async fn persist_clip_storage_path(
    state: &Arc<Mutex<SessionState>>,
    cache_path: &std::path::Path,
    path: &str,
) {
    {
        let mut s = state.lock().await;
        s.clip_storage_path = Some(path.to_string());
    }
    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(cache_path, path);
}

/// List clips directly from SQLite files on disk (no addon required).
pub fn list_clips_from_disk(storage_path: &str) -> Result<serde_json::Value, McpError> {
    let entries = std::fs::read_dir(storage_path).map_err(|e| {
        McpError::internal_error(format!("Cannot read clip storage directory: {e}"), None)
    })?;

    let mut clips = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("sqlite") {
            continue;
        }
        let clip_id = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let size_kb = entry.metadata().map(|m| m.len() / 1024).unwrap_or(0);

        // Open read-only and extract metadata
        let db = match Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
            Ok(db) => db,
            Err(_) => continue,
        };

        let meta = match read_recording_meta(&db) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let frames_captured: u32 = db
            .query_row("SELECT COUNT(*) FROM frames", [], |row| row.get(0))
            .unwrap_or(0);
        let markers_count: u32 = db
            .query_row("SELECT COUNT(*) FROM markers", [], |row| row.get(0))
            .unwrap_or(0);

        let duration_ms = meta.ended_at_ms.unwrap_or(meta.started_at_ms) - meta.started_at_ms;
        let created_at_unix_ms: i64 = db
            .query_row(
                "SELECT created_at_unix_ms FROM recording LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(meta.started_at_ms);
        let created_at = unix_ms_to_iso8601(created_at_unix_ms);

        // Try to extract dashcam info from capture_config
        let capture_json: Option<String> = db
            .query_row("SELECT capture_config FROM recording LIMIT 1", [], |row| {
                row.get(0)
            })
            .ok();
        let capture: Option<serde_json::Value> =
            capture_json.and_then(|s| serde_json::from_str(&s).ok());
        let dashcam = capture
            .as_ref()
            .and_then(|c| c.get("dashcam").and_then(|v| v.as_bool()))
            .unwrap_or(false);
        let tier = capture
            .as_ref()
            .and_then(|c| c.get("tier").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();

        let mut entry = json!({
            "clip_id": clip_id,
            "name": meta.name,
            "frames_captured": frames_captured,
            "duration_ms": duration_ms,
            "frame_range": [meta.started_at_frame, meta.ended_at_frame],
            "markers_count": markers_count,
            "size_kb": size_kb,
            "created_at": created_at,
            "dashcam": dashcam,
            "tier": tier,
        });

        if let Some(cap) = capture {
            entry["capture"] = cap;
        }

        clips.push(entry);
    }

    // Sort by created_at descending (newest first)
    clips.sort_by(|a, b| {
        let a_time = a["created_at"].as_str().unwrap_or("");
        let b_time = b["created_at"].as_str().unwrap_or("");
        b_time.cmp(a_time)
    });

    Ok(json!({ "clips": clips }))
}

/// List markers for a clip directly from SQLite (no addon required).
pub fn list_markers_from_disk(
    storage_path: &str,
    clip_id: &str,
) -> Result<serde_json::Value, McpError> {
    let db = open_clip_db(storage_path, clip_id)?;
    let mut stmt = db
        .prepare("SELECT frame, timestamp_ms, source, label FROM markers ORDER BY frame")
        .map_err(sqlite_err)?;
    let markers: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(json!({
                "frame": row.get::<_, i64>(0)?,
                "timestamp_ms": row.get::<_, i64>(1)?,
                "source": row.get::<_, String>(2).unwrap_or_default(),
                "label": row.get::<_, String>(3).unwrap_or_default(),
            }))
        })
        .map_err(sqlite_err)?
        .filter_map(|r| r.ok())
        .collect();

    Ok(json!({ "clip_id": clip_id, "markers": markers }))
}

/// Delete a clip's SQLite file from disk (no addon required).
pub fn delete_clip_from_disk(
    storage_path: &str,
    clip_id: &str,
) -> Result<serde_json::Value, McpError> {
    let db_path = format!("{}/{}.sqlite", storage_path, clip_id);
    if !std::path::Path::new(&db_path).exists() {
        return Err(McpError::invalid_params(
            format!("Clip '{clip_id}' not found"),
            None,
        ));
    }
    std::fs::remove_file(&db_path).map_err(|e| {
        McpError::internal_error(format!("Failed to delete clip '{clip_id}': {e}"), None)
    })?;
    Ok(json!({ "result": "ok", "clip_id": clip_id }))
}

fn unix_ms_to_iso8601(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let secs = ms / 1000;
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let h = time_of_day / 3600;
    let m = (time_of_day % 3600) / 60;
    let s = time_of_day % 60;

    let mut days = days_since_epoch;
    let mut year = 1970i64;
    loop {
        let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days: [i64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1i64;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    let day = days + 1;

    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Open a recording's SQLite database read-only.
pub fn open_clip_db(storage_path: &str, clip_id: &str) -> Result<Connection, McpError> {
    let db_path = format!("{}/{}.sqlite", storage_path, clip_id);
    Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| {
        McpError::invalid_params(
            format!("Clip '{clip_id}' not found or unreadable: {e}"),
            None,
        )
    })
}

/// Convert any display-able error into an internal McpError with "SQLite error: " prefix.
fn sqlite_err(e: impl std::fmt::Display) -> McpError {
    McpError::internal_error(format!("SQLite error: {e}"), None)
}

// ---------------------------------------------------------------------------
// Frame deserialization helpers
// ---------------------------------------------------------------------------

/// Read and deserialize a single frame's entity data from SQLite.
pub fn read_frame(db: &Connection, frame: u64) -> Result<Vec<FrameEntityData>, McpError> {
    let data: Vec<u8> = db
        .query_row(
            "SELECT data FROM frames WHERE frame = ?1",
            rusqlite::params![frame],
            |row| row.get(0),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                McpError::invalid_params(format!("Frame {frame} not found in recording"), None)
            }
            other => sqlite_err(other),
        })?;
    rmp_serde::from_slice(&data).map_err(|e| {
        McpError::internal_error(
            format!("MessagePack decode error at frame {frame}: {e}"),
            None,
        )
    })
}

/// Read frame data by timestamp (finds nearest frame).
pub fn read_frame_at_time(
    db: &Connection,
    time_ms: u64,
) -> Result<(u64, Vec<FrameEntityData>), McpError> {
    let (frame, data): (u64, Vec<u8>) = db
        .query_row(
            "SELECT frame, data FROM frames ORDER BY ABS(CAST(timestamp_ms AS INTEGER) - ?1) LIMIT 1",
            rusqlite::params![time_ms as i64],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(sqlite_err)?;
    let entities = rmp_serde::from_slice(&data)
        .map_err(|e| McpError::internal_error(format!("MessagePack decode error: {e}"), None))?;
    Ok((frame, entities))
}

/// Recording metadata from the recording table.
pub struct ClipMeta {
    pub id: String,
    pub name: String,
    pub started_at_frame: i64,
    pub ended_at_frame: Option<i64>,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub scene_dimensions: u32,
    pub physics_ticks_per_sec: u32,
}

impl ClipMeta {
    /// Build a light JSON context object for inclusion in analysis responses.
    pub fn to_context(&self) -> serde_json::Value {
        json!({
            "clip_id": self.id,
            "name": self.name,
            "frame_range": [self.started_at_frame, self.ended_at_frame],
            "dimensions": match self.scene_dimensions { 2 => "2d", 3 => "3d", _ => "mixed" },
        })
    }

    /// Validate that a frame number is within the recording's bounds.
    pub fn validate_frame(&self, frame: u64) -> Result<(), McpError> {
        let start = self.started_at_frame as u64;
        if let Some(end) = self.ended_at_frame {
            let end = end as u64;
            if frame < start || frame > end {
                return Err(McpError::invalid_params(
                    format!("Frame {frame} out of range [{start}-{end}]"),
                    None,
                ));
            }
        }
        Ok(())
    }
}

/// Get recording metadata from the recording table.
pub fn read_recording_meta(db: &Connection) -> Result<ClipMeta, McpError> {
    db.query_row(
        "SELECT id, name, started_at_frame, ended_at_frame, started_at_ms, ended_at_ms, \
         scene_dimensions, physics_ticks_per_sec FROM recording LIMIT 1",
        [],
        |row| {
            Ok(ClipMeta {
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

// ---------------------------------------------------------------------------
// ClipSession
// ---------------------------------------------------------------------------

/// Open recording DB and metadata in one step. Used by all 4 analysis handlers.
pub struct ClipSession {
    pub db: Connection,
    pub meta: ClipMeta,
    pub storage_path: String,
    pub clip_id: String,
}

impl ClipSession {
    pub async fn open(
        state: &Arc<Mutex<SessionState>>,
        clip_id: Option<&str>,
    ) -> Result<Self, McpError> {
        let storage_path = resolve_clip_storage_path(state).await?;
        let clip_id = match clip_id {
            Some(id) => id.to_string(),
            None => most_recent_clip(&storage_path).ok_or_else(|| {
                McpError::invalid_params("No clip_id specified and no clips found", None)
            })?,
        };
        let db = open_clip_db(&storage_path, &clip_id)?;
        let meta = read_recording_meta(&db)?;
        Ok(Self {
            db,
            meta,
            storage_path,
            clip_id,
        })
    }

    pub fn finalize(
        &self,
        response: &mut serde_json::Value,
        budget_limit: u32,
        hard_cap: u32,
    ) -> Result<String, McpError> {
        if let Some(obj) = response.as_object_mut() {
            obj.insert("clip_context".into(), self.meta.to_context());
        }
        crate::mcp::finalize_response(response, budget_limit, hard_cap)
    }
}

/// Find the most recently modified .sqlite file in the storage directory.
/// Public alias for use in handler code that resolves "most recent" clip.
pub fn most_recent_clip_id(storage_path: &str) -> Option<String> {
    most_recent_clip(storage_path)
}

fn most_recent_clip(storage_path: &str) -> Option<String> {
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

// ---------------------------------------------------------------------------
// snapshot_at
// ---------------------------------------------------------------------------

/// Reconstruct spatial state at a specific recorded frame.
/// Returns the same shape as spatial_snapshot (standard detail).
pub fn snapshot_at(
    db: &Connection,
    frame: u64,
    detail: &str,
    budget_limit: u32,
    _hard_cap: u32,
) -> Result<serde_json::Value, McpError> {
    let entities = read_frame(db, frame)?;
    let timestamp_ms: u64 = db
        .query_row(
            "SELECT timestamp_ms FROM frames WHERE frame = ?1",
            rusqlite::params![frame],
            |row| row.get::<_, i64>(0),
        )
        .map(|v| v as u64)
        .unwrap_or(0);

    let perspective_pos: stage_core::types::Position3 = [0.0, 0.0, 0.0];
    let perspective = stage_core::bearing::perspective_from_yaw(perspective_pos, 0.0);

    let mut with_dist: Vec<(f64, serde_json::Value)> = entities
        .iter()
        .map(|e| {
            let pos = stage_core::types::vec_to_array3(&e.position);
            let rel = stage_core::bearing::relative_position(&perspective, pos, !e.visible);
            let dist = rel.distance;

            let mut entry = json!({
                "path": e.path,
                "class": e.class,
                "abs": e.position,
                "groups": e.groups,
                "visible": e.visible,
            });

            if detail != "summary" {
                entry["relative"] = json!({
                    "distance": rel.distance,
                    "bearing": rel.bearing,
                    "bearing_deg": rel.bearing_deg,
                });
                entry["rotation_y_deg"] = json!(e.rotation_deg.get(1).copied().unwrap_or(0.0));
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

            (dist, entry)
        })
        .collect();

    with_dist.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

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
        result.push(entities[0].clone());
    }
    result
}

// ---------------------------------------------------------------------------
// query_range
// ---------------------------------------------------------------------------

/// Query condition for range search.
#[derive(Debug, Deserialize)]
pub struct QueryCondition {
    /// Condition type: proximity, property_change, signal_emitted, entered_area, velocity_spike, state_transition, collision.
    #[serde(rename = "type")]
    pub condition_type: String,
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
#[allow(clippy::too_many_arguments)]
pub fn query_range(
    db: &Connection,
    storage_path: &str,
    clip_id: &str,
    node: &str,
    from_frame: u64,
    to_frame: u64,
    condition: &QueryCondition,
    budget_limit: u32,
) -> Result<serde_json::Value, McpError> {
    let mut stmt = db
        .prepare(
            "SELECT frame, timestamp_ms, data FROM frames \
             WHERE frame BETWEEN ?1 AND ?2 ORDER BY frame",
        )
        .map_err(sqlite_err)?;

    let rows = stmt
        .query_map(rusqlite::params![from_frame, to_frame], |row| {
            Ok((
                row.get::<_, u64>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        })
        .map_err(sqlite_err)?;

    let mut matches: Vec<RangeMatch> = Vec::new();
    let mut total_frames: u64 = 0;
    let mut prev_entities: Option<Vec<FrameEntityData>> = None;

    let mut first_breach_frame: Option<u64> = None;
    let mut deepest_value: Option<f64> = None;
    let mut deepest_frame: Option<u64> = None;

    for row_result in rows {
        let (frame, time_ms, data) = row_result.map_err(sqlite_err)?;
        total_frames += 1;

        let entities: Vec<FrameEntityData> = rmp_serde::from_slice(&data).map_err(|e| {
            McpError::internal_error(
                format!("MessagePack decode error at frame {frame}: {e}"),
                None,
            )
        })?;

        if let Some(range_match) = evaluate_condition(
            db,
            frame,
            time_ms,
            node,
            &entities,
            condition,
            &prev_entities,
        )? {
            if condition.condition_type == "proximity"
                && let Some(dist) = range_match.distance
            {
                if first_breach_frame.is_none() {
                    first_breach_frame = Some(frame);
                }
                if deepest_value.is_none() || dist < deepest_value.unwrap() {
                    deepest_value = Some(dist);
                    deepest_frame = Some(frame);
                }
            }
            matches.push(range_match);
        }

        prev_entities = Some(entities);
    }

    // Annotate first_breach
    if let Some(first_frame) = first_breach_frame {
        for m in &mut matches {
            if m.frame == first_frame && m.note.is_none() {
                m.note = Some("first_breach".into());
            }
        }
    }
    // Annotate deepest_penetration (don't overwrite first_breach if same frame)
    if let Some(deep_frame) = deepest_frame {
        for m in &mut matches {
            if m.frame == deep_frame && m.note.is_none() {
                m.note = Some("deepest_penetration".into());
            }
        }
    }

    // Insert system markers for significant findings
    let marker_types = ["velocity_spike", "proximity", "collision"];
    if marker_types.contains(&condition.condition_type.as_str()) {
        for m in &matches {
            if let Some(ref note) = m.note {
                insert_system_marker(
                    storage_path,
                    clip_id,
                    m.frame,
                    m.time_ms,
                    &format!("{}: {}", condition.condition_type, note),
                );
            }
        }
    }

    let total_matching = matches.len();
    let showing = budget_truncate_count(&matches, budget_limit);
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

fn evaluate_condition(
    db: &Connection,
    frame: u64,
    time_ms: u64,
    node: &str,
    entities: &[FrameEntityData],
    condition: &QueryCondition,
    prev_entities: &Option<Vec<FrameEntityData>>,
) -> Result<Option<RangeMatch>, McpError> {
    match condition.condition_type.as_str() {
        "proximity" => Ok(evaluate_proximity(
            frame, time_ms, node, entities, condition,
        )),
        "velocity_spike" => Ok(evaluate_velocity_spike(
            frame,
            time_ms,
            node,
            entities,
            prev_entities,
            condition,
        )),
        "property_change" => Ok(evaluate_property_change(
            frame,
            time_ms,
            node,
            entities,
            prev_entities,
            condition,
        )),
        "state_transition" => Ok(evaluate_property_change(
            frame,
            time_ms,
            node,
            entities,
            prev_entities,
            condition,
        )),
        "signal_emitted" => Ok(evaluate_signal_emitted(db, frame, time_ms, node, condition)),
        "entered_area" => Ok(evaluate_entered_area(db, frame, time_ms, node)),
        "collision" => Ok(evaluate_collision(db, frame, time_ms, node)),
        "moved" => Ok(evaluate_moved(
            frame,
            time_ms,
            node,
            entities,
            prev_entities,
            condition,
        )),
        other => Err(McpError::invalid_params(
            format!(
                "Unknown condition type '{}'. Valid types: moved, proximity, velocity_spike, \
                 property_change, state_transition, signal_emitted, entered_area, collision",
                other
            ),
            None,
        )),
    }
}

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

    let cur = entities.iter().find(|e| e.path == node)?;
    let old = prev.iter().find(|e| e.path == node)?;

    let dx: f64 = cur
        .position
        .iter()
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

fn evaluate_signal_emitted(
    db: &Connection,
    frame: u64,
    time_ms: u64,
    node: &str,
    condition: &QueryCondition,
) -> Option<RangeMatch> {
    let mut sql = String::from(
        "SELECT 1 FROM events WHERE event_type = 'signal' AND frame = ?1 AND node_path = ?2",
    );
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
        vec![Box::new(frame as i64), Box::new(node.to_string())];

    if let Some(ref signal_name) = condition.signal {
        sql.push_str(" AND data LIKE ?3");
        params.push(Box::new(format!("%\"signal\":\"{signal_name}\"%")));
    }
    sql.push_str(" LIMIT 1");

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let found: bool = db
        .query_row(&sql, param_refs.as_slice(), |_| Ok(true))
        .unwrap_or(false);

    if found {
        Some(RangeMatch {
            frame,
            time_ms,
            distance: None,
            node_pos: None,
            node_velocity: None,
            note: Some(format!(
                "signal: {}",
                condition.signal.as_deref().unwrap_or("(any)")
            )),
        })
    } else {
        None
    }
}

fn evaluate_entered_area(
    db: &Connection,
    frame: u64,
    time_ms: u64,
    node: &str,
) -> Option<RangeMatch> {
    let found: bool = db
        .query_row(
            "SELECT 1 FROM events WHERE event_type = 'area_enter' AND frame = ?1 AND node_path = ?2 LIMIT 1",
            rusqlite::params![frame as i64, node],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if found {
        Some(RangeMatch {
            frame,
            time_ms,
            distance: None,
            node_pos: None,
            node_velocity: None,
            note: Some("area_enter".into()),
        })
    } else {
        None
    }
}

fn evaluate_collision(db: &Connection, frame: u64, time_ms: u64, node: &str) -> Option<RangeMatch> {
    let found: bool = db
        .query_row(
            "SELECT 1 FROM events WHERE event_type = 'collision' AND frame = ?1 AND node_path = ?2 LIMIT 1",
            rusqlite::params![frame as i64, node],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if found {
        Some(RangeMatch {
            frame,
            time_ms,
            distance: None,
            node_pos: None,
            node_velocity: None,
            note: Some("collision".into()),
        })
    } else {
        None
    }
}

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
    let prev_speed: f64 = prev_entity
        .velocity
        .iter()
        .map(|v| v * v)
        .sum::<f64>()
        .sqrt();
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

/// Simple glob matching for target patterns like "walls/*".
fn path_matches(path: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix("/*") {
        path.starts_with(prefix)
            && path.len() > prefix.len()
            && path.as_bytes()[prefix.len()] == b'/'
    } else if let Some(stripped) = pattern.strip_suffix('*') {
        path.starts_with(stripped)
    } else {
        path == pattern
    }
}

fn budget_truncate_count(matches: &[RangeMatch], budget_limit: u32) -> usize {
    let mut bytes: usize = 100; // overhead
    for (i, m) in matches.iter().enumerate() {
        let entry_bytes = serde_json::to_vec(m).unwrap_or_default().len();
        bytes += entry_bytes;
        if stage_core::budget::estimate_tokens(bytes) > budget_limit {
            return i.max(1);
        }
    }
    matches.len()
}

// ---------------------------------------------------------------------------
// System marker insertion
// ---------------------------------------------------------------------------

fn insert_system_marker(
    storage_path: &str,
    clip_id: &str,
    frame: u64,
    timestamp_ms: u64,
    label: &str,
) {
    let db_path = format!("{}/{}.sqlite", storage_path, clip_id);
    let db = match Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_WRITE) {
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

// ---------------------------------------------------------------------------
// trajectory
// ---------------------------------------------------------------------------

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
) -> Result<serde_json::Value, McpError> {
    let interval = sample_interval.max(1);

    let total_frames: u64 = db
        .query_row(
            "SELECT COUNT(*) FROM frames WHERE frame BETWEEN ?1 AND ?2",
            rusqlite::params![from_frame, to_frame],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let props: Vec<String> = if properties.is_empty() {
        vec!["position".to_string()]
    } else {
        properties.to_vec()
    };

    let mut stmt = db
        .prepare(
            "SELECT frame, timestamp_ms, data FROM frames \
             WHERE frame BETWEEN ?1 AND ?2 ORDER BY frame",
        )
        .map_err(sqlite_err)?;

    let rows = stmt
        .query_map(rusqlite::params![from_frame, to_frame], |row| {
            Ok((
                row.get::<_, u64>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        })
        .map_err(sqlite_err)?;

    let mut samples: Vec<serde_json::Value> = Vec::new();
    let mut budget_bytes: usize = 100; // overhead

    for row_result in rows {
        let (frame, time_ms, data) = row_result.map_err(sqlite_err)?;

        // Sample every Nth frame relative to from_frame
        if !(frame - from_frame).is_multiple_of(interval) {
            continue;
        }

        let entities: Vec<FrameEntityData> = rmp_serde::from_slice(&data).map_err(|e| {
            McpError::internal_error(
                format!("MessagePack decode error at frame {frame}: {e}"),
                None,
            )
        })?;

        let entity = match entities
            .iter()
            .find(|e| e.path == node || path_matches(&e.path, node))
        {
            Some(e) => e,
            None => continue,
        };

        let mut sample = json!({
            "frame": frame,
            "time_ms": time_ms,
        });

        for prop in &props {
            match prop.as_str() {
                "position" => {
                    sample["position"] = json!(entity.position);
                }
                "rotation_deg" => {
                    sample["rotation_deg"] = json!(entity.rotation_deg);
                }
                "velocity" => {
                    sample["velocity"] = json!(entity.velocity);
                }
                "speed" => {
                    let speed: f64 = entity.velocity.iter().map(|v| v * v).sum::<f64>().sqrt();
                    sample["speed"] = json!(speed);
                }
                other => {
                    if let Some(val) = entity.state.get(other) {
                        sample[other] = val.clone();
                    }
                }
            }
        }

        let sample_bytes = serde_json::to_vec(&sample).unwrap_or_default().len();
        budget_bytes += sample_bytes;
        if stage_core::budget::estimate_tokens(budget_bytes) > budget_limit {
            break;
        }
        samples.push(sample);
    }

    let samples_returned = samples.len();
    Ok(json!({
        "node": node,
        "from_frame": from_frame,
        "to_frame": to_frame,
        "sample_interval": interval,
        "samples": samples,
        "total_frames_in_range": total_frames,
        "samples_returned": samples_returned,
    }))
}

// ---------------------------------------------------------------------------
// diff_frames
// ---------------------------------------------------------------------------

/// Compare spatial state between two recorded frames.
pub fn diff_frames(
    db: &Connection,
    frame_a: u64,
    frame_b: u64,
    _budget_limit: u32,
) -> Result<serde_json::Value, McpError> {
    let entities_a = read_frame(db, frame_a)?;
    let entities_b = read_frame(db, frame_b)?;

    let ts_a: u64 = db
        .query_row(
            "SELECT timestamp_ms FROM frames WHERE frame = ?1",
            rusqlite::params![frame_a],
            |r| r.get::<_, i64>(0),
        )
        .map(|v| v as u64)
        .unwrap_or(0);
    let ts_b: u64 = db
        .query_row(
            "SELECT timestamp_ms FROM frames WHERE frame = ?1",
            rusqlite::params![frame_b],
            |r| r.get::<_, i64>(0),
        )
        .map(|v| v as u64)
        .unwrap_or(0);

    let map_a: HashMap<&str, &FrameEntityData> =
        entities_a.iter().map(|e| (e.path.as_str(), e)).collect();
    let map_b: HashMap<&str, &FrameEntityData> =
        entities_b.iter().map(|e| (e.path.as_str(), e)).collect();

    let mut changes: Vec<serde_json::Value> = Vec::new();
    let mut unchanged_count: usize = 0;

    for (path, b_entity) in &map_b {
        if let Some(a_entity) = map_a.get(path) {
            let mut entry = json!({ "path": path });
            let mut has_change = false;

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
    }

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

fn query_markers_between(
    db: &Connection,
    frame_a: u64,
    frame_b: u64,
) -> Result<Vec<serde_json::Value>, McpError> {
    let mut stmt = db
        .prepare(
            "SELECT frame, timestamp_ms, source, label FROM markers \
             WHERE frame BETWEEN ?1 AND ?2 ORDER BY frame",
        )
        .map_err(sqlite_err)?;

    let rows = stmt
        .query_map(rusqlite::params![frame_a, frame_b], |row| {
            Ok(json!({
                "frame": row.get::<_, i64>(0)?,
                "source": row.get::<_, String>(2)?,
                "label": row.get::<_, String>(3)?,
            }))
        })
        .map_err(sqlite_err)?;

    let markers: Vec<serde_json::Value> = rows.flatten().collect();
    Ok(markers)
}

// ---------------------------------------------------------------------------
// find_event
// ---------------------------------------------------------------------------

/// Search the recording timeline for specific event types.
#[allow(clippy::too_many_arguments)]
pub fn find_event(
    db: &Connection,
    clip_id: &str,
    event_type: &str,
    event_filter: Option<&str>,
    node: Option<&str>,
    from_frame: Option<u64>,
    to_frame: Option<u64>,
    budget_limit: u32,
) -> Result<serde_json::Value, McpError> {
    if event_type == "marker" {
        return find_markers(db, clip_id, event_filter, from_frame, to_frame);
    }

    let mut sql =
        String::from("SELECT frame, event_type, node_path, data FROM events WHERE event_type = ?1");
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(event_type.to_string())];
    let mut param_idx = 2usize;

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
    }

    sql.push_str(" ORDER BY frame");

    let mut stmt = db.prepare(&sql).map_err(sqlite_err)?;

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
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
        })
        .map_err(sqlite_err)?;

    let mut events: Vec<serde_json::Value> = Vec::new();
    for row in rows.flatten() {
        if let Some(filter) = event_filter {
            let row_str = row.to_string();
            if !row_str.contains(filter) {
                continue;
            }
        }
        events.push(row);
    }

    let total = events.len();
    let showing = budget_truncate_count_json(&events, budget_limit);
    events.truncate(showing);

    Ok(json!({
        "clip_id": clip_id,
        "event_type": event_type,
        "filter": event_filter,
        "events": events,
        "total_events": total,
        "showing": showing,
    }))
}

fn find_markers(
    db: &Connection,
    clip_id: &str,
    label_filter: Option<&str>,
    from_frame: Option<u64>,
    to_frame: Option<u64>,
) -> Result<serde_json::Value, McpError> {
    let mut sql = String::from("SELECT frame, timestamp_ms, source, label FROM markers WHERE 1=1");
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 1usize;

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

    let mut stmt = db.prepare(&sql).map_err(sqlite_err)?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok(json!({
                "frame": row.get::<_, i64>(0)?,
                "time_ms": row.get::<_, i64>(1)?,
                "source": row.get::<_, String>(2)?,
                "label": row.get::<_, String>(3)?,
            }))
        })
        .map_err(sqlite_err)?;

    let events: Vec<serde_json::Value> = rows.flatten().collect();

    Ok(json!({
        "clip_id": clip_id,
        "event_type": "marker",
        "events": events,
    }))
}

fn budget_truncate_count_json(items: &[serde_json::Value], budget_limit: u32) -> usize {
    let mut bytes: usize = 100;
    for (i, item) in items.iter().enumerate() {
        bytes += serde_json::to_vec(item).unwrap_or_default().len();
        if stage_core::budget::estimate_tokens(bytes) > budget_limit {
            return i.max(1);
        }
    }
    items.len()
}

// ---------------------------------------------------------------------------
// Screenshot reading
// ---------------------------------------------------------------------------

/// A screenshot read from a clip's SQLite database.
pub struct ClipScreenshot {
    pub frame: u64,
    pub timestamp_ms: u64,
    pub jpeg_data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Screenshot metadata (no image data) for listing.
pub struct ScreenshotMeta {
    pub frame: u64,
    pub timestamp_ms: u64,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
}

/// Returns true if the screenshots table exists in this clip's DB.
fn screenshots_table_exists(db: &Connection) -> bool {
    db.query_row(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='screenshots'",
        [],
        |_| Ok(()),
    )
    .is_ok()
}

/// Read the screenshot nearest to the given frame number.
pub fn read_screenshot_near_frame(
    db: &Connection,
    frame: u64,
) -> Result<Option<ClipScreenshot>, McpError> {
    if !screenshots_table_exists(db) {
        return Ok(None);
    }
    let result = db.query_row(
        "SELECT frame, timestamp_ms, image_data, width, height \
         FROM screenshots \
         ORDER BY ABS(CAST(frame AS INTEGER) - ?1) LIMIT 1",
        rusqlite::params![frame as i64],
        |row| {
            Ok(ClipScreenshot {
                frame: row.get::<_, i64>(0)? as u64,
                timestamp_ms: row.get::<_, i64>(1)? as u64,
                jpeg_data: row.get(2)?,
                width: row.get::<_, i64>(3)? as u32,
                height: row.get::<_, i64>(4)? as u32,
            })
        },
    );
    match result {
        Ok(s) => Ok(Some(s)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(sqlite_err(e)),
    }
}

/// Read the screenshot nearest to the given timestamp in milliseconds.
pub fn read_screenshot_near_time(
    db: &Connection,
    time_ms: u64,
) -> Result<Option<ClipScreenshot>, McpError> {
    if !screenshots_table_exists(db) {
        return Ok(None);
    }
    let result = db.query_row(
        "SELECT frame, timestamp_ms, image_data, width, height \
         FROM screenshots \
         ORDER BY ABS(CAST(timestamp_ms AS INTEGER) - ?1) LIMIT 1",
        rusqlite::params![time_ms as i64],
        |row| {
            Ok(ClipScreenshot {
                frame: row.get::<_, i64>(0)? as u64,
                timestamp_ms: row.get::<_, i64>(1)? as u64,
                jpeg_data: row.get(2)?,
                width: row.get::<_, i64>(3)? as u32,
                height: row.get::<_, i64>(4)? as u32,
            })
        },
    );
    match result {
        Ok(s) => Ok(Some(s)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(sqlite_err(e)),
    }
}

/// List all screenshot metadata in a clip (no image data).
pub fn list_screenshots(db: &Connection) -> Result<Vec<ScreenshotMeta>, McpError> {
    if !screenshots_table_exists(db) {
        return Ok(vec![]);
    }
    let mut stmt = db
        .prepare(
            "SELECT frame, timestamp_ms, width, height, LENGTH(image_data) \
             FROM screenshots ORDER BY frame",
        )
        .map_err(sqlite_err)?;

    let rows = stmt
        .query_map([], |row| {
            Ok(ScreenshotMeta {
                frame: row.get::<_, i64>(0)? as u64,
                timestamp_ms: row.get::<_, i64>(1)? as u64,
                width: row.get::<_, i64>(2)? as u32,
                height: row.get::<_, i64>(3)? as u32,
                size_bytes: row.get::<_, i64>(4)? as u64,
            })
        })
        .map_err(sqlite_err)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(sqlite_err)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS recording (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    started_at_frame INTEGER NOT NULL,
    ended_at_frame INTEGER,
    started_at_ms INTEGER NOT NULL,
    ended_at_ms INTEGER,
    scene_dimensions INTEGER,
    physics_ticks_per_sec INTEGER,
    capture_config TEXT
);
CREATE TABLE IF NOT EXISTS frames (
    frame INTEGER PRIMARY KEY,
    timestamp_ms INTEGER NOT NULL,
    data BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    frame INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    node_path TEXT NOT NULL,
    data TEXT NOT NULL,
    FOREIGN KEY (frame) REFERENCES frames(frame)
);
CREATE TABLE IF NOT EXISTS markers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    frame INTEGER NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    source TEXT NOT NULL,
    label TEXT NOT NULL,
    FOREIGN KEY (frame) REFERENCES frames(frame)
);
CREATE INDEX IF NOT EXISTS idx_frames_ts ON frames(timestamp_ms);
CREATE INDEX IF NOT EXISTS idx_events_frame ON events(frame);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_node ON events(node_path);
CREATE INDEX IF NOT EXISTS idx_markers_frame ON markers(frame);
";

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

    fn test_entity_with_state(
        path: &str,
        pos: [f64; 3],
        state: &[(&str, serde_json::Value)],
    ) -> FrameEntityData {
        let mut e = test_entity(path, pos);
        e.state = state
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect();
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
        )
        .unwrap();
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
        insert_frame(
            &db,
            100,
            1000,
            &[
                test_entity("far", [20.0, 0.0, 0.0]),
                test_entity("near", [1.0, 0.0, 0.0]),
            ],
        );
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
        insert_frame(
            &db,
            100,
            1000,
            &[test_entity_with_state(
                "enemy",
                [0.0, 0.0, 0.0],
                &[("health", serde_json::json!(100))],
            )],
        );
        insert_frame(
            &db,
            110,
            1167,
            &[test_entity_with_state(
                "enemy",
                [0.0, 0.0, 0.0],
                &[("health", serde_json::json!(50))],
            )],
        );

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
        // Insert marker at existing frame 100 (FK requires frame to exist)
        db.execute(
            "INSERT INTO markers (frame, timestamp_ms, source, label) VALUES (100, 1000, 'human', 'bug')",
            [],
        )
        .unwrap();

        let response = diff_frames(&db, 100, 110, 5000).unwrap();
        let markers = response["markers_between"].as_array().unwrap();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0]["label"], "bug");
    }

    // --- query_range tests ---

    #[test]
    fn query_range_proximity_finds_breach() {
        let db = test_db();
        insert_frame(
            &db,
            100,
            1000,
            &[
                test_entity("enemy", [10.0, 0.0, 0.0]),
                test_entity("wall", [0.0, 0.0, 0.0]),
            ],
        );
        insert_frame(
            &db,
            101,
            1017,
            &[
                test_entity("enemy", [0.3, 0.0, 0.0]),
                test_entity("wall", [0.0, 0.0, 0.0]),
            ],
        );

        let condition = QueryCondition {
            condition_type: "proximity".into(),
            target: Some("wall".into()),
            threshold: Some(0.5),
            property: None,
            signal: None,
        };

        let response =
            query_range(&db, "/tmp", "rec_1", "enemy", 100, 101, &condition, 5000).unwrap();
        let results = response["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["frame"], 101);
        assert_eq!(results[0]["note"], "first_breach");
    }

    #[test]
    fn query_range_velocity_spike() {
        let db = test_db();
        insert_frame(
            &db,
            100,
            1000,
            &[test_entity_with_velocity(
                "enemy",
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
            )],
        );
        insert_frame(
            &db,
            101,
            1017,
            &[test_entity_with_velocity(
                "enemy",
                [1.0, 0.0, 0.0],
                [10.0, 0.0, 0.0],
            )],
        );

        let condition = QueryCondition {
            condition_type: "velocity_spike".into(),
            target: None,
            threshold: Some(5.0),
            property: None,
            signal: None,
        };

        let response =
            query_range(&db, "/tmp", "rec_1", "enemy", 100, 101, &condition, 5000).unwrap();
        let results = response["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
    }

    // --- find_event tests ---

    #[test]
    fn find_event_searches_events_table() {
        let db = test_db();
        insert_frame(&db, 100, 1000, &[]);
        db.execute(
            "INSERT INTO events (frame, event_type, node_path, data) VALUES (100, 'signal', 'enemy', '{\"signal\":\"hit\"}')",
            [],
        )
        .unwrap();

        let response =
            find_event(&db, "rec_1", "signal", Some("hit"), None, None, None, 5000).unwrap();
        let events = response["events"].as_array().unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn find_event_marker_type_searches_markers_table() {
        let db = test_db();
        insert_frame(&db, 100, 1000, &[]);
        db.execute(
            "INSERT INTO markers (frame, timestamp_ms, source, label) VALUES (100, 1000, 'human', 'bug here')",
            [],
        )
        .unwrap();

        let response =
            find_event(&db, "rec_1", "marker", Some("bug"), None, None, None, 5000).unwrap();
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

    // --- query_range: signal_emitted condition ---

    #[test]
    fn query_range_signal_emitted_finds_matching_events() {
        let db = test_db();
        insert_frame(&db, 100, 1000, &[test_entity("enemy", [0.0, 0.0, 0.0])]);
        insert_frame(&db, 101, 1017, &[test_entity("enemy", [0.0, 0.0, 0.0])]);
        db.execute(
            "INSERT INTO events (frame, event_type, node_path, data) VALUES (101, 'signal', 'enemy', '{\"signal\":\"hit\"}')",
            [],
        )
        .unwrap();

        let condition = QueryCondition {
            condition_type: "signal_emitted".into(),
            target: None,
            threshold: None,
            property: None,
            signal: Some("hit".into()),
        };

        let response =
            query_range(&db, "/tmp", "rec_1", "enemy", 100, 101, &condition, 5000).unwrap();
        let results = response["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["frame"], 101);
    }

    #[test]
    fn query_range_entered_area_finds_matching_events() {
        let db = test_db();
        insert_frame(&db, 100, 1000, &[test_entity("enemy", [0.0, 0.0, 0.0])]);
        insert_frame(&db, 101, 1017, &[test_entity("enemy", [0.0, 0.0, 0.0])]);
        db.execute(
            "INSERT INTO events (frame, event_type, node_path, data) VALUES (101, 'area_enter', 'enemy', '{\"area\":\"danger_zone\"}')",
            [],
        )
        .unwrap();

        let condition = QueryCondition {
            condition_type: "entered_area".into(),
            target: None,
            threshold: None,
            property: None,
            signal: None,
        };

        let response =
            query_range(&db, "/tmp", "rec_1", "enemy", 100, 101, &condition, 5000).unwrap();
        let results = response["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["note"], "area_enter");
    }

    #[test]
    fn query_range_collision_finds_matching_events() {
        let db = test_db();
        insert_frame(&db, 100, 1000, &[test_entity("enemy", [0.0, 0.0, 0.0])]);
        db.execute(
            "INSERT INTO events (frame, event_type, node_path, data) VALUES (100, 'collision', 'enemy', '{\"with\":\"wall\"}')",
            [],
        )
        .unwrap();

        let condition = QueryCondition {
            condition_type: "collision".into(),
            target: None,
            threshold: None,
            property: None,
            signal: None,
        };

        let response =
            query_range(&db, "/tmp", "rec_1", "enemy", 100, 100, &condition, 5000).unwrap();
        let results = response["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["note"], "collision");
    }

    // --- moved condition tests ---

    #[test]
    fn test_moved_condition_detects_displacement() {
        let db = test_db();
        // 3 frames: Camera3D moves [0,60,60] → [0,55,55] → [0,48,48]
        insert_frame(&db, 1, 1000, &[test_entity("Camera3D", [0.0, 60.0, 60.0])]);
        insert_frame(&db, 2, 1017, &[test_entity("Camera3D", [0.0, 55.0, 55.0])]);
        insert_frame(&db, 3, 1033, &[test_entity("Camera3D", [0.0, 48.0, 48.0])]);

        let condition = QueryCondition {
            condition_type: "moved".into(),
            target: None,
            threshold: Some(1.0),
            property: None,
            signal: None,
        };

        let response =
            query_range(&db, "/tmp", "rec_1", "Camera3D", 1, 3, &condition, 5000).unwrap();
        let results = response["results"].as_array().unwrap();
        // Frame 2: moved ~7.07 units; Frame 3: moved ~9.9 units — both > 1.0
        assert_eq!(results.len(), 2);
        assert!(results[0]["distance"].as_f64().unwrap() > 1.0);
    }

    #[test]
    fn test_moved_condition_stationary_node() {
        let db = test_db();
        insert_frame(&db, 1, 1000, &[test_entity("Player", [0.0, 0.0, 0.0])]);
        insert_frame(&db, 2, 1017, &[test_entity("Player", [0.0, 0.0, 0.0])]);
        insert_frame(&db, 3, 1033, &[test_entity("Player", [0.0, 0.0, 0.0])]);

        let condition = QueryCondition {
            condition_type: "moved".into(),
            target: None,
            threshold: None, // default 0.01
            property: None,
            signal: None,
        };

        let response = query_range(&db, "/tmp", "rec_1", "Player", 1, 3, &condition, 5000).unwrap();
        let results = response["results"].as_array().unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_moved_condition_respects_threshold() {
        let db = test_db();
        // Node moves 0.005 units per frame — below default threshold of 0.01
        insert_frame(&db, 1, 1000, &[test_entity("Player", [0.0, 0.0, 0.0])]);
        insert_frame(&db, 2, 1017, &[test_entity("Player", [0.005, 0.0, 0.0])]);
        insert_frame(&db, 3, 1033, &[test_entity("Player", [0.010, 0.0, 0.0])]);

        let condition = QueryCondition {
            condition_type: "moved".into(),
            target: None,
            threshold: None, // default 0.01
            property: None,
            signal: None,
        };

        let response = query_range(&db, "/tmp", "rec_1", "Player", 1, 3, &condition, 5000).unwrap();
        let results = response["results"].as_array().unwrap();
        // Frame 2 moved 0.005 (below 0.01), Frame 3 moved 0.005 (below 0.01)
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_moved_condition_default_threshold() {
        let db = test_db();
        // Node moves exactly 0.01 units — at threshold, should match
        insert_frame(&db, 1, 1000, &[test_entity("Player", [0.0, 0.0, 0.0])]);
        insert_frame(&db, 2, 1017, &[test_entity("Player", [0.01, 0.0, 0.0])]);

        let condition = QueryCondition {
            condition_type: "moved".into(),
            target: None,
            threshold: None, // default 0.01
            property: None,
            signal: None,
        };

        let response = query_range(&db, "/tmp", "rec_1", "Player", 1, 2, &condition, 5000).unwrap();
        let results = response["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
    }

    // --- trajectory tests ---

    #[test]
    fn test_trajectory_basic() {
        let db = test_db();
        for i in 1u64..=10 {
            insert_frame(
                &db,
                i,
                i * 17,
                &[test_entity("Camera3D", [i as f64, 0.0, 0.0])],
            );
        }

        let response =
            trajectory(&db, "Camera3D", 1, 10, &["position".to_string()], 1, 5000).unwrap();
        let samples = response["samples"].as_array().unwrap();
        assert_eq!(samples.len(), 10);
        assert_eq!(samples[0]["frame"], 1);
        assert_eq!(samples[9]["frame"], 10);
        assert!(samples[0]["position"].is_array());
    }

    #[test]
    fn test_trajectory_sample_interval() {
        let db = test_db();
        for i in 1u64..=100 {
            insert_frame(
                &db,
                i,
                i * 17,
                &[test_entity("Camera3D", [i as f64, 0.0, 0.0])],
            );
        }

        let response =
            trajectory(&db, "Camera3D", 1, 100, &["position".to_string()], 10, 5000).unwrap();
        let samples = response["samples"].as_array().unwrap();
        // sample_interval=10 from from_frame=1: frames 1, 11, 21, ..., 91
        assert_eq!(samples.len(), 10);
        assert_eq!(samples[0]["frame"], 1);
        assert_eq!(samples[1]["frame"], 11);
        assert_eq!(samples[9]["frame"], 91);
    }

    #[test]
    fn test_trajectory_multiple_properties() {
        let db = test_db();
        let mut e = test_entity_with_state(
            "Player",
            [1.0, 2.0, 3.0],
            &[("health", serde_json::json!(100))],
        );
        e.velocity = vec![1.0, 0.0, 0.0];
        let data = rmp_serde::to_vec(&vec![e]).unwrap();
        db.execute(
            "INSERT INTO frames (frame, timestamp_ms, data) VALUES (?1, ?2, ?3)",
            rusqlite::params![1u64, 17u64, &data],
        )
        .unwrap();

        let props = vec![
            "position".to_string(),
            "velocity".to_string(),
            "health".to_string(),
        ];
        let response = trajectory(&db, "Player", 1, 1, &props, 1, 5000).unwrap();
        let samples = response["samples"].as_array().unwrap();
        assert_eq!(samples.len(), 1);
        assert!(samples[0]["position"].is_array());
        assert!(samples[0]["velocity"].is_array());
        assert_eq!(samples[0]["health"], 100);
    }

    // --- invalid condition type test ---

    #[test]
    fn test_invalid_condition_type_returns_error() {
        let db = test_db();
        insert_frame(&db, 1, 1000, &[test_entity("Player", [0.0, 0.0, 0.0])]);

        let condition = QueryCondition {
            condition_type: "foo".into(),
            target: None,
            threshold: None,
            property: None,
            signal: None,
        };

        let result = query_range(&db, "/tmp", "rec_1", "Player", 1, 1, &condition, 5000);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("foo"));
        assert!(err.message.contains("moved"));
    }

    #[test]
    fn test_moved_is_valid_condition_type() {
        let db = test_db();
        insert_frame(&db, 1, 1000, &[test_entity("Player", [0.0, 0.0, 0.0])]);
        insert_frame(&db, 2, 1017, &[test_entity("Player", [5.0, 0.0, 0.0])]);

        let condition = QueryCondition {
            condition_type: "moved".into(),
            target: None,
            threshold: None,
            property: None,
            signal: None,
        };

        let result = query_range(&db, "/tmp", "rec_1", "Player", 1, 2, &condition, 5000);
        assert!(result.is_ok());
    }

    #[test]
    fn test_trajectory_budget_truncation() {
        let db = test_db();
        // Insert 20 frames — more than any tight budget can hold
        for i in 1..=20u64 {
            insert_frame(
                &db,
                i,
                i * 17,
                &[test_entity("Player", [i as f64, 0.0, 0.0])],
            );
        }

        // Each position sample is ~50 bytes → ~12 tokens.
        // Overhead is 100 bytes → 25 tokens.
        // budget_limit = 50 allows ~2 samples before exceeding.
        let response = trajectory(&db, "Player", 1, 20, &[], 1, 50).unwrap();
        let samples = response["samples"].as_array().unwrap();
        let total = response["total_frames_in_range"].as_u64().unwrap();
        let returned = response["samples_returned"].as_u64().unwrap();

        assert_eq!(total, 20);
        assert!(
            returned < total,
            "expected budget truncation: returned {returned} but total was {total}"
        );
        assert_eq!(returned, samples.len() as u64);
    }

    // --- RecordingParams deserialization tests are in recording.rs ---

    // ---------------------------------------------------------------------------
    // Screenshot reading tests
    // ---------------------------------------------------------------------------

    fn screenshot_db() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS screenshots (
                frame INTEGER PRIMARY KEY,
                timestamp_ms INTEGER,
                image_data BLOB,
                width INTEGER,
                height INTEGER
            );",
        )
        .unwrap();
        db
    }

    fn insert_screenshot(db: &Connection, frame: u64, timestamp_ms: u64, width: u32, height: u32) {
        let jpeg = vec![0xFFu8, 0xD8, 0xFF, 0xE0, 0x00, frame as u8];
        db.execute(
            "INSERT INTO screenshots (frame, timestamp_ms, image_data, width, height) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                frame as i64,
                timestamp_ms as i64,
                &jpeg,
                width as i64,
                height as i64
            ],
        )
        .unwrap();
    }

    #[test]
    fn test_read_screenshot_near_frame() {
        let db = screenshot_db();
        insert_screenshot(&db, 100, 2000, 960, 540);
        insert_screenshot(&db, 200, 4000, 960, 540);
        insert_screenshot(&db, 300, 6000, 960, 540);

        let s = read_screenshot_near_frame(&db, 210).unwrap().unwrap();
        assert_eq!(s.frame, 200); // nearest to 210

        let s = read_screenshot_near_frame(&db, 260).unwrap().unwrap();
        assert_eq!(s.frame, 300); // nearest to 260

        let s = read_screenshot_near_frame(&db, 100).unwrap().unwrap();
        assert_eq!(s.frame, 100); // exact match
    }

    #[test]
    fn test_read_screenshot_near_time() {
        let db = screenshot_db();
        insert_screenshot(&db, 100, 2000, 960, 540);
        insert_screenshot(&db, 200, 4000, 960, 540);
        insert_screenshot(&db, 300, 6000, 960, 540);

        let s = read_screenshot_near_time(&db, 3500).unwrap().unwrap();
        assert_eq!(s.frame, 200); // timestamp 4000 is nearest to 3500

        let s = read_screenshot_near_time(&db, 5500).unwrap().unwrap();
        assert_eq!(s.frame, 300); // timestamp 6000 is nearest to 5500
    }

    #[test]
    fn test_list_screenshots_returns_metadata() {
        let db = screenshot_db();
        insert_screenshot(&db, 100, 2000, 960, 540);
        insert_screenshot(&db, 200, 4000, 480, 270);

        let list = list_screenshots(&db).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].frame, 100);
        assert_eq!(list[0].width, 960);
        assert_eq!(list[0].height, 540);
        assert!(list[0].size_bytes > 0);
        assert_eq!(list[1].frame, 200);
        assert_eq!(list[1].width, 480);
    }

    #[test]
    fn test_screenshot_missing_table_returns_empty() {
        // DB without screenshots table (pre-feature clip)
        let db = Connection::open_in_memory().unwrap();

        let result = read_screenshot_near_frame(&db, 100).unwrap();
        assert!(result.is_none(), "Should return None when table is missing");

        let result = read_screenshot_near_time(&db, 2000).unwrap();
        assert!(result.is_none(), "Should return None when table is missing");

        let list = list_screenshots(&db).unwrap();
        assert!(
            list.is_empty(),
            "Should return empty list when table is missing"
        );
    }

    #[test]
    fn test_screenshot_empty_table_returns_none() {
        let db = screenshot_db(); // table exists but no rows

        let result = read_screenshot_near_frame(&db, 100).unwrap();
        assert!(result.is_none());

        let result = read_screenshot_near_time(&db, 2000).unwrap();
        assert!(result.is_none());

        let list = list_screenshots(&db).unwrap();
        assert!(list.is_empty());
    }

    // -----------------------------------------------------------------------
    // Offline persistence tests
    // -----------------------------------------------------------------------

    /// Extended schema matching production (includes created_at_unix_ms).
    const FULL_SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS recording (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    started_at_frame INTEGER NOT NULL,
    ended_at_frame INTEGER,
    started_at_ms INTEGER NOT NULL,
    ended_at_ms INTEGER,
    scene_dimensions INTEGER,
    physics_ticks_per_sec INTEGER,
    capture_config TEXT,
    created_at_unix_ms INTEGER
);
CREATE TABLE IF NOT EXISTS frames (
    frame INTEGER PRIMARY KEY,
    timestamp_ms INTEGER NOT NULL,
    data BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS markers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    frame INTEGER NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    source TEXT NOT NULL,
    label TEXT NOT NULL,
    FOREIGN KEY (frame) REFERENCES frames(frame)
);
CREATE INDEX IF NOT EXISTS idx_frames_ts ON frames(timestamp_ms);
CREATE INDEX IF NOT EXISTS idx_markers_frame ON markers(frame);
";

    /// Create a real SQLite clip file on disk for offline tests.
    fn create_clip_on_disk(dir: &std::path::Path, clip_id: &str, created_at_ms: i64) {
        let db_path = dir.join(format!("{clip_id}.sqlite"));
        let db = Connection::open(&db_path).unwrap();
        db.execute_batch(FULL_SCHEMA_SQL).unwrap();

        let capture_config = serde_json::json!({
            "dashcam": true,
            "tier": "deliberate",
        });
        db.execute(
            "INSERT INTO recording (id, name, started_at_frame, ended_at_frame, \
             started_at_ms, ended_at_ms, scene_dimensions, physics_ticks_per_sec, \
             capture_config, created_at_unix_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                clip_id,
                format!("dashcam_{clip_id}"),
                100,
                200,
                created_at_ms,
                created_at_ms + 5000,
                3,
                60,
                capture_config.to_string(),
                created_at_ms,
            ],
        )
        .unwrap();

        // Insert a few frames
        for f in [100u64, 150, 200] {
            let entities = vec![test_entity("player", [1.0, 0.0, 0.0])];
            let data = rmp_serde::to_vec(&entities).unwrap();
            db.execute(
                "INSERT INTO frames (frame, timestamp_ms, data) VALUES (?1, ?2, ?3)",
                rusqlite::params![f, created_at_ms + (f - 100) as i64 * 50, &data],
            )
            .unwrap();
        }
    }

    fn add_marker_on_disk(dir: &std::path::Path, clip_id: &str, frame: i64, label: &str) {
        let db_path = dir.join(format!("{clip_id}.sqlite"));
        let db = Connection::open(&db_path).unwrap();
        db.execute(
            "INSERT INTO markers (frame, timestamp_ms, source, label) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![frame, frame * 16, "agent", label],
        )
        .unwrap();
    }

    // --- list_clips_from_disk ---

    #[test]
    fn list_clips_from_disk_returns_clips_with_metadata() {
        let tmp = tempfile::TempDir::new().unwrap();
        create_clip_on_disk(tmp.path(), "clip_aaa", 1710000000000);
        add_marker_on_disk(tmp.path(), "clip_aaa", 150, "bug here");

        let result = list_clips_from_disk(tmp.path().to_str().unwrap()).unwrap();
        let clips = result["clips"].as_array().unwrap();
        assert_eq!(clips.len(), 1);

        let clip = &clips[0];
        assert_eq!(clip["clip_id"].as_str().unwrap(), "clip_aaa");
        assert_eq!(clip["frames_captured"].as_u64().unwrap(), 3);
        assert_eq!(clip["markers_count"].as_u64().unwrap(), 1);
        assert_eq!(clip["dashcam"].as_bool().unwrap(), true);
        assert_eq!(clip["tier"].as_str().unwrap(), "deliberate");
        assert!(clip["frame_range"].is_array());
        assert!(clip["duration_ms"].as_i64().unwrap() > 0);
        assert!(clip["created_at"].as_str().unwrap().contains("T"));
    }

    #[test]
    fn list_clips_from_disk_empty_directory() {
        let tmp = tempfile::TempDir::new().unwrap();
        let result = list_clips_from_disk(tmp.path().to_str().unwrap()).unwrap();
        let clips = result["clips"].as_array().unwrap();
        assert!(clips.is_empty());
    }

    #[test]
    fn list_clips_from_disk_skips_non_sqlite_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        create_clip_on_disk(tmp.path(), "clip_good", 1710000000000);
        // Create a non-sqlite file that should be ignored
        std::fs::write(tmp.path().join("notes.txt"), "not a clip").unwrap();
        // Create a corrupt sqlite file that should be skipped gracefully
        std::fs::write(tmp.path().join("clip_bad.sqlite"), "not valid sqlite").unwrap();

        let result = list_clips_from_disk(tmp.path().to_str().unwrap()).unwrap();
        let clips = result["clips"].as_array().unwrap();
        // Only the valid clip should appear
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0]["clip_id"].as_str().unwrap(), "clip_good");
    }

    #[test]
    fn list_clips_from_disk_sorted_newest_first() {
        let tmp = tempfile::TempDir::new().unwrap();
        create_clip_on_disk(tmp.path(), "clip_old", 1700000000000);
        create_clip_on_disk(tmp.path(), "clip_new", 1710000000000);

        let result = list_clips_from_disk(tmp.path().to_str().unwrap()).unwrap();
        let clips = result["clips"].as_array().unwrap();
        assert_eq!(clips.len(), 2);
        // Newest first
        assert_eq!(clips[0]["clip_id"].as_str().unwrap(), "clip_new");
        assert_eq!(clips[1]["clip_id"].as_str().unwrap(), "clip_old");
    }

    #[test]
    fn list_clips_from_disk_nonexistent_directory() {
        let result = list_clips_from_disk("/nonexistent/path/to/clips");
        assert!(result.is_err());
    }

    // --- list_markers_from_disk ---

    #[test]
    fn list_markers_from_disk_returns_markers_sorted_by_frame() {
        let tmp = tempfile::TempDir::new().unwrap();
        create_clip_on_disk(tmp.path(), "clip_001", 1710000000000);
        add_marker_on_disk(tmp.path(), "clip_001", 200, "end");
        add_marker_on_disk(tmp.path(), "clip_001", 100, "start");
        add_marker_on_disk(tmp.path(), "clip_001", 150, "middle");

        let result = list_markers_from_disk(tmp.path().to_str().unwrap(), "clip_001").unwrap();
        assert_eq!(result["clip_id"].as_str().unwrap(), "clip_001");

        let markers = result["markers"].as_array().unwrap();
        assert_eq!(markers.len(), 3);
        // Sorted by frame ascending
        assert_eq!(markers[0]["frame"].as_i64().unwrap(), 100);
        assert_eq!(markers[0]["label"].as_str().unwrap(), "start");
        assert_eq!(markers[1]["frame"].as_i64().unwrap(), 150);
        assert_eq!(markers[2]["frame"].as_i64().unwrap(), 200);
    }

    #[test]
    fn list_markers_from_disk_empty_markers() {
        let tmp = tempfile::TempDir::new().unwrap();
        create_clip_on_disk(tmp.path(), "clip_001", 1710000000000);

        let result = list_markers_from_disk(tmp.path().to_str().unwrap(), "clip_001").unwrap();
        let markers = result["markers"].as_array().unwrap();
        assert!(markers.is_empty());
    }

    #[test]
    fn list_markers_from_disk_missing_clip_returns_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let result = list_markers_from_disk(tmp.path().to_str().unwrap(), "clip_nope");
        assert!(result.is_err());
    }

    // --- delete_clip_from_disk ---

    #[test]
    fn delete_clip_from_disk_removes_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        create_clip_on_disk(tmp.path(), "clip_del", 1710000000000);

        let db_path = tmp.path().join("clip_del.sqlite");
        assert!(db_path.exists());

        let result = delete_clip_from_disk(tmp.path().to_str().unwrap(), "clip_del").unwrap();
        assert_eq!(result["result"].as_str().unwrap(), "ok");
        assert_eq!(result["clip_id"].as_str().unwrap(), "clip_del");
        assert!(!db_path.exists());
    }

    #[test]
    fn delete_clip_from_disk_missing_returns_error() {
        let tmp = tempfile::TempDir::new().unwrap();
        let result = delete_clip_from_disk(tmp.path().to_str().unwrap(), "clip_nope");
        assert!(result.is_err());
    }

    // --- most_recent_clip_id ---

    #[test]
    fn most_recent_clip_id_selects_newest_by_mtime() {
        let tmp = tempfile::TempDir::new().unwrap();
        create_clip_on_disk(tmp.path(), "clip_older", 1700000000000);
        // Brief sleep to ensure different mtime
        std::thread::sleep(std::time::Duration::from_millis(50));
        create_clip_on_disk(tmp.path(), "clip_newer", 1710000000000);

        let result = most_recent_clip_id(tmp.path().to_str().unwrap());
        assert_eq!(result.as_deref(), Some("clip_newer"));
    }

    #[test]
    fn most_recent_clip_id_empty_directory_returns_none() {
        let tmp = tempfile::TempDir::new().unwrap();
        let result = most_recent_clip_id(tmp.path().to_str().unwrap());
        assert!(result.is_none());
    }

    // --- resolve_clip_storage_path (offline fallback chain) ---

    #[tokio::test]
    async fn resolve_clip_storage_path_returns_memory_cache() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cached_path = tmp.path().to_str().unwrap().to_string();

        let state = Arc::new(Mutex::new(SessionState {
            clip_storage_path: Some(cached_path.clone()),
            ..Default::default()
        }));

        let result = resolve_clip_storage_path(&state).await.unwrap();
        assert_eq!(result, cached_path);
    }

    #[tokio::test]
    async fn resolve_clip_storage_path_reads_disk_cache_when_tcp_down() {
        let tmp = tempfile::TempDir::new().unwrap();
        let storage_dir = tmp.path().join("recordings");
        std::fs::create_dir_all(&storage_dir).unwrap();

        // Write the disk cache file
        let cache_dir = tmp.path().join("project").join(".stage");
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(
            cache_dir.join("clip_storage_path"),
            storage_dir.to_str().unwrap(),
        )
        .unwrap();

        let state = Arc::new(Mutex::new(SessionState {
            clip_storage_path: None, // memory cache empty
            project_dir: tmp.path().join("project"),
            ..Default::default()
        }));

        let result = resolve_clip_storage_path(&state).await.unwrap();
        assert_eq!(result, storage_dir.to_str().unwrap());

        // Should also cache in memory now
        let s = state.lock().await;
        assert_eq!(
            s.clip_storage_path.as_deref(),
            Some(storage_dir.to_str().unwrap())
        );
    }

    #[tokio::test]
    async fn resolve_clip_storage_path_ignores_invalid_disk_cache() {
        let tmp = tempfile::TempDir::new().unwrap();

        // Write a disk cache pointing to a nonexistent directory
        let cache_dir = tmp.path().join(".stage");
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(
            cache_dir.join("clip_storage_path"),
            "/nonexistent/invalid/path",
        )
        .unwrap();

        let state = Arc::new(Mutex::new(SessionState {
            clip_storage_path: None,
            project_dir: tmp.path().to_path_buf(),
            ..Default::default()
        }));

        // No TCP either → should fail
        let result = resolve_clip_storage_path(&state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn resolve_clip_storage_path_fails_when_all_sources_unavailable() {
        let tmp = tempfile::TempDir::new().unwrap();

        let state = Arc::new(Mutex::new(SessionState {
            clip_storage_path: None,
            project_dir: tmp.path().to_path_buf(),
            ..Default::default()
        }));

        // No memory cache, no disk cache, no TCP → error
        let result = resolve_clip_storage_path(&state).await;
        assert!(result.is_err());
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.contains("Cannot resolve clip storage path"),
            "error should explain the failure: {err_msg}"
        );
    }

    // --- unix_ms_to_iso8601 ---

    #[test]
    fn unix_ms_to_iso8601_converts_correctly() {
        // 2024-03-10T00:00:00Z = 1710028800000 ms
        let result = unix_ms_to_iso8601(1710028800000);
        assert_eq!(result, "2024-03-10T00:00:00Z");
    }

    #[test]
    fn unix_ms_to_iso8601_zero_returns_empty() {
        assert_eq!(unix_ms_to_iso8601(0), "");
        assert_eq!(unix_ms_to_iso8601(-1), "");
    }
}
