use base64::Engine as Base64Engine;
use rmcp::model::{CallToolResult, Content, ErrorData as McpError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use stage_core::budget::resolve_budget;

use crate::clip_analysis;
use crate::tcp::{SessionState, query_addon};

use super::{finalize_response, require_param};

// ---------------------------------------------------------------------------
// MCP parameter types
// ---------------------------------------------------------------------------

/// Clip management and analysis action.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClipAction {
    /// Mark the current moment, triggers clip capture.
    AddMarker,
    /// Force-save the dashcam buffer as a clip.
    Save,
    /// Dashcam buffer state and config.
    Status,
    /// List saved clips.
    List,
    /// Remove a clip by clip_id.
    Delete,
    /// List markers in a saved clip.
    Markers,
    /// Spatial state at a frame in a clip.
    SnapshotAt,
    /// Position/property timeseries across frame range.
    Trajectory,
    /// Search frames for spatial conditions.
    QueryRange,
    /// Compare two frames in a clip.
    DiffFrames,
    /// Search events in a clip.
    FindEvent,
    /// Get the viewport screenshot nearest to a frame or timestamp.
    ScreenshotAt,
    /// List screenshot metadata in a clip.
    Screenshots,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ClipsParams {
    /// Action to perform.
    pub action: ClipAction,

    /// Clip to operate on. Uses most recent if omitted.
    #[schemars(
        description = "Clip to operate on (from list response). Defaults to most recent clip if omitted."
    )]
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
    #[schemars(
        description = "Condition for query_range. Object with \"type\" key. Types: \"moved\" (threshold), \"proximity\" (target, threshold), \"velocity_spike\" (threshold), \"property_change\" (property), \"state_transition\" (property), \"signal_emitted\" (signal), \"entered_area\", \"collision\". Example: {\"type\": \"proximity\", \"target\": \"walls/*\", \"threshold\": 0.5}"
    )]
    pub condition: Option<serde_json::Value>,

    /// Properties to sample in trajectory. Default: ["position"].
    /// Options: "position", "rotation_deg", "velocity", "speed", or any state property name.
    #[schemars(
        description = "Properties to sample in trajectory. Default: [\"position\"]. Options: position, rotation_deg, velocity, speed, or any state property name."
    )]
    pub properties: Option<Vec<String>>,

    /// Sample every Nth frame in trajectory. Default: 1 (every frame).
    #[schemars(description = "Sample every Nth frame for trajectory. Default: 1.")]
    pub sample_interval: Option<u64>,

    /// Event type for find_event.
    pub event_type: Option<String>,

    /// Event filter for find_event (substring match).
    pub event_filter: Option<String>,

    /// Frame A for diff_frames.
    pub frame_a: Option<u64>,

    /// Frame B for diff_frames.
    pub frame_b: Option<u64>,
}

// ---------------------------------------------------------------------------
// Top-level handler
// ---------------------------------------------------------------------------

pub async fn handle_clips(
    params: ClipsParams,
    state: &Arc<Mutex<SessionState>>,
) -> Result<CallToolResult, McpError> {
    let config = crate::tcp::get_config(state).await;
    let hard_cap = config.token_hard_cap;
    let budget_limit = resolve_budget(params.token_budget, 1500, hard_cap);

    match params.action {
        ClipAction::AddMarker => {
            let s = handle_add_marker(&params, state, budget_limit, hard_cap).await?;
            Ok(text_result(s))
        }
        ClipAction::Save => {
            let s = handle_save(&params, state, budget_limit, hard_cap).await?;
            Ok(text_result(s))
        }
        ClipAction::Status => {
            let s = query_and_finalize(state, "dashcam_status", json!({}), budget_limit, hard_cap)
                .await?;
            Ok(text_result(s))
        }
        ClipAction::List => {
            let s = handle_list(state, budget_limit, hard_cap).await?;
            Ok(text_result(s))
        }
        ClipAction::Delete => {
            let s = handle_delete(&params, state, budget_limit, hard_cap).await?;
            Ok(text_result(s))
        }
        ClipAction::Markers => {
            let s = handle_markers(&params, state, budget_limit, hard_cap).await?;
            Ok(text_result(s))
        }
        ClipAction::SnapshotAt => {
            let s = handle_snapshot_at(&params, state, budget_limit, hard_cap).await?;
            Ok(text_result(s))
        }
        ClipAction::Trajectory => {
            let s = handle_trajectory(&params, state, budget_limit, hard_cap).await?;
            Ok(text_result(s))
        }
        ClipAction::QueryRange => {
            let s = handle_query_range(&params, state, budget_limit, hard_cap).await?;
            Ok(text_result(s))
        }
        ClipAction::DiffFrames => {
            let s = handle_diff_frames(&params, state, budget_limit, hard_cap).await?;
            Ok(text_result(s))
        }
        ClipAction::FindEvent => {
            let s = handle_find_event(&params, state, budget_limit, hard_cap).await?;
            Ok(text_result(s))
        }
        ClipAction::ScreenshotAt => handle_screenshot_at(&params, state).await,
        ClipAction::Screenshots => {
            let s = handle_screenshots(&params, state).await?;
            Ok(text_result(s))
        }
    }
}

