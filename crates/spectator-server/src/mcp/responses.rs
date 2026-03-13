//! Typed response schema structs for Spectator MCP tool outputs.
//!
//! These define the MCP output contract for documentation and schema generation.
//! They are documentation-only schemas — the actual handler code still builds
//! serde_json::Value responses for flexibility. The `OutputEntity`, `PaginationBlock`,
//! and `SignalEntry` structs in snapshot.rs are fully typed and derive JsonSchema.

use schemars::JsonSchema;
use serde::Serialize;

use crate::mcp::snapshot::{OutputEntity, PaginationBlock};
use spectator_core::config::SessionConfig;

// ---------------------------------------------------------------------------
// Shared envelope blocks
// ---------------------------------------------------------------------------

/// Token budget block included in every Spectator response.
#[derive(Debug, Serialize, JsonSchema)]
pub struct BudgetBlock {
    /// Estimated tokens used by this response.
    pub used: u32,
    /// Soft token budget limit for this request.
    pub limit: u32,
    /// Hard cap on token budget.
    pub hard_cap: u32,
}

/// Perspective block in snapshot responses.
#[derive(Debug, Serialize, JsonSchema)]
pub struct PerspectiveBlock {
    /// World position of the perspective point.
    pub position: Vec<f64>,
    /// Cardinal direction facing (e.g. "north", "ahead").
    pub facing: String,
    /// Facing direction in degrees.
    pub facing_deg: f64,
}

// ---------------------------------------------------------------------------
// spatial_snapshot responses
// ---------------------------------------------------------------------------

/// Response from spatial_snapshot with detail=summary.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SnapshotSummaryResponse {
    pub frame: u64,
    pub timestamp_ms: u64,
    pub perspective: PerspectiveBlock,
    pub total_nodes_tracked: usize,
    pub total_nodes_visible: usize,
    /// Clusters of entities grouped by the configured clustering strategy.
    pub clusters: Vec<serde_json::Value>,
    pub budget: BudgetBlock,
}

/// Response from spatial_snapshot with detail=standard or full.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SnapshotStandardResponse {
    pub frame: u64,
    pub timestamp_ms: u64,
    pub perspective: PerspectiveBlock,
    pub entities: Vec<OutputEntity>,
    pub pagination: PaginationBlock,
    /// Summary of static geometry nodes (detail=standard).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_summary: Option<serde_json::Value>,
    /// Individual static nodes (detail=full).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_nodes: Option<Vec<serde_json::Value>>,
    pub budget: BudgetBlock,
}

// ---------------------------------------------------------------------------
// spatial_delta
// ---------------------------------------------------------------------------

/// Response from spatial_delta.
#[derive(Debug, Serialize, JsonSchema)]
pub struct DeltaResponse {
    pub from_frame: u64,
    pub to_frame: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moved: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_changed: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entered: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exited: Vec<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub watch_triggers: Vec<serde_json::Value>,
    pub static_changed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals_emitted: Vec<serde_json::Value>,
    pub budget: BudgetBlock,
}

// ---------------------------------------------------------------------------
// spatial_watch
// ---------------------------------------------------------------------------

/// Response from spatial_watch action=add.
#[derive(Debug, Serialize, JsonSchema)]
pub struct WatchAddResponse {
    pub watch_id: String,
    pub node: String,
    pub conditions: String,
    pub track: Vec<serde_json::Value>,
    pub budget: BudgetBlock,
}

/// Response from spatial_watch action=remove.
#[derive(Debug, Serialize, JsonSchema)]
pub struct WatchRemoveResponse {
    /// "ok" or "not_found".
    pub result: String,
    pub watch_id: String,
    pub budget: BudgetBlock,
}

/// Response from spatial_watch action=list.
#[derive(Debug, Serialize, JsonSchema)]
pub struct WatchListResponse {
    pub watches: Vec<WatchListEntry>,
    pub budget: BudgetBlock,
}

/// Entry in the watch list.
#[derive(Debug, Serialize, JsonSchema)]
pub struct WatchListEntry {
    pub watch_id: String,
    pub node: String,
    pub conditions: String,
    pub track: Vec<serde_json::Value>,
}

/// Response from spatial_watch action=clear.
#[derive(Debug, Serialize, JsonSchema)]
pub struct WatchClearResponse {
    pub result: String,
    pub removed: usize,
    pub budget: BudgetBlock,
}

// ---------------------------------------------------------------------------
// spatial_config
// ---------------------------------------------------------------------------

/// Response from spatial_config.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ConfigResponse {
    pub result: String,
    pub config: SessionConfig,
    pub budget: BudgetBlock,
}
