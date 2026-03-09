use rmcp::model::ErrorData as McpError;

use super::defaults::{default_detail, default_perspective, default_radius};
use super::require_param;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use spectator_core::{
    bearing,
    budget::BudgetEnforcer,
    cluster::{self, Cluster},
    config::{BearingFormat, SessionConfig},
    delta::EntitySnapshot,
    types::{Perspective, Position3, RawEntityData, RecentSignal, RelativePosition, vec_to_array3},
};
use spectator_protocol::query::{DetailLevel, EntityData, PerspectiveParam, SnapshotResponse};
use spectator_protocol::static_classes::{classify_static_category, is_static_class};

/// Parameters for the spatial_snapshot MCP tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialSnapshotParams {
    /// Perspective type: "camera", "node", or "point". Default: "camera".
    #[serde(default = "default_perspective")]
    #[schemars(
        description = "Where to look from: \"camera\" (active camera, default), \"node\" (requires focal_node), \"point\" (requires focal_point)"
    )]
    pub perspective: String,

    /// Node path, required when perspective is "node".
    pub focal_node: Option<String>,

    /// World position [x, y, z], required when perspective is "point".
    pub focal_point: Option<Vec<f64>>,

    /// Max distance from perspective to include. Default: 50.0.
    #[serde(default = "default_radius")]
    pub radius: f64,

    /// Detail tier: "summary", "standard", or "full". Default: "standard".
    #[serde(default = "default_detail")]
    #[schemars(
        description = "Detail tier: \"summary\" (~200 tokens, clusters only), \"standard\" (~400-800 tokens, per-entity), \"full\" (~1000+ tokens, transforms/physics/children)"
    )]
    pub detail: String,

    /// Filter by group membership.
    pub groups: Option<Vec<String>>,

    /// Filter by node class.
    pub class_filter: Option<Vec<String>>,

    /// Include nodes outside camera frustum. Default: false.
    #[serde(default)]
    pub include_offscreen: bool,

    /// Soft token budget override.
    pub token_budget: Option<u32>,

    /// Expand a cluster from a previous summary response.
    pub expand: Option<String>,
}


/// Processed entity for MCP output.
#[derive(Debug, Serialize)]
pub struct OutputEntity {
    pub path: String,
    pub class: String,
    pub relative: serde_json::Value,
    pub global_position: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation_y_deg: Option<f64>,
    /// 2D rotation angle in degrees (present only for 2D entities).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation_deg: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub velocity: Option<Vec<f64>>,
    pub groups: Vec<String>,
    pub state: serde_json::Map<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub signals_recent: Vec<SignalEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub physics: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signals_connected: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_exported_vars: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Serialize)]
pub struct SignalEntry {
    pub signal: String,
    pub frame: u64,
}

#[derive(Debug, Serialize)]
pub struct PaginationBlock {
    pub truncated: bool,
    pub showing: usize,
    pub total: usize,
    pub cursor: String,
    pub omitted_nearest_distance: f64,
}

/// Convert protocol EntityData to a delta-compatible EntitySnapshot.
pub fn to_entity_snapshot(e: &EntityData) -> EntitySnapshot {
    EntitySnapshot {
        path: e.path.clone(),
        class: e.class.clone(),
        position: vec_to_array3(&e.position),
        rotation_deg: vec_to_array3(&e.rotation_deg),
        velocity: vec_to_array3(&e.velocity),
        groups: e.groups.clone(),
        state: e.state.clone(),
        visible: e.visible,
    }
}

pub fn parse_detail(s: &str) -> Result<DetailLevel, McpError> {
    super::parse_enum_param(
        s,
        "detail level",
        &[
            ("summary", DetailLevel::Summary),
            ("standard", DetailLevel::Standard),
            ("full", DetailLevel::Full),
        ],
    )
}

