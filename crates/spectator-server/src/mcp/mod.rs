pub mod inspect;
pub mod scene_tree;
pub mod snapshot;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ErrorData as McpError;
use rmcp::tool;
use rmcp::tool_router;
use serde::{Deserialize, Serialize};
use spectator_core::{bearing, budget::SnapshotBudgetDefaults, budget::resolve_budget, types::{Position3, vec_to_array3}};
use spectator_protocol::query::{DetailLevel, GetNodeInspectParams, GetSnapshotDataParams, NodeInspectResponse, SnapshotResponse};

use crate::server::SpectatorServer;
use crate::tcp::query_addon;

// ---------------------------------------------------------------------------
// Shared MCP helpers
// ---------------------------------------------------------------------------

fn serialize_params<T: Serialize>(params: &T) -> Result<serde_json::Value, McpError> {
    serde_json::to_value(params).map_err(|e| {
        McpError::internal_error(format!("Param serialization error: {e}"), None)
    })
}

fn deserialize_response<T: for<'de> Deserialize<'de>>(
    data: serde_json::Value,
) -> Result<T, McpError> {
    serde_json::from_value(data).map_err(|e| {
        McpError::internal_error(format!("Response deserialization error: {e}"), None)
    })
}

fn serialize_response<T: Serialize>(response: &T) -> Result<String, McpError> {
    serde_json::to_string(response).map_err(|e| {
        McpError::internal_error(format!("Response serialization error: {e}"), None)
    })
}

/// Inject a `budget` block into a JSON object value.
fn inject_budget(response: &mut serde_json::Value, used: u32, limit: u32) {
    if let serde_json::Value::Object(map) = response {
        map.insert(
            "budget".to_string(),
            serde_json::json!({
                "used": used,
                "limit": limit,
                "hard_cap": SnapshotBudgetDefaults::HARD_CAP,
            }),
        );
    }
}
use inspect::{SpatialInspectParams, build_spatial_context, parse_include};
use scene_tree::{SceneTreeToolParams, build_scene_tree_params};
use snapshot::{
    SpatialSnapshotParams, build_expand_response, build_full_response, build_perspective,
    build_perspective_param, build_standard_response, build_summary_response, parse_detail,
};

