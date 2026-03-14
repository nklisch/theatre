use schemars::JsonSchema;
use serde::Deserialize;
use stage_core::{
    bearing,
    types::{Position3, vec_to_array3},
};
use stage_protocol::query::{InspectCategory, SpatialContextRaw};

/// Parameters for the spatial_inspect MCP tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialInspectParams {
    /// Node path to inspect (relative to scene root).
    pub node: String,

    /// Which data categories to include. Default: all except resources (opt-in to save tokens).
    #[serde(default = "default_include")]
    pub include: Vec<InspectCategory>,
}

fn default_include() -> Vec<InspectCategory> {
    vec![
        InspectCategory::Transform,
        InspectCategory::Physics,
        InspectCategory::State,
        InspectCategory::Children,
        InspectCategory::Signals,
        InspectCategory::Script,
        InspectCategory::SpatialContext,
    ]
}

/// Build the spatial_context block from raw addon data.
/// Computes bearings server-side from the raw positions.
pub fn build_spatial_context(raw: &SpatialContextRaw) -> serde_json::Value {
    let node_pos: Position3 = vec_to_array3(&raw.node_position);
    // z defaults to -1.0 (Godot forward) in case the addon sends an incomplete vector
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
            let target_pos: Position3 = vec_to_array3(&e.position);
            let rel = bearing::relative_position(&perspective, target_pos, false);
            let mut entry = serde_json::json!({
                "path": e.path,
                "distance": rel.distance,
                "bearing": rel.bearing,
                "class": e.class,
            });
            if !e.groups.is_empty() {
                entry["group"] = serde_json::json!(e.groups.first().unwrap_or(&String::new()));
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
    use stage_protocol::query::NearbyEntityRaw;

    #[test]
    fn parse_include_valid() {
        let inc: Vec<InspectCategory> = serde_json::from_str(r#"["transform","physics"]"#).unwrap();
        assert_eq!(inc.len(), 2);
        assert_eq!(inc[0], InspectCategory::Transform);
    }

    #[test]
    fn parse_include_invalid() {
        assert!(serde_json::from_str::<InspectCategory>(r#""invalid""#).is_err());
    }

    #[test]
    fn parse_include_resources() {
        let inc: Vec<InspectCategory> = serde_json::from_str(r#"["resources"]"#).unwrap();
        assert_eq!(inc.len(), 1);
        assert_eq!(inc[0], InspectCategory::Resources);
    }

    #[test]
    fn default_include_excludes_resources() {
        let defaults = default_include();
        assert!(!defaults.contains(&InspectCategory::Resources));
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
