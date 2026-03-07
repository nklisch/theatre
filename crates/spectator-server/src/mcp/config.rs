use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;
use spectator_core::cluster::ClusterStrategy;
use spectator_core::config::{BearingFormat, ConfigUpdate};
use std::collections::HashMap;

use super::serialize_response;

fn parse_cluster_by(s: &str) -> Result<ClusterStrategy, McpError> {
    super::parse_enum_param(s, "cluster_by", &[
        ("group", ClusterStrategy::Group),
        ("class", ClusterStrategy::Class),
        ("proximity", ClusterStrategy::Proximity),
        ("none", ClusterStrategy::None),
    ])
}

fn parse_bearing_format(s: &str) -> Result<BearingFormat, McpError> {
    super::parse_enum_param(s, "bearing_format", &[
        ("cardinal", BearingFormat::Cardinal),
        ("degrees", BearingFormat::Degrees),
        ("both", BearingFormat::Both),
    ])
}

/// MCP parameters for the spatial_config tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialConfigParams {
    /// Glob patterns for static node classification.
    /// Nodes matching these are treated as static. Example: ["walls/*", "terrain/*"]
    pub static_patterns: Option<Vec<String>>,

    /// Properties to include in state output per group or class.
    /// Key "*" applies to all nodes. Example: { "enemies": ["health", "alert_level"] }
    pub state_properties: Option<HashMap<String, Vec<String>>>,

    /// How to cluster nodes in summary views: "group", "class", "proximity", or "none".
    pub cluster_by: Option<String>,

    /// Bearing format: "cardinal" (e.g. "ahead_left"), "degrees" (e.g. 322), or "both" (default).
    pub bearing_format: Option<String>,

    /// Include non-exported (internal) variables in state output. Default: false.
    pub expose_internals: Option<bool>,

    /// Collection frequency: every N physics frames. Default: 1.
    pub poll_interval: Option<u32>,

    /// Hard cap on tokens for any single response. Default: 5000.
    pub token_hard_cap: Option<u32>,
}

impl SpatialConfigParams {
    pub fn to_config_update(&self) -> Result<ConfigUpdate, McpError> {
        Ok(ConfigUpdate {
            static_patterns: self.static_patterns.clone(),
            state_properties: self.state_properties.clone(),
            cluster_by: self.cluster_by.as_deref().map(parse_cluster_by).transpose()?,
            bearing_format: self.bearing_format.as_deref().map(parse_bearing_format).transpose()?,
            expose_internals: self.expose_internals,
            poll_interval: self.poll_interval,
            token_hard_cap: self.token_hard_cap,
        })
    }
}

pub async fn handle_spatial_config(
    params: SpatialConfigParams,
    state: &std::sync::Arc<tokio::sync::Mutex<crate::tcp::SessionState>>,
) -> Result<String, McpError> {
    let update = params.to_config_update()?;

    let effective_config = {
        let mut s = state.lock().await;
        s.config.apply_update(&update);
        s.config.clone()
    };

    let response = serde_json::json!({
        "result": "ok",
        "config": effective_config,
        "budget": {
            "used": 50,
            "limit": 200,
            "hard_cap": effective_config.token_hard_cap,
        }
    });

    serialize_response(&response)
}
