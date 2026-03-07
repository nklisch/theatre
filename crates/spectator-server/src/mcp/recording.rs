use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use spectator_core::budget::resolve_budget;

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
        other => Err(McpError::invalid_params(
            format!(
                "Unknown recording action: '{other}'. Valid: start, stop, status, list, delete, markers, add_marker"
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
}
