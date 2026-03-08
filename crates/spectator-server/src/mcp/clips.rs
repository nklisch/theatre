use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use spectator_core::budget::resolve_budget;

use crate::clip_analysis;
use crate::tcp::{SessionState, query_addon};

use super::{finalize_response, require_param};

// ---------------------------------------------------------------------------
// MCP parameter types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Top-level handler
// ---------------------------------------------------------------------------

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
            format!(
                "Unknown clips action: '{other}'. Valid: add_marker, save, status, list, delete, \
                 markers, snapshot_at, query_range, diff_frames, find_event"
            ),
            None,
        )),
    }
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
    let data = query_addon(state, "recording_marker", query).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
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
    let data = query_addon(state, "dashcam_flush", query).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_delete(
    params: &ClipsParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let id = require_param!(params.clip_id.as_deref(), "clip_id is required for delete");
    let data = query_addon(state, "recording_delete", json!({ "clip_id": id })).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_markers(
    params: &ClipsParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let id = require_param!(params.clip_id.as_deref(), "clip_id is required for markers");
    let data = query_addon(state, "recording_markers", json!({ "clip_id": id })).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
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

    let frame = if let Some(f) = params.at_frame {
        session.meta.validate_frame(f)?;
        f
    } else if let Some(t) = params.at_time_ms {
        let (frame, _) = clip_analysis::read_frame_at_time(&session.db, t)?;
        frame
    } else {
        return Err(McpError::invalid_params(
            "snapshot_at requires at_frame or at_time_ms".to_string(),
            None,
        ));
    };

    let detail = params.detail.as_deref().unwrap_or("standard");
    let mut response =
        clip_analysis::snapshot_at(&session.db, frame, detail, budget_limit, hard_cap)?;
    session.finalize(&mut response, budget_limit, hard_cap)
}

async fn handle_query_range(
    params: &ClipsParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let session = clip_analysis::ClipSession::open(state, params.clip_id.as_deref()).await?;

    let node = require_param!(params.node.as_deref(), "query_range requires 'node' parameter");
    let from = require_param!(params.from_frame, "query_range requires 'from_frame'");
    let to = require_param!(params.to_frame, "query_range requires 'to_frame'");
    session.meta.validate_frame(from)?;
    session.meta.validate_frame(to)?;
    let condition: clip_analysis::QueryCondition = params
        .condition
        .as_ref()
        .ok_or_else(|| McpError::invalid_params("query_range requires 'condition'".to_string(), None))
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

    let event_type = require_param!(params.event_type.as_deref(), "find_event requires 'event_type'");

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
        assert_eq!(params.action, "add_marker");
        assert_eq!(params.marker_label.as_deref(), Some("bug here"));
    }

    #[test]
    fn clips_params_save() {
        let json = serde_json::json!({
            "action": "save",
            "marker_label": "suspected physics glitch",
        });
        let params: ClipsParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.action, "save");
        assert_eq!(params.marker_label.as_deref(), Some("suspected physics glitch"));
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
        assert_eq!(params.action, "snapshot_at");
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
        assert_eq!(params.action, "query_range");
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
        assert_eq!(params.event_type.as_deref(), Some("signal"));
        assert_eq!(params.event_filter.as_deref(), Some("health_changed"));
    }

    #[test]
    fn clips_params_status() {
        let json = serde_json::json!({ "action": "status" });
        let params: ClipsParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.action, "status");
    }
}
