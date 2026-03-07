use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

use spectator_core::{
    bearing::{self, perspective_from_forward, perspective_from_yaw},
    budget::{estimate_tokens, resolve_budget, SnapshotBudgetDefaults},
    index::NearestResult,
    types::{vec_to_array3, Position3},
};
use spectator_protocol::query::{
    NavPathResponse, QueryOrigin, RaycastResponse, ResolveNodeResponse, SpatialQueryRequest,
};

use crate::tcp::{query_addon, SessionState};

use super::{deserialize_response, inject_budget, serialize_params, serialize_response};

/// MCP parameters for the spatial_query tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialQueryParams {
    /// Query type: nearest, radius, raycast, area, path_distance, relationship.
    #[schemars(
        description = "Query type: nearest, radius, raycast, area, path_distance, relationship"
    )]
    pub query_type: String,

    /// Origin — node path (string) or world position [x, y, z].
    #[schemars(description = "Origin: node path (string) or position array [x,y,z]")]
    pub from: serde_json::Value,

    /// Target — for raycast, path_distance, and relationship queries.
    #[schemars(description = "Target: node path (string) or position array [x,y,z]")]
    pub to: Option<serde_json::Value>,

    /// For nearest: number of results.
    #[serde(default = "default_k")]
    pub k: usize,

    /// For radius/area: search radius.
    #[serde(default = "default_query_radius")]
    pub radius: f64,

    /// Filter by group membership.
    pub groups: Option<Vec<String>>,

    /// Filter by node class.
    pub class_filter: Option<Vec<String>>,

    /// Token budget for the response.
    pub token_budget: Option<u32>,
}

fn default_k() -> usize {
    5
}
fn default_query_radius() -> f64 {
    20.0
}

/// Parse a `from`/`to` JSON value into a QueryOrigin.
pub fn parse_origin(value: &serde_json::Value) -> Result<QueryOrigin, McpError> {
    match value {
        serde_json::Value::String(s) => Ok(QueryOrigin::Node(s.clone())),
        serde_json::Value::Array(arr) => {
            let coords: Result<Vec<f64>, _> = arr
                .iter()
                .map(|v| {
                    v.as_f64().ok_or_else(|| {
                        McpError::invalid_params(
                            "Position array must contain numbers",
                            None,
                        )
                    })
                })
                .collect();
            Ok(QueryOrigin::Position(coords?))
        }
        _ => Err(McpError::invalid_params(
            "Origin must be a node path (string) or position array [x,y,z]",
            None,
        )),
    }
}

/// Resolve a query origin to a Position3 and optional forward vector.
/// For node origins, queries the addon for the node's position.
pub async fn resolve_origin(
    origin: &QueryOrigin,
    state: &Arc<Mutex<SessionState>>,
) -> Result<(Position3, Option<[f64; 3]>), McpError> {
    match origin {
        QueryOrigin::Position(pos) => Ok((vec_to_array3(pos), None)),
        QueryOrigin::Node(path) => {
            let req = SpatialQueryRequest::ResolveNode { path: path.clone() };
            let data = query_addon(state, "spatial_query", serialize_params(&req)?).await?;
            let resolved: ResolveNodeResponse = deserialize_response(data)?;
            Ok((
                vec_to_array3(&resolved.position),
                Some(vec_to_array3(&resolved.forward)),
            ))
        }
    }
}

fn build_nearest_response(
    results: &[NearestResult],
    from_pos: Position3,
    from_forward: Option<[f64; 3]>,
) -> serde_json::Value {
    let perspective = from_forward
        .map(|fwd| perspective_from_forward(from_pos, fwd))
        .unwrap_or_else(|| perspective_from_yaw(from_pos, 0.0));

    let entries: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            let rel = bearing::relative_position(&perspective, r.position, false);
            serde_json::json!({
                "path": r.path,
                "dist": (r.distance * 10.0).round() / 10.0,
                "bearing": rel.bearing,
                "class": r.class,
            })
        })
        .collect();

    serde_json::json!({
        "query": "nearest",
        "results": entries,
    })
}

