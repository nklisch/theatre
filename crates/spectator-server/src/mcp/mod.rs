pub mod snapshot;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ErrorData as McpError;
use rmcp::tool;
use rmcp::tool_router;
use spectator_core::{bearing, budget::SnapshotBudgetDefaults, budget::resolve_budget, types::Position3};
use spectator_protocol::query::{DetailLevel, GetSnapshotDataParams, SnapshotResponse};

use crate::server::SpectatorServer;
use crate::tcp::query_addon;
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
            let data = query_addon(
                &self.state,
                "get_snapshot_data",
                serde_json::to_value(&query_params).map_err(|e| {
                    McpError::internal_error(format!("Param serialization error: {e}"), None)
                })?,
            )
            .await?;
            serde_json::from_value(data).map_err(|e| {
                McpError::internal_error(format!("Response deserialization error: {e}"), None)
            })?
        };

        // 4. Build perspective for spatial calculations
        let persp = build_perspective(&raw_data.perspective);

        // 5. Compute relative positions and filter by radius/visibility
        let mut entities_with_rel: Vec<_> = raw_data
            .entities
            .iter()
            .filter_map(|e| {
                let pos: Position3 = [
                    e.position.first().copied().unwrap_or(0.0),
                    e.position.get(1).copied().unwrap_or(0.0),
                    e.position.get(2).copied().unwrap_or(0.0),
                ];
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
            return serde_json::to_string(&response).map_err(|e| {
                McpError::internal_error(format!("Serialization error: {e}"), None)
            });
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

        serde_json::to_string(&response).map_err(|e| {
            McpError::internal_error(format!("Response serialization error: {e}"), None)
        })
    }
}