/// Wrap a string result in a single-text-block CallToolResult.
fn text_result(s: String) -> CallToolResult {
    CallToolResult::success(vec![Content::text(s)])
}

// ---------------------------------------------------------------------------
// Action handlers
// ---------------------------------------------------------------------------

async fn query_and_finalize(
    state: &Arc<Mutex<SessionState>>,
    method: &str,
    params: serde_json::Value,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let mut data = query_addon(state, method, params).await?;
    finalize_response(&mut data, budget_limit, hard_cap)
}

async fn handle_list(
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    // Try live addon first (includes currently-recording info), fall back to disk
    match query_and_finalize(state, "recording_list", json!({}), budget_limit, hard_cap).await {
        Ok(s) => Ok(s),
        Err(_) => {
            let storage_path = clip_analysis::resolve_clip_storage_path(state).await?;
            let mut data = clip_analysis::list_clips_from_disk(&storage_path)?;
            finalize_response(&mut data, budget_limit, hard_cap)
        }
    }
}

async fn handle_add_marker(
    params: &ClipsParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let mut query = json!({
        "source": "agent",
        "label": params.marker_label.as_deref().unwrap_or(""),
    });
    if let Some(frame) = params.marker_frame {
        query["frame"] = json!(frame);
    }
    query_and_finalize(state, "recording_marker", query, budget_limit, hard_cap).await
}

async fn handle_save(
    params: &ClipsParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let query = json!({
        "marker_label": params.marker_label.as_deref().unwrap_or("agent save"),
    });
    query_and_finalize(state, "dashcam_flush", query, budget_limit, hard_cap).await
}

async fn handle_delete(
    params: &ClipsParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let id = require_param!(params.clip_id.as_deref(), "clip_id is required for delete");
    let storage_path = clip_analysis::resolve_clip_storage_path(state).await?;
    let mut data = clip_analysis::delete_clip_from_disk(&storage_path, id)?;
    finalize_response(&mut data, budget_limit, hard_cap)
}

async fn handle_markers(
    params: &ClipsParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let storage_path = clip_analysis::resolve_clip_storage_path(state).await?;
    let clip_id = match params.clip_id.as_deref() {
        Some(id) => id.to_string(),
        None => clip_analysis::most_recent_clip_id(&storage_path)
            .ok_or_else(|| McpError::invalid_params("clip_id is required for markers", None))?,
    };
    let mut data = clip_analysis::list_markers_from_disk(&storage_path, &clip_id)?;
    finalize_response(&mut data, budget_limit, hard_cap)
}

// ---------------------------------------------------------------------------
// Frame resolution helpers
// ---------------------------------------------------------------------------

fn resolve_frame(
    session: &clip_analysis::ClipSession,
    at_frame: Option<u64>,
    at_time_ms: Option<u64>,
    action: &str,
) -> Result<u64, McpError> {
    if let Some(f) = at_frame {
        session.meta.validate_frame(f)?;
        Ok(f)
    } else if let Some(t) = at_time_ms {
        let (frame, _) = clip_analysis::read_frame_at_time(&session.db, t)?;
        Ok(frame)
    } else {
        Err(McpError::invalid_params(
            format!("{action} requires at_frame or at_time_ms"),
            None,
        ))
    }
}

fn resolve_frame_range(
    session: &clip_analysis::ClipSession,
    from: Option<u64>,
    to: Option<u64>,
    action: &str,
) -> Result<(u64, u64), McpError> {
    let from = from
        .ok_or_else(|| McpError::invalid_params(format!("{action} requires 'from_frame'"), None))?;
    let to =
        to.ok_or_else(|| McpError::invalid_params(format!("{action} requires 'to_frame'"), None))?;
    session.meta.validate_frame(from)?;
    session.meta.validate_frame(to)?;
    Ok((from, to))
}

