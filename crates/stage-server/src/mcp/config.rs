use schemars::JsonSchema;
use serde::Deserialize;
use stage_core::budget::resolve_budget;
use stage_core::cluster::ClusterStrategy;
use stage_core::config::{BearingFormat, ConfigUpdate};
use std::collections::HashMap;

use super::finalize_response;

/// MCP parameters for the spatial_config tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialConfigParams {
    /// Glob patterns for static node classification.
    /// Nodes matching these are treated as static. Example: ["walls/*", "terrain/*"]
    pub static_patterns: Option<Vec<String>>,

    /// Properties to include in state output per group or class.
    /// Key "*" applies to all nodes. Example: { "enemies": ["health", "alert_level"] }
    pub state_properties: Option<HashMap<String, Vec<String>>>,

    /// How to cluster nodes in summary views.
    pub cluster_by: Option<ClusterStrategy>,

    /// Bearing format preference.
    pub bearing_format: Option<BearingFormat>,

    /// Include non-exported (internal) variables in state output. Default: false.
    pub expose_internals: Option<bool>,

    /// Collection frequency: every N physics frames. Default: 1.
    pub poll_interval: Option<u32>,

    /// Hard cap on tokens for any single response. Default: 5000.
    pub token_hard_cap: Option<u32>,
}

impl SpatialConfigParams {
    pub fn to_config_update(&self) -> ConfigUpdate {
        ConfigUpdate {
            static_patterns: self.static_patterns.clone(),
            state_properties: self.state_properties.clone(),
            cluster_by: self.cluster_by,
            bearing_format: self.bearing_format,
            expose_internals: self.expose_internals,
            poll_interval: self.poll_interval,
            token_hard_cap: self.token_hard_cap,
            // Dashcam config fields are not exposed via spatial_config (set via dashcam_config TCP method).
            ..Default::default()
        }
    }
}

pub async fn handle_spatial_config(
    params: SpatialConfigParams,
    state: &std::sync::Arc<tokio::sync::Mutex<crate::tcp::SessionState>>,
) -> Result<String, rmcp::model::ErrorData> {
    let update = params.to_config_update();

    let effective_config = {
        let mut s = state.lock().await;
        s.config.apply_update(&update);
        s.config.clone()
    };

    let mut response = serde_json::json!({
        "result": "ok",
        "config": effective_config,
    });

    let budget_limit = resolve_budget(None, 200, effective_config.token_hard_cap);
    finalize_response(&mut response, budget_limit, effective_config.token_hard_cap)
}
