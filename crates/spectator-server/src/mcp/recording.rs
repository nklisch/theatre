use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use spectator_core::budget::resolve_budget;

use crate::recording_analysis;
use crate::tcp::{SessionState, query_addon};

use super::finalize_response;

// ---------------------------------------------------------------------------
// MCP parameter types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RecordingParams {
    /// Action to perform.
    /// "start" — begin recording.
    /// "stop" — end recording.
    /// "status" — check if recording.
    /// "list" — list saved recordings.
    /// "delete" — remove a recording.
    /// "markers" — list markers in a recording.
    /// "add_marker" — add an agent marker to the active recording.
    pub action: String,

    /// Name for the recording (start only). Auto-generated if omitted.
    pub recording_name: Option<String>,

    /// Capture configuration (start only).
    pub capture: Option<CaptureConfig>,

    /// Recording to query (markers, delete). Uses most recent if omitted.
    pub recording_id: Option<String>,

    /// Marker label (add_marker only).
    pub marker_label: Option<String>,

    /// Frame to attach marker to (add_marker only). Defaults to current frame.
    pub marker_frame: Option<u64>,

    /// Soft token budget for the response.
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CaptureConfig {
    /// Capture every N physics frames. Default 1.
    pub capture_interval: Option<u32>,
    /// Maximum frames to capture. Default 36000.
    pub max_frames: Option<u32>,
    /// Whether to capture signal emissions. Default true.
    pub include_signals: Option<bool>,
    /// Whether to capture input events. Default false.
    pub include_input: Option<bool>,
}

// ---------------------------------------------------------------------------
// Top-level handler
// ---------------------------------------------------------------------------

pub async fn handle_recording(
    params: RecordingParams,
    state: &Arc<Mutex<SessionState>>,
) -> Result<String, McpError> {
    let config = crate::tcp::get_config(state).await;
    let hard_cap = config.token_hard_cap;
    let budget_limit = resolve_budget(params.token_budget, 1500, hard_cap);

    match params.action.as_str() {
        "start" => handle_start(&params, state, budget_limit, hard_cap).await,
        "stop" => handle_stop(state, budget_limit, hard_cap).await,
        "status" => handle_status(state, budget_limit, hard_cap).await,
        "list" => handle_list(state, budget_limit, hard_cap).await,
        "delete" => handle_delete(&params, state, budget_limit, hard_cap).await,
        "markers" => handle_markers(&params, state, budget_limit, hard_cap).await,
        "add_marker" => handle_add_marker(&params, state, budget_limit, hard_cap).await,

        // --- M8 analysis actions ---
        "snapshot_at" => handle_snapshot_at(&params, state, budget_limit, hard_cap).await,
        "query_range" => handle_query_range(&params, state, budget_limit, hard_cap).await,
        "diff_frames" => handle_diff_frames(&params, state, budget_limit, hard_cap).await,
        "find_event" => handle_find_event(&params, state, budget_limit, hard_cap).await,

        other => Err(McpError::invalid_params(
            format!(
                "Unknown recording action: '{other}'. Valid: start, stop, status, list, delete, \
                 markers, add_marker, snapshot_at, query_range, diff_frames, find_event"
            ),
            None,
        )),
    }
}

// ---------------------------------------------------------------------------
// Action handlers
// ---------------------------------------------------------------------------