// ---------------------------------------------------------------------------
// Analysis handlers
// ---------------------------------------------------------------------------

async fn handle_snapshot_at(
    params: &ClipsParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let session = clip_analysis::ClipSession::open(state, params.clip_id.as_deref()).await?;

    let frame = resolve_frame(&session, params.at_frame, params.at_time_ms, "snapshot_at")?;

    let detail = params.detail.as_deref().unwrap_or("standard");
    let mut response =
        clip_analysis::snapshot_at(&session.db, frame, detail, budget_limit, hard_cap)?;
    session.finalize(&mut response, budget_limit, hard_cap)
}

async fn handle_trajectory(
    params: &ClipsParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let session = clip_analysis::ClipSession::open(state, params.clip_id.as_deref()).await?;

    let node = require_param!(
        params.node.as_deref(),
        "trajectory requires 'node' parameter"
    );
    let (from, to) =
        resolve_frame_range(&session, params.from_frame, params.to_frame, "trajectory")?;

    let properties = params.properties.as_deref().unwrap_or(&[]);
    let sample_interval = params.sample_interval.unwrap_or(1);

    let mut response = clip_analysis::trajectory(
        &session.db,
        node,
        from,
        to,
        properties,
        sample_interval,
        budget_limit,
    )?;
    session.finalize(&mut response, budget_limit, hard_cap)
}

async fn handle_query_range(
    params: &ClipsParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let session = clip_analysis::ClipSession::open(state, params.clip_id.as_deref()).await?;

    let node = require_param!(
        params.node.as_deref(),
        "query_range requires 'node' parameter"
    );
    let (from, to) =
        resolve_frame_range(&session, params.from_frame, params.to_frame, "query_range")?;
    let condition: clip_analysis::QueryCondition = params
        .condition
        .as_ref()
        .ok_or_else(|| {
            McpError::invalid_params("query_range requires 'condition'".to_string(), None)
        })
        .and_then(|v| {
            serde_json::from_value(v.clone())
                .map_err(|e| McpError::invalid_params(format!("Invalid condition: {e}"), None))
        })?;

    let mut response = clip_analysis::query_range(
        &session.db,
        &session.storage_path,
        &session.clip_id,
        node,
        from,
        to,
        &condition,
        budget_limit,
    )?;
    session.finalize(&mut response, budget_limit, hard_cap)
}

async fn handle_diff_frames(
    params: &ClipsParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let session = clip_analysis::ClipSession::open(state, params.clip_id.as_deref()).await?;

    let frame_a = require_param!(params.frame_a, "diff_frames requires 'frame_a'");
    let frame_b = require_param!(params.frame_b, "diff_frames requires 'frame_b'");
    session.meta.validate_frame(frame_a)?;
    session.meta.validate_frame(frame_b)?;

    let mut response = clip_analysis::diff_frames(&session.db, frame_a, frame_b, budget_limit)?;
    session.finalize(&mut response, budget_limit, hard_cap)
}

async fn handle_find_event(
    params: &ClipsParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let session = clip_analysis::ClipSession::open(state, params.clip_id.as_deref()).await?;

    if let Some(from) = params.from_frame {
        session.meta.validate_frame(from)?;
    }
    if let Some(to) = params.to_frame {
        session.meta.validate_frame(to)?;
    }

    let event_type = require_param!(
        params.event_type.as_deref(),
        "find_event requires 'event_type'"
    );

    let mut response = clip_analysis::find_event(
        &session.db,
        &session.clip_id,
        event_type,
        params.event_filter.as_deref(),
        params.node.as_deref(),
        params.from_frame,
        params.to_frame,
        budget_limit,
    )?;
    session.finalize(&mut response, budget_limit, hard_cap)
}

async fn handle_screenshot_at(
    params: &ClipsParams,
    state: &Arc<Mutex<SessionState>>,
) -> Result<CallToolResult, McpError> {
    let session = clip_analysis::ClipSession::open(state, params.clip_id.as_deref()).await?;

    let screenshot = if let Some(frame) = params.at_frame {
        clip_analysis::read_screenshot_near_frame(&session.db, frame)?
    } else if let Some(time_ms) = params.at_time_ms {
        clip_analysis::read_screenshot_near_time(&session.db, time_ms)?
    } else {
        return Err(McpError::invalid_params(
            "screenshot_at requires at_frame or at_time_ms".to_string(),
            None,
        ));
    };

    let Some(screenshot) = screenshot else {
        return Ok(text_result(
            json!({
                "error": "no_screenshots",
                "clip_id": session.clip_id,
                "message": "This clip contains no screenshots",
            })
            .to_string(),
        ));
    };

    let metadata = json!({
        "clip_id": session.clip_id,
        "frame": screenshot.frame,
        "timestamp_ms": screenshot.timestamp_ms,
        "width": screenshot.width,
        "height": screenshot.height,
        "size_bytes": screenshot.jpeg_data.len(),
    });

    let b64 = base64::engine::general_purpose::STANDARD.encode(&screenshot.jpeg_data);

    Ok(CallToolResult::success(vec![
        Content::text(metadata.to_string()),
        Content::image(b64, "image/jpeg"),
    ]))
}