#[tool_router(vis = "pub")]
impl SpectatorServer {
    /// Get a spatial snapshot of the current scene from a perspective.
    /// Use detail 'summary' for a cheap overview (~200 tokens), 'standard' for per-entity data
    /// (~400-800 tokens), or 'full' for everything including transforms, physics, and children
    /// (~1000+ tokens). Start with summary, then drill down.
    #[tool(description = "Get a spatial snapshot of the current scene from a perspective. Use detail 'summary' for a cheap overview (~200 tokens), 'standard' for per-entity data (~400-800 tokens), or 'full' for everything including transforms, physics, and children (~1000+ tokens). Start with summary, then drill down.")]
    pub async fn spatial_snapshot(
        &self,
        Parameters(params): Parameters<SpatialSnapshotParams>,
    ) -> Result<String, McpError> {
        // 1. Parse detail level
        let detail = parse_detail(&params.detail)?;

        // 2. Build perspective param for addon query
        let perspective_param = build_perspective_param(&params)?;

        // 3. Query addon for raw data
        let query_params = GetSnapshotDataParams {
            perspective: perspective_param,
            radius: params.radius,
            include_offscreen: params.include_offscreen,
            groups: params.groups.clone().unwrap_or_default(),
            class_filter: params.class_filter.clone().unwrap_or_default(),
            detail,
        };

        let raw_data: SnapshotResponse = {
            let data = query_addon(&self.state, "get_snapshot_data", serialize_params(&query_params)?)
                .await?;
            deserialize_response(data)?
        };

        // 4. Build perspective for spatial calculations
        let persp = build_perspective(&raw_data.perspective);

        // 5. Compute relative positions and filter by radius/visibility
        let mut entities_with_rel: Vec<_> = raw_data
            .entities
            .iter()
            .filter_map(|e| {
                let pos: Position3 = vec_to_array3(&e.position);
                let rel = bearing::relative_position(&persp, pos, !e.visible);
                if rel.dist > params.radius {
                    return None;
                }
                if !params.include_offscreen && !e.visible {
                    return None;
                }
                Some((e.clone(), rel))
            })
            .collect();

        // 6. Sort by distance (nearest first)
        entities_with_rel.sort_by(|a, b| {
            a.1.dist
                .partial_cmp(&b.1.dist)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 7. Resolve budget
        let tier_default = match detail {
            DetailLevel::Summary => SnapshotBudgetDefaults::SUMMARY,
            DetailLevel::Standard => SnapshotBudgetDefaults::STANDARD,
            DetailLevel::Full => SnapshotBudgetDefaults::FULL,
        };
        let hard_cap = SnapshotBudgetDefaults::HARD_CAP;
        let budget_limit = resolve_budget(params.token_budget, tier_default, hard_cap);

        // 8. Handle expand (drill into a cluster from summary)
        if let Some(ref cluster_label) = params.expand {
            let response = build_expand_response(
                &entities_with_rel,
                cluster_label,
                &raw_data,
                budget_limit,
                hard_cap,
            )?;
            return serialize_response(&response);
        }

        // 9. Build response based on detail level
        let response = match detail {
            DetailLevel::Summary => {
                build_summary_response(&raw_data, &entities_with_rel, &persp, budget_limit, hard_cap)
            }
            DetailLevel::Standard => {
                build_standard_response(&raw_data, &entities_with_rel, &persp, budget_limit, hard_cap)
            }
            DetailLevel::Full => {
                build_full_response(&raw_data, &entities_with_rel, &persp, budget_limit, hard_cap)
            }
        };

        serialize_response(&response)
    }

    /// Deep inspection of a single node — transform, physics, state, children,
    /// signals, script, and spatial context. The "tell me everything about this
    /// one thing" tool.
    #[tool(description = "Deep inspection of a single node. Returns transform, physics, state, children, signals, script, and spatial context. Use the 'include' parameter to select specific categories and reduce token usage. Default includes all categories.")]
    pub async fn spatial_inspect(
        &self,
        Parameters(params): Parameters<SpatialInspectParams>,
    ) -> Result<String, McpError> {
        let include = parse_include(&params.include)?;

        let query_params = GetNodeInspectParams {
            path: params.node.clone(),
            include: include.clone(),
        };

        let raw_data: NodeInspectResponse = {
            let data = query_addon(&self.state, "get_node_inspect", serialize_params(&query_params)?)
                .await?;
            deserialize_response(data)?
        };

        let mut response = serde_json::to_value(&raw_data).map_err(|e| {
            McpError::internal_error(format!("Serialization error: {e}"), None)
        })?;

        if let Some(raw_ctx) = &raw_data.spatial_context_raw {
            let spatial_context = build_spatial_context(raw_ctx);
            if let serde_json::Value::Object(ref mut map) = response {
                map.remove("spatial_context_raw");
                map.insert("spatial_context".to_string(), spatial_context);
            }
        }

        let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
        let used = spectator_core::budget::estimate_tokens(json_bytes);
        inject_budget(&mut response, used, 1500);

        serialize_response(&response)
    }

    /// Navigate and query the Godot scene tree structure. Not spatial — this is
    /// about understanding the node hierarchy.
    #[tool(description = "Navigate the Godot scene tree. Actions: 'roots' (top-level nodes), 'children' (immediate children), 'subtree' (recursive tree with depth limit), 'ancestors' (parent chain to root), 'find' (search by name/class/group/script). Use 'include' to control per-node data.")]
    pub async fn scene_tree(
        &self,
        Parameters(params): Parameters<SceneTreeToolParams>,
    ) -> Result<String, McpError> {
        let query_params = build_scene_tree_params(&params)?;

        let data = query_addon(&self.state, "get_scene_tree", serialize_params(&query_params)?)
            .await?;

        let json_bytes = serde_json::to_vec(&data).unwrap_or_default().len();
        let used = spectator_core::budget::estimate_tokens(json_bytes);
        let budget_limit = resolve_budget(params.token_budget, 1500, SnapshotBudgetDefaults::HARD_CAP);

        let mut response = data;
        inject_budget(&mut response, used, budget_limit);

        serialize_response(&response)
    }
}