async fn handle_start(
    params: &RecordingParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let capture = params.capture.as_ref();
    let query_params = json!({
        "name": params.recording_name.as_deref().unwrap_or(""),
        "capture_interval": capture.and_then(|c| c.capture_interval).unwrap_or(1),
        "max_frames": capture.and_then(|c| c.max_frames).unwrap_or(36000),
    });

    let data = query_addon(state, "recording_start", query_params).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_stop(
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let data = query_addon(state, "recording_stop", json!({})).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_status(
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let data = query_addon(state, "recording_status", json!({})).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_list(
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let data = query_addon(state, "recording_list", json!({})).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_delete(
    params: &RecordingParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let id = params.recording_id.as_deref().ok_or_else(|| {
        McpError::invalid_params("recording_id is required for delete".to_string(), None)
    })?;
    let data = query_addon(state, "recording_delete", json!({ "recording_id": id })).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_markers(
    params: &RecordingParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let id = params.recording_id.as_deref().ok_or_else(|| {
        McpError::invalid_params("recording_id is required for markers".to_string(), None)
    })?;
    let data = query_addon(state, "recording_markers", json!({ "recording_id": id })).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}

async fn handle_add_marker(
    params: &RecordingParams,
    state: &Arc<Mutex<SessionState>>,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let query = json!({
        "source": "agent",
        "label": params.marker_label.as_deref().unwrap_or(""),
    });
    let data = query_addon(state, "recording_marker", query).await?;
    let mut response = data;
    finalize_response(&mut response, budget_limit, hard_cap)
}

// ---------------------------------------------------------------------------
// M8 analysis handlers
// ---------------------------------------------------------------------------

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
            "snapshot_at requires at_frame or at_time_ms".to_string(),
            None,
        ));
    };

    let detail = params.detail.as_deref().unwrap_or("standard");
    let mut response =
        recording_analysis::snapshot_at(&db, frame, detail, budget_limit, hard_cap)?;
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
        McpError::invalid_params("query_range requires 'node' parameter".to_string(), None)
    })?;
    let from = params.from_frame.ok_or_else(|| {
        McpError::invalid_params("query_range requires 'from_frame'".to_string(), None)
    })?;
    let to = params.to_frame.ok_or_else(|| {
        McpError::invalid_params("query_range requires 'to_frame'".to_string(), None)
    })?;
    let condition: recording_analysis::QueryCondition = params
        .condition
        .as_ref()
        .ok_or_else(|| McpError::invalid_params("query_range requires 'condition'".to_string(), None))
        .and_then(|v| {
            serde_json::from_value(v.clone())
                .map_err(|e| McpError::invalid_params(format!("Invalid condition: {e}"), None))
        })?;

    let mut response = recording_analysis::query_range(
        &db,
        &storage_path,
        &recording_id,
        node,
        from,
        to,
        &condition,
        budget_limit,
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
        McpError::invalid_params("diff_frames requires 'frame_a'".to_string(), None)
    })?;
    let frame_b = params.frame_b.ok_or_else(|| {
        McpError::invalid_params("diff_frames requires 'frame_b'".to_string(), None)
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
        McpError::invalid_params("find_event requires 'event_type'".to_string(), None)
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
    most_recent_recording(storage_path).ok_or_else(|| {
        McpError::invalid_params(
            "No recording_id specified and no recordings found".to_string(),
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_params_deserializes() {
        let json = serde_json::json!({
            "action": "start",
            "recording_name": "test_rec",
            "capture": {
                "capture_interval": 2,
                "max_frames": 1000,
            }
        });
        let params: RecordingParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.action, "start");
        assert_eq!(params.recording_name.as_deref(), Some("test_rec"));
        assert_eq!(params.capture.as_ref().unwrap().capture_interval, Some(2));
    }

    #[test]
    fn recording_params_minimal_start() {
        let json = serde_json::json!({ "action": "start" });
        let params: RecordingParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.action, "start");
        assert!(params.recording_name.is_none());
        assert!(params.capture.is_none());
    }

    #[test]
    fn recording_params_add_marker() {
        let json = serde_json::json!({
            "action": "add_marker",
            "marker_label": "bug here",
        });
        let params: RecordingParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.action, "add_marker");
        assert_eq!(params.marker_label.as_deref(), Some("bug here"));
    }

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
        assert_eq!(params.detail.as_deref(), Some("standard"));
    }

    #[test]
    fn recording_params_query_range() {
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
        let params: RecordingParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.action, "query_range");
        assert!(params.condition.is_some());
        assert_eq!(params.from_frame, Some(4570));
        assert_eq!(params.to_frame, Some(4590));
    }

    #[test]
    fn recording_params_diff_frames() {
        let json = serde_json::json!({
            "action": "diff_frames",
            "frame_a": 3010u64,
            "frame_b": 3020u64,
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