async fn handle_screenshots(
    params: &ClipsParams,
    state: &Arc<Mutex<SessionState>>,
) -> Result<String, McpError> {
    let session = clip_analysis::ClipSession::open(state, params.clip_id.as_deref()).await?;
    let list = clip_analysis::list_screenshots(&session.db)?;

    let result = json!({
        "clip_id": session.clip_id,
        "screenshots": list.iter().map(|s| json!({
            "frame": s.frame,
            "timestamp_ms": s.timestamp_ms,
            "width": s.width,
            "height": s.height,
            "size_bytes": s.size_bytes,
        })).collect::<Vec<_>>(),
        "total": list.len(),
    });

    Ok(result.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_params_deserializes_add_marker() {
        let json = serde_json::json!({
            "action": "add_marker",
            "marker_label": "bug here",
        });
        let params: ClipsParams = serde_json::from_value(json).unwrap();
        assert!(matches!(params.action, ClipAction::AddMarker));
        assert_eq!(params.marker_label.as_deref(), Some("bug here"));
    }

    #[test]
    fn clips_params_save() {
        let json = serde_json::json!({
            "action": "save",
            "marker_label": "suspected physics glitch",
        });
        let params: ClipsParams = serde_json::from_value(json).unwrap();
        assert!(matches!(params.action, ClipAction::Save));
        assert_eq!(
            params.marker_label.as_deref(),
            Some("suspected physics glitch")
        );
    }

    #[test]
    fn clips_params_snapshot_at() {
        let json = serde_json::json!({
            "action": "snapshot_at",
            "at_frame": 4575,
            "detail": "standard",
            "clip_id": "clip_001",
        });
        let params: ClipsParams = serde_json::from_value(json).unwrap();
        assert!(matches!(params.action, ClipAction::SnapshotAt));
        assert_eq!(params.at_frame, Some(4575));
        assert_eq!(params.detail.as_deref(), Some("standard"));
        assert_eq!(params.clip_id.as_deref(), Some("clip_001"));
    }

    #[test]
    fn clips_params_query_range() {
        let json = serde_json::json!({
            "action": "query_range",
            "from_frame": 4570u64,
            "to_frame": 4590u64,
            "node": "enemies/guard_01",
            "condition": {
                "type": "proximity",
                "target": "walls/*",
                "threshold": 0.5,
            },
        });
        let params: ClipsParams = serde_json::from_value(json).unwrap();
        assert!(matches!(params.action, ClipAction::QueryRange));
        assert!(params.condition.is_some());
        assert_eq!(params.from_frame, Some(4570));
        assert_eq!(params.to_frame, Some(4590));
    }

    #[test]
    fn clips_params_diff_frames() {
        let json = serde_json::json!({
            "action": "diff_frames",
            "frame_a": 3010u64,
            "frame_b": 3020u64,
        });
        let params: ClipsParams = serde_json::from_value(json).unwrap();
        assert!(matches!(params.action, ClipAction::DiffFrames));
        assert_eq!(params.frame_a, Some(3010));
        assert_eq!(params.frame_b, Some(3020));
    }

    #[test]
    fn clips_params_find_event() {
        let json = serde_json::json!({
            "action": "find_event",
            "event_type": "signal",
            "event_filter": "health_changed",
            "clip_id": "clip_001",
        });
        let params: ClipsParams = serde_json::from_value(json).unwrap();
        assert!(matches!(params.action, ClipAction::FindEvent));
        assert_eq!(params.event_type.as_deref(), Some("signal"));
        assert_eq!(params.event_filter.as_deref(), Some("health_changed"));
    }

    #[test]
    fn clips_params_status() {
        let json = serde_json::json!({ "action": "status" });
        let params: ClipsParams = serde_json::from_value(json).unwrap();
        assert!(matches!(params.action, ClipAction::Status));
    }
}