pub fn build_perspective_param(
    params: &SpatialSnapshotParams,
) -> Result<PerspectiveParam, McpError> {
    match params.perspective.as_str() {
        "camera" => Ok(PerspectiveParam::Camera),
        "node" => {
            let path = require_param!(
                params.focal_node.as_ref(),
                "focal_node is required when perspective is 'node'"
            );
            Ok(PerspectiveParam::Node { path: path.clone() })
        }
        "point" => {
            let pos = require_param!(
                params.focal_point.as_ref(),
                "focal_point is required when perspective is 'point'"
            );
            Ok(PerspectiveParam::Point {
                position: pos.clone(),
            })
        }
        other => Err(McpError::invalid_params(
            format!("Invalid perspective '{other}'. Must be 'camera', 'node', or 'point'."),
            None,
        )),
    }
}

pub fn build_perspective(data: &spectator_protocol::query::PerspectiveData) -> Perspective {
    if data.position.len() == 2 {
        // 2D perspective: pad position/forward to 3D, use XY-plane bearing
        let position: Position3 = [data.position[0], data.position[1], 0.0];
        let forward = [
            data.forward.first().copied().unwrap_or(1.0),
            data.forward.get(1).copied().unwrap_or(0.0),
            0.0,
        ];
        // For 2D, compute facing from the 2D forward angle (X-axis = 0°)
        let angle_deg = forward[1].atan2(forward[0]).to_degrees();
        let facing_deg = ((angle_deg % 360.0) + 360.0) % 360.0;
        let facing = bearing::to_cardinal(facing_deg);
        Perspective {
            position,
            forward,
            facing,
            facing_deg,
        }
    } else {
        let position: Position3 = [
            data.position.first().copied().unwrap_or(0.0),
            data.position.get(1).copied().unwrap_or(0.0),
            data.position.get(2).copied().unwrap_or(0.0),
        ];
        let forward = [
            data.forward.first().copied().unwrap_or(0.0),
            data.forward.get(1).copied().unwrap_or(0.0),
            data.forward.get(2).copied().unwrap_or(-1.0),
        ];
        let (facing, facing_deg) = bearing::compass_bearing(forward);
        Perspective {
            position,
            forward,
            facing,
            facing_deg,
        }
    }
}

/// Format relative position according to bearing format config.
fn format_rel(rel: &RelativePosition, format: BearingFormat) -> serde_json::Value {
    match format {
        BearingFormat::Both => serde_json::to_value(rel).unwrap_or_default(),
        BearingFormat::Cardinal => serde_json::json!({
            "distance": rel.dist,
            "bearing": rel.bearing,
            "elevation": rel.elevation,
            "occluded": rel.occluded,
        }),
        BearingFormat::Degrees => serde_json::json!({
            "distance": rel.dist,
            "bearing_deg": rel.bearing_deg,
            "elevation": rel.elevation,
            "occluded": rel.occluded,
        }),
    }
}

