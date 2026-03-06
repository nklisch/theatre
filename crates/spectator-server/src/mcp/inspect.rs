use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;
use spectator_core::{bearing, types::Position3};
use spectator_protocol::query::{InspectCategory, SpatialContextRaw};

/// Parameters for the spatial_inspect MCP tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialInspectParams {
    /// Node path to inspect (relative to scene root).
    pub node: String,

    /// Which data categories to include.
    /// Options: "transform", "physics", "state", "children", "signals",
    ///          "script", "spatial_context"
    /// Default: all categories.
    #[serde(default = "default_include")]
    pub include: Vec<String>,
}

fn default_include() -> Vec<String> {
    vec![
        "transform".into(),
        "physics".into(),
        "state".into(),
        "children".into(),
        "signals".into(),
        "script".into(),
        "spatial_context".into(),
    ]
}

/// Parse include strings to InspectCategory enums.
pub fn parse_include(strings: &[String]) -> Result<Vec<InspectCategory>, McpError> {
    strings
        .iter()
        .map(|s| match s.as_str() {
            "transform" => Ok(InspectCategory::Transform),
            "physics" => Ok(InspectCategory::Physics),
            "state" => Ok(InspectCategory::State),
            "children" => Ok(InspectCategory::Children),
            "signals" => Ok(InspectCategory::Signals),
            "script" => Ok(InspectCategory::Script),
            "spatial_context" => Ok(InspectCategory::SpatialContext),
            other => Err(McpError::invalid_params(
                format!(
                    "Invalid include category '{other}'. Options: transform, physics, state, children, signals, script, spatial_context"
                ),
                None,
            )),
        })
        .collect()
}

/// Build the spatial_context block from raw addon data.
/// Computes bearings server-side from the raw positions.
pub fn build_spatial_context(raw: &SpatialContextRaw) -> serde_json::Value {
    let node_pos: Position3 = [
        raw.node_position.first().copied().unwrap_or(0.0),
        raw.node_position.get(1).copied().unwrap_or(0.0),
        raw.node_position.get(2).copied().unwrap_or(0.0),
    ];
    let node_fwd = [
        raw.node_forward.first().copied().unwrap_or(0.0),
        raw.node_forward.get(1).copied().unwrap_or(0.0),
        raw.node_forward.get(2).copied().unwrap_or(-1.0),
    ];

    let perspective = bearing::perspective_from_forward(node_pos, node_fwd);

    let nearby_entities: Vec<serde_json::Value> = raw
        .nearby
        .iter()
        .map(|e| {
            let target_pos: Position3 = [
                e.position.first().copied().unwrap_or(0.0),
                e.position.get(1).copied().unwrap_or(0.0),
                e.position.get(2).copied().unwrap_or(0.0),
            ];
            let rel = bearing::relative_position(&perspective, target_pos, false);
            let mut entry = serde_json::json!({
                "path": e.path,
                "dist": rel.dist,
                "bearing": rel.bearing,
                "class": e.class,
            });
            if !e.groups.is_empty() {
                entry["group"] =
                    serde_json::json!(e.groups.first().unwrap_or(&String::new()));
            }
            entry
        })
        .collect();

    serde_json::json!({
        "nearby_entities": nearby_entities,
        "in_areas": raw.in_areas,
        "camera_visible": raw.camera_visible,
        "camera_distance": raw.camera_distance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use spectator_protocol::query::NearbyEntityRaw;

    #[test]
    fn parse_include_valid() {
        let include = vec!["transform".into(), "physics".into()];
        let result = parse_include(&include).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], InspectCategory::Transform);
    }

    #[test]
    fn parse_include_invalid() {
        let include = vec!["invalid".into()];
        assert!(parse_include(&include).is_err());
    }

    #[test]
    fn build_spatial_context_computes_bearing() {
        let raw = SpatialContextRaw {
            nearby: vec![NearbyEntityRaw {
                path: "enemy".into(),
                class: "CharacterBody3D".into(),
                position: vec![0.0, 0.0, -10.0],
                groups: vec!["enemies".into()],
            }],
            in_areas: vec!["zone_a".into()],
            camera_visible: true,
            camera_distance: 15.0,
            node_position: vec![0.0, 0.0, 0.0],
            node_forward: vec![0.0, 0.0, -1.0],
        };
        let ctx = build_spatial_context(&raw);
        let nearby = ctx["nearby_entities"].as_array().unwrap();
        assert_eq!(nearby.len(), 1);
        // Target at [0,0,-10] with forward [0,0,-1] should be "ahead"
        assert_eq!(nearby[0]["bearing"], "ahead");
    }
}