fn build_radius_response(
    results: &[NearestResult],
    from_pos: Position3,
    from_forward: Option<[f64; 3]>,
    radius: f64,
) -> serde_json::Value {
    let perspective = from_forward
        .map(|fwd| perspective_from_forward(from_pos, fwd))
        .unwrap_or_else(|| perspective_from_yaw(from_pos, 0.0));

    let entries: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            let rel = bearing::relative_position(&perspective, r.position, false);
            serde_json::json!({
                "path": r.path,
                "dist": (r.distance * 10.0).round() / 10.0,
                "bearing": rel.bearing,
                "class": r.class,
            })
        })
        .collect();

    serde_json::json!({
        "query": "radius",
        "radius": radius,
        "results": entries,
    })
}

pub async fn build_relationship_response(
    from_origin: &QueryOrigin,
    to_origin: &QueryOrigin,
    from_pos: Position3,
    from_forward: Option<[f64; 3]>,
    to_pos: Position3,
    to_forward: Option<[f64; 3]>,
    state: &Arc<Mutex<SessionState>>,
) -> Result<serde_json::Value, McpError> {
    let distance = bearing::distance(from_pos, to_pos);

    let persp_a = from_forward
        .map(|fwd| perspective_from_forward(from_pos, fwd))
        .unwrap_or_else(|| perspective_from_yaw(from_pos, 0.0));
    let rel_a_to_b = bearing::relative_position(&persp_a, to_pos, false);

    let persp_b = to_forward
        .map(|fwd| perspective_from_forward(to_pos, fwd))
        .unwrap_or_else(|| perspective_from_yaw(to_pos, 0.0));
    let rel_b_to_a = bearing::relative_position(&persp_b, from_pos, false);

    // Raycast for line of sight
    let raycast_req = SpatialQueryRequest::Raycast {
        from: from_origin.clone(),
        to: to_origin.clone(),
        collision_mask: None,
    };
    let raycast_data =
        query_addon(state, "spatial_query", serialize_params(&raycast_req)?).await?;
    let raycast: RaycastResponse = deserialize_response(raycast_data)?;

    // Optional nav distance
    let nav_distance = {
        let nav_req = SpatialQueryRequest::PathDistance {
            from: from_origin.clone(),
            to: to_origin.clone(),
        };
        match query_addon(state, "spatial_query", serialize_params(&nav_req)?).await {
            Ok(data) => {
                let nav: NavPathResponse = deserialize_response(data)?;
                if nav.traversable { Some(nav.nav_distance) } else { None }
            }
            Err(_) => None,
        }
    };

    let mut result = serde_json::json!({
        "distance": (distance * 10.0).round() / 10.0,
        "bearing_from_a": rel_a_to_b.bearing,
        "bearing_from_b": rel_b_to_a.bearing,
        "line_of_sight": raycast.clear,
    });

    if let Some(elev) = &rel_a_to_b.elevation {
        use spectator_core::types::Elevation;
        result["elevation_diff"] = match elev {
            Elevation::Level => serde_json::json!(0.0),
            Elevation::Above(d) => serde_json::json!(d),
            Elevation::Below(d) => serde_json::json!(-d),
        };
    }
    if !raycast.clear {
        if let Some(ref occ) = raycast.blocked_by {
            result["occluder"] = serde_json::json!(occ);
        }
    }
    if let Some(nav) = nav_distance {
        result["nav_distance"] = serde_json::json!((nav * 10.0).round() / 10.0);
    }

    Ok(serde_json::json!({
        "query": "relationship",
        "from": from_origin,
        "to": to_origin,
        "result": result,
    }))
}