pub fn build_output_entity(
    entity: &EntityData,
    rel: &RelativePosition,
    full: bool,
    config: &SessionConfig,
) -> OutputEntity {
    let velocity = if entity.velocity.iter().any(|v| v.abs() > 0.01) {
        Some(entity.velocity.clone())
    } else {
        None
    };

    // Filter state properties based on config
    let state = match config.filter_state_properties(&entity.groups, &entity.class) {
        Some(allowed_props) => entity
            .state
            .iter()
            .filter(|(k, _)| allowed_props.contains(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        None => entity.state.clone(),
    };

    let is_2d = entity.position.len() == 2;

    OutputEntity {
        path: entity.path.clone(),
        class: entity.class.clone(),
        relative: format_rel(rel, config.bearing_format),
        global_position: entity.position.clone(),
        rotation_y_deg: if is_2d {
            None
        } else {
            entity.rotation_deg.get(1).copied()
        },
        rotation_deg: if is_2d {
            entity.rotation_deg.first().copied()
        } else {
            None
        },
        velocity,
        groups: entity.groups.clone(),
        state,
        signals_recent: entity
            .signals_recent
            .iter()
            .map(|s| SignalEntry {
                signal: s.signal.clone(),
                frame: s.frame,
            })
            .collect(),
        transform: if full {
            entity
                .transform
                .as_ref()
                .map(|t| serde_json::to_value(t).unwrap_or_default())
        } else {
            None
        },
        physics: if full {
            entity
                .physics
                .as_ref()
                .map(|p| serde_json::to_value(p).unwrap_or_default())
        } else {
            None
        },
        children: if full {
            Some(serde_json::to_value(&entity.children).unwrap_or_default())
        } else {
            None
        },
        script: if full { entity.script.clone() } else { None },
        signals_connected: if full {
            Some(entity.signals_connected.clone())
        } else {
            None
        },
        all_exported_vars: if full {
            entity.all_exported_vars.clone()
        } else {
            None
        },
    }
}

fn is_entity_static(entity: &EntityData, config: &SessionConfig) -> bool {
    config.matches_static_pattern(&entity.path) || is_static_class(&entity.class)
}

/// Convert EntityData to RawEntityData for use with the clustering engine.
fn to_raw_entity(e: &EntityData, config: &SessionConfig) -> RawEntityData {
    RawEntityData {
        path: e.path.clone(),
        class: e.class.clone(),
        position: vec_to_array3(&e.position),
        rotation_deg: vec_to_array3(&e.rotation_deg),
        velocity: vec_to_array3(&e.velocity),
        groups: e.groups.clone(),
        state: e.state.clone(),
        visible: e.visible,
        is_static: is_entity_static(e, config),
        children: Vec::new(),
        script: e.script.clone(),
        signals_recent: e
            .signals_recent
            .iter()
            .map(|s| RecentSignal {
                signal: s.signal.clone(),
                frame: s.frame,
            })
            .collect(),
        signals_connected: e.signals_connected.clone(),
        physics: None,
        transform: None,
    }
}

fn perspective_json(
    raw: &spectator_protocol::query::PerspectiveData,
    persp: &Perspective,
) -> serde_json::Value {
    serde_json::json!({
        "position": raw.position,
        "facing": persp.facing,
        "facing_deg": persp.facing_deg,
    })
}

pub fn build_summary_response(
    raw: &SnapshotResponse,
    entities: &[(EntityData, RelativePosition)],
    perspective: &Perspective,
    budget_limit: u32,
    hard_cap: u32,
    config: &SessionConfig,
) -> serde_json::Value {
    let raw_entities: Vec<(RawEntityData, RelativePosition)> = entities
        .iter()
        .map(|(e, rel)| (to_raw_entity(e, config), rel.clone()))
        .collect();

    let clusters = cluster::cluster_entities(&raw_entities, config.cluster_by);

    let total = entities.len();
    let visible = entities.iter().filter(|(e, _)| e.visible).count();

    let mut enforcer = BudgetEnforcer::new(budget_limit, hard_cap);
    enforcer.try_add(200); // perspective + frame + metadata overhead

    let mut output_clusters: Vec<&Cluster> = Vec::new();
    for c in &clusters {
        let bytes = serde_json::to_vec(c).unwrap_or_default();
        if !enforcer.try_add(bytes.len()) {
            break;
        }
        output_clusters.push(c);
    }

    serde_json::json!({
        "frame": raw.frame,
        "timestamp_ms": raw.timestamp_ms,
        "perspective": perspective_json(&raw.perspective, perspective),
        "clusters": output_clusters,
        "total_nodes_tracked": total,
        "total_nodes_visible": visible,
        "budget": enforcer.report(),
    })
}

enum SnapshotTier {
    Standard,
    Full,
}

fn build_snapshot_body(
    raw: &SnapshotResponse,
    entities: &[(EntityData, RelativePosition)],
    perspective: &Perspective,
    budget_limit: u32,
    hard_cap: u32,
    config: &SessionConfig,
    tier: SnapshotTier,
) -> serde_json::Value {
    let mut enforcer = BudgetEnforcer::new(budget_limit, hard_cap);
    enforcer.try_add(200);

    let full = matches!(tier, SnapshotTier::Full);
    let mut dynamic_entities: Vec<OutputEntity> = Vec::new();
    // Standard: accumulate count + category summary; Full: individual static nodes
    let mut static_count = 0usize;
    let mut static_categories: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    let mut static_nodes: Vec<serde_json::Value> = Vec::new();
    let total = entities.len();

    for (entity, rel) in entities {
        if is_entity_static(entity, config) {
            if full {
                let node = serde_json::json!({
                    "path": entity.path,
                    "class": entity.class,
                    "position": entity.position,
                });
                let bytes = serde_json::to_vec(&node).unwrap_or_default();
                if enforcer.try_add(bytes.len()) {
                    static_nodes.push(node);
                }
            } else {
                static_count += 1;
                let cat = classify_static_category(&entity.class).to_string();
                let counter = static_categories.entry(cat).or_insert(serde_json::json!(0));
                if let Some(n) = counter.as_u64() {
                    *counter = serde_json::json!(n + 1);
                }
            }
            continue;
        }

        let out = build_output_entity(entity, rel, full, config);
        let bytes = serde_json::to_vec(&out).unwrap_or_default();
        if !enforcer.try_add(bytes.len()) {
            let pagination = PaginationBlock {
                truncated: true,
                showing: dynamic_entities.len(),
                total,
                cursor: format!("snap_{}_p{}", raw.frame, dynamic_entities.len()),
                omitted_nearest_distance: rel.dist,
            };
            let mut resp = serde_json::json!({
                "frame": raw.frame,
                "timestamp_ms": raw.timestamp_ms,
                "perspective": perspective_json(&raw.perspective, perspective),
                "entities": dynamic_entities,
                "pagination": pagination,
                "budget": enforcer.report(),
            });
            if full {
                resp["static_nodes"] = serde_json::json!(static_nodes);
            } else {
                resp["static_summary"] =
                    serde_json::json!({ "count": static_count, "categories": static_categories });
            }
            return resp;
        }
        dynamic_entities.push(out);
    }

    let mut resp = serde_json::json!({
        "frame": raw.frame,
        "timestamp_ms": raw.timestamp_ms,
        "perspective": perspective_json(&raw.perspective, perspective),
        "entities": dynamic_entities,
        "budget": enforcer.report(),
    });
    if full {
        resp["static_nodes"] = serde_json::json!(static_nodes);
    } else {
        resp["static_summary"] =
            serde_json::json!({ "count": static_count, "categories": static_categories });
    }
    resp
}

pub fn build_standard_response(
    raw: &SnapshotResponse,
    entities: &[(EntityData, RelativePosition)],
    perspective: &Perspective,
    budget_limit: u32,
    hard_cap: u32,
    config: &SessionConfig,
) -> serde_json::Value {
    build_snapshot_body(
        raw,
        entities,
        perspective,
        budget_limit,
        hard_cap,
        config,
        SnapshotTier::Standard,
    )
}

pub fn build_full_response(
    raw: &SnapshotResponse,
    entities: &[(EntityData, RelativePosition)],
    perspective: &Perspective,
    budget_limit: u32,
    hard_cap: u32,
    config: &SessionConfig,
) -> serde_json::Value {
    build_snapshot_body(
        raw,
        entities,
        perspective,
        budget_limit,
        hard_cap,
        config,
        SnapshotTier::Full,
    )
}

pub fn build_expand_response(
    entities: &[(EntityData, RelativePosition)],
    cluster_label: &str,
    raw: &SnapshotResponse,
    budget_limit: u32,
    hard_cap: u32,
    config: &SessionConfig,
) -> Result<serde_json::Value, McpError> {
    let matching: Vec<&(EntityData, RelativePosition)> = entities
        .iter()
        .filter(|(e, _)| {
            e.groups.first().map(|g| g.as_str()) == Some(cluster_label)
                || (cluster_label == "other" && e.groups.is_empty())
        })
        .collect();

    if matching.is_empty() {
        return Err(McpError::invalid_params(
            format!(
                "No cluster named '{cluster_label}' found. Use spatial_snapshot(detail: 'summary') to see available clusters."
            ),
            None,
        ));
    }

    let mut enforcer = BudgetEnforcer::new(budget_limit, hard_cap);
    let mut output_entities: Vec<OutputEntity> = Vec::new();

    for (entity, rel) in &matching {
        let out = build_output_entity(entity, rel, false, config);
        let bytes = serde_json::to_vec(&out).unwrap_or_default();
        if !enforcer.try_add(bytes.len()) {
            break;
        }
        output_entities.push(out);
    }

    Ok(serde_json::json!({
        "frame": raw.frame,
        "timestamp_ms": raw.timestamp_ms,
        "expand": cluster_label,
        "entities": output_entities,
        "budget": enforcer.report(),
    }))
}
