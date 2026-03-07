pub mod action;
pub mod config;
pub mod delta;
pub mod inspect;
pub mod query;
pub mod scene_tree;
pub mod snapshot;
pub mod watch;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ErrorData as McpError;
use rmcp::tool;
use rmcp::tool_router;
use serde::{Deserialize, Serialize};
use spectator_core::{
    bearing,
    budget::{resolve_budget, SnapshotBudgetDefaults},
    index::{IndexedEntity, SpatialIndex},
    types::{vec_to_array3, Position3},
};
use spectator_protocol::query::{
    DetailLevel, GetNodeInspectParams, GetSnapshotDataParams, NodeInspectResponse, SnapshotResponse,
};

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
fn inject_budget(response: &mut serde_json::Value, used: u32, limit: u32, hard_cap: u32) {
    if let serde_json::Value::Object(map) = response {
        map.insert(
            "budget".to_string(),
            serde_json::json!({
                "used": used,
                "limit": limit,
                "hard_cap": hard_cap,
            }),
        );
    }
}
use action::{SpatialActionParams, build_action_request};
use config::{SpatialConfigParams, handle_spatial_config};
use delta::SpatialDeltaParams;
use inspect::{SpatialInspectParams, build_spatial_context, parse_include};
use query::{SpatialQueryParams, handle_spatial_query};
use scene_tree::{SceneTreeToolParams, build_scene_tree_params};
use snapshot::{
    SpatialSnapshotParams, build_expand_response, build_full_response, build_perspective,
    build_perspective_param, build_standard_response, build_summary_response, parse_detail,
};
use watch::SpatialWatchParams;

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

        // 2b. Get current session config
        let config = {
            let s = self.state.lock().await;
            s.config.clone()
        };

        // 3. Query addon for raw data
        let query_params = GetSnapshotDataParams {
            perspective: perspective_param,
            radius: params.radius,
            include_offscreen: params.include_offscreen,
            groups: params.groups.clone().unwrap_or_default(),
            class_filter: params.class_filter.clone().unwrap_or_default(),
            detail,
            expose_internals: config.expose_internals,
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

        // 6b. Rebuild spatial index and store delta baseline
        {
            let indexed: Vec<IndexedEntity> = raw_data
                .entities
                .iter()
                .map(|e| IndexedEntity {
                    path: e.path.clone(),
                    class: e.class.clone(),
                    position: vec_to_array3(&e.position),
                    groups: e.groups.clone(),
                })
                .collect();
            let snapshots: Vec<spectator_core::delta::EntitySnapshot> = raw_data
                .entities
                .iter()
                .map(snapshot::to_entity_snapshot)
                .collect();
            let mut state = self.state.lock().await;
            state.spatial_index = SpatialIndex::build(indexed);
            // 6c. Store snapshot in delta engine for subsequent delta queries
            state.delta_engine.store_snapshot(raw_data.frame, snapshots);
        }

        // 7. Resolve budget
        let tier_default = match detail {
            DetailLevel::Summary => SnapshotBudgetDefaults::SUMMARY,
            DetailLevel::Standard => SnapshotBudgetDefaults::STANDARD,
            DetailLevel::Full => SnapshotBudgetDefaults::FULL,
        };
        let hard_cap = config.token_hard_cap;
        let budget_limit = resolve_budget(params.token_budget, tier_default, hard_cap);

        // 8. Handle expand (drill into a cluster from summary)
        if let Some(ref cluster_label) = params.expand {
            let response = build_expand_response(
                &entities_with_rel,
                cluster_label,
                &raw_data,
                budget_limit,
                hard_cap,
                &config,
            )?;
            return serialize_response(&response);
        }

        // 9. Build response based on detail level
        let response = match detail {
            DetailLevel::Summary => {
                build_summary_response(&raw_data, &entities_with_rel, &persp, budget_limit, hard_cap, &config)
            }
            DetailLevel::Standard => {
                build_standard_response(&raw_data, &entities_with_rel, &persp, budget_limit, hard_cap, &config)
            }
            DetailLevel::Full => {
                build_full_response(&raw_data, &entities_with_rel, &persp, budget_limit, hard_cap, &config)
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
        let config = {
            let s = self.state.lock().await;
            s.config.clone()
        };

        let include = parse_include(&params.include)?;

        let query_params = GetNodeInspectParams {
            path: params.node.clone(),
            include: include.clone(),
            expose_internals: config.expose_internals,
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
        let budget_limit = resolve_budget(None, 1500, config.token_hard_cap);
        inject_budget(&mut response, used, budget_limit, config.token_hard_cap);

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

        let config = {
            let s = self.state.lock().await;
            s.config.clone()
        };

        let json_bytes = serde_json::to_vec(&data).unwrap_or_default().len();
        let used = spectator_core::budget::estimate_tokens(json_bytes);
        let budget_limit = resolve_budget(params.token_budget, 1500, config.token_hard_cap);

        let mut response = data;
        inject_budget(&mut response, used, budget_limit, config.token_hard_cap);

        serialize_response(&response)
    }

    /// Manipulate game state for debugging. Actions: pause (pause/unpause scene),
    /// advance_frames (step N physics frames while paused), advance_time (step N seconds
    /// while paused), teleport (move node to position), set_property (change a property),
    /// call_method (call a method), emit_signal (emit a signal), spawn_node (instantiate
    /// a scene), remove_node (queue_free a node).
    #[tool(description = "Manipulate game state for debugging. Actions: pause (pause/unpause scene), advance_frames (step N physics frames while paused), advance_time (step N seconds while paused), teleport (move node to position), set_property (change a property), call_method (call a method), emit_signal (emit a signal), spawn_node (instantiate a scene), remove_node (queue_free a node). Use return_delta=true to get a spatial delta showing what changed as a result of the action.")]
    pub async fn spatial_action(
        &self,
        Parameters(params): Parameters<SpatialActionParams>,
    ) -> Result<String, McpError> {
        let config = {
            let s = self.state.lock().await;
            s.config.clone()
        };

        let action_request = build_action_request(&params)?;
        let data = query_addon(
            &self.state,
            "execute_action",
            serialize_params(&action_request)?,
        )
        .await?;

        let mut response: serde_json::Value = data;

        let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
        let used = spectator_core::budget::estimate_tokens(json_bytes);
        let action_budget = resolve_budget(None, 500, config.token_hard_cap);
        inject_budget(&mut response, used, action_budget, config.token_hard_cap);

        if params.return_delta {
            let has_baseline = {
                let s = self.state.lock().await;
                s.delta_engine.has_baseline()
            };

            if has_baseline {
                let query_params = spectator_protocol::query::GetSnapshotDataParams {
                    perspective: spectator_protocol::query::PerspectiveParam::Camera,
                    radius: 50.0,
                    include_offscreen: true,
                    groups: vec![],
                    class_filter: vec![],
                    detail: spectator_protocol::query::DetailLevel::Standard,
                    expose_internals: config.expose_internals,
                };

                if let Ok(snap_data) = query_addon(
                    &self.state,
                    "get_snapshot_data",
                    serialize_params(&query_params)?,
                )
                .await
                {
                    if let Ok(raw_data) = serde_json::from_value::<
                        spectator_protocol::query::SnapshotResponse,
                    >(snap_data)
                    {
                        let current_snapshots: Vec<spectator_core::delta::EntitySnapshot> =
                            raw_data
                                .entities
                                .iter()
                                .map(snapshot::to_entity_snapshot)
                                .collect();

                        let mut s = self.state.lock().await;
                        let delta =
                            s.delta_engine.compute_delta(&current_snapshots, raw_data.frame);
                        let triggers = s.watch_engine.evaluate(
                            s.delta_engine.last_snapshot_map(),
                            &current_snapshots,
                            raw_data.frame,
                        );

                        // Update baseline
                        s.delta_engine
                            .store_snapshot(raw_data.frame, current_snapshots);

                        // Build inline delta
                        let mut delta_json = serde_json::json!({
                            "from_frame": delta.from_frame,
                            "to_frame": delta.to_frame,
                        });
                        if let serde_json::Value::Object(ref mut map) = delta_json {
                            if !delta.moved.is_empty() {
                                map.insert(
                                    "moved".into(),
                                    serde_json::to_value(&delta.moved).unwrap_or_default(),
                                );
                            }
                            if !delta.state_changed.is_empty() {
                                map.insert(
                                    "state_changed".into(),
                                    serde_json::to_value(&delta.state_changed).unwrap_or_default(),
                                );
                            }
                            if !delta.entered.is_empty() {
                                map.insert(
                                    "entered".into(),
                                    serde_json::to_value(&delta.entered).unwrap_or_default(),
                                );
                            }
                            if !delta.exited.is_empty() {
                                map.insert(
                                    "exited".into(),
                                    serde_json::to_value(&delta.exited).unwrap_or_default(),
                                );
                            }
                            if !triggers.is_empty() {
                                map.insert(
                                    "watch_triggers".into(),
                                    serde_json::to_value(&triggers).unwrap_or_default(),
                                );
                            }
                        }

                        if let serde_json::Value::Object(ref mut map) = response {
                            map.insert("delta".into(), delta_json);
                        }
                    }
                }
            } else {
                // No baseline — can't compute delta
                if let serde_json::Value::Object(ref mut map) = response {
                    map.insert("delta".into(), serde_json::json!(null));
                    map.insert(
                        "delta_note".into(),
                        serde_json::json!(
                            "No baseline snapshot. Call spatial_snapshot first, \
                             then use return_delta on actions."
                        ),
                    );
                }
            }
        }

        serialize_response(&response)
    }

    /// Targeted spatial questions: nearest nodes, radius search, raycast line-of-sight,
    /// navigation path distance, or mutual relationship between two nodes.
    #[tool(description = "Targeted spatial questions. Query types: 'nearest' (K nearest nodes to a point/node, requires prior spatial_snapshot), 'radius' (all nodes within radius, requires prior spatial_snapshot), 'raycast' (line-of-sight check between two points/nodes), 'path_distance' (navmesh distance), 'relationship' (mutual spatial relationship between two nodes), 'area' (alias for radius).")]
    pub async fn spatial_query(
        &self,
        Parameters(params): Parameters<SpatialQueryParams>,
    ) -> Result<String, McpError> {
        handle_spatial_query(params, &self.state).await
    }

    /// See what changed since the last query. Returns moved entities, state
    /// changes, new/removed nodes, emitted signals, and watch triggers.
    #[tool(description = "See what changed since the last query. Returns moved entities, state changes, new/removed nodes, and watch triggers. Use after spatial_snapshot or spatial_action to see effects. Use since_frame to diff against a specific frame.")]
    pub async fn spatial_delta(
        &self,
        Parameters(params): Parameters<SpatialDeltaParams>,
    ) -> Result<String, McpError> {
        delta::handle_spatial_delta(params, &self.state).await
    }

    /// Subscribe to changes on nodes or groups with optional conditions.
    /// Watch triggers appear in spatial_delta responses.
    #[tool(description = "Subscribe to changes on nodes or groups. Actions: 'add' (subscribe with optional conditions like health < 20), 'remove' (by watch_id), 'list' (show active watches), 'clear' (remove all). Watch triggers appear in spatial_delta responses under 'watch_triggers'.")]
    pub async fn spatial_watch(
        &self,
        Parameters(params): Parameters<SpatialWatchParams>,
    ) -> Result<String, McpError> {
        watch::handle_spatial_watch(params, &self.state).await
    }

    /// Configure tracking behavior — static patterns, state properties,
    /// clustering, bearing format, and token limits. Changes apply for the
    /// current session. Call with no parameters to see current config.
    #[tool(description = "Configure tracking behavior. Set static_patterns (glob patterns for static nodes like [\"walls/*\"]), state_properties (per-group/class property tracking like {\"enemies\": [\"health\"]}), cluster_by (group/class/proximity/none), bearing_format (cardinal/degrees/both), expose_internals (include non-exported vars), poll_interval (collection frequency), token_hard_cap (max tokens per response). Changes apply for the current session.")]
    pub async fn spatial_config(
        &self,
        Parameters(params): Parameters<SpatialConfigParams>,
    ) -> Result<String, McpError> {
        handle_spatial_config(params, &self.state).await
    }
}