/// Handle a spatial_query MCP call. Returns the JSON response string.
pub async fn handle_spatial_query(
    params: SpatialQueryParams,
    state: &Arc<Mutex<SessionState>>,
) -> Result<String, McpError> {
    let from_origin = parse_origin(&params.from)?;
    let groups = params.groups.as_deref().unwrap_or(&[]);
    let class_filter = params.class_filter.as_deref().unwrap_or(&[]);
    let budget_limit = resolve_budget(
        params.token_budget,
        500,
        SnapshotBudgetDefaults::HARD_CAP,
    );

    let mut response = match params.query_type.as_str() {
        "nearest" => {
            let (from_pos, from_fwd) = resolve_origin(&from_origin, state).await?;
            let results = {
                let s = state.lock().await;
                s.spatial_index.nearest(from_pos, params.k, groups, class_filter)
            };
            build_nearest_response(&results, from_pos, from_fwd)
        }
        "radius" | "area" => {
            let (from_pos, from_fwd) = resolve_origin(&from_origin, state).await?;
            let results = {
                let s = state.lock().await;
                s.spatial_index
                    .within_radius(from_pos, params.radius, groups, class_filter)
            };
            build_radius_response(&results, from_pos, from_fwd, params.radius)
        }
        "raycast" => {
            let to_val = params.to.as_ref().ok_or_else(|| {
                McpError::invalid_params("'to' is required for raycast query", None)
            })?;
            let to_origin = parse_origin(to_val)?;
            let req = SpatialQueryRequest::Raycast {
                from: from_origin.clone(),
                to: to_origin,
                collision_mask: None,
            };
            let data =
                query_addon(state, "spatial_query", serialize_params(&req)?).await?;
            let raycast: RaycastResponse = deserialize_response(data)?;
            serde_json::json!({
                "query": "raycast",
                "from": params.from,
                "to": params.to,
                "result": raycast,
            })
        }
        "path_distance" => {
            let to_val = params.to.as_ref().ok_or_else(|| {
                McpError::invalid_params("'to' is required for path_distance query", None)
            })?;
            let to_origin = parse_origin(to_val)?;
            let req = SpatialQueryRequest::PathDistance {
                from: from_origin,
                to: to_origin,
            };
            let data =
                query_addon(state, "spatial_query", serialize_params(&req)?).await?;
            let nav: NavPathResponse = deserialize_response(data)?;
            serde_json::json!({
                "query": "path_distance",
                "from": params.from,
                "to": params.to,
                "result": nav,
            })
        }
        "relationship" => {
            let to_val = params.to.as_ref().ok_or_else(|| {
                McpError::invalid_params("'to' is required for relationship query", None)
            })?;
            let to_origin = parse_origin(to_val)?;
            let (from_pos, from_fwd) = resolve_origin(&from_origin, state).await?;
            let (to_pos, to_fwd) = resolve_origin(&to_origin, state).await?;
            build_relationship_response(
                &from_origin,
                &to_origin,
                from_pos,
                from_fwd,
                to_pos,
                to_fwd,
                state,
            )
            .await?
        }
        other => {
            return Err(McpError::invalid_params(
                format!(
                    "Unknown query_type: '{other}'. Valid types: \
                     nearest, radius, raycast, path_distance, relationship, area"
                ),
                None,
            ));
        }
    };

    // Add "from" field if not already present
    if let serde_json::Value::Object(ref mut map) = response {
        if !map.contains_key("from") {
            map.insert("from".into(), params.from.clone());
        }
    }

    // Inject budget
    let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
    let used = estimate_tokens(json_bytes);
    inject_budget(&mut response, used, budget_limit);

    serialize_response(&response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_origin_string() {
        let val = serde_json::json!("player");
        let origin = parse_origin(&val).unwrap();
        assert!(matches!(origin, QueryOrigin::Node(s) if s == "player"));
    }

    #[test]
    fn parse_origin_array() {
        let val = serde_json::json!([1.0, 2.0, 3.0]);
        let origin = parse_origin(&val).unwrap();
        assert!(matches!(origin, QueryOrigin::Position(v) if v.len() == 3));
    }

    #[test]
    fn parse_origin_invalid() {
        let val = serde_json::json!(42);
        assert!(parse_origin(&val).is_err());
    }

    #[test]
    fn parse_origin_bool_invalid() {
        let val = serde_json::json!(true);
        assert!(parse_origin(&val).is_err());
    }
}
