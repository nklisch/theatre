pub mod action;
pub mod config;
pub mod delta;
pub mod inspect;
pub mod query;
pub mod recording;
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
    index::{IndexedEntity, IndexedEntity2D, SpatialIndex},
    types::{vec_to_array2, vec_to_array3},
};
use spectator_protocol::query::{
    DetailLevel, GetNodeInspectParams, GetSnapshotDataParams, NodeInspectResponse, SnapshotResponse,
};

use crate::server::SpectatorServer;
use crate::tcp::{get_config, query_addon};

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

/// Extract a required parameter, returning McpError::invalid_params if None.
macro_rules! require_param {
    ($expr:expr, $msg:expr) => {
        $expr.ok_or_else(|| McpError::invalid_params($msg, None))?
    };
}
use require_param;

/// Query the addon and deserialize the response in one step.
async fn query_and_deserialize<P: Serialize, R: for<'de> Deserialize<'de>>(
    state: &std::sync::Arc<tokio::sync::Mutex<crate::tcp::SessionState>>,
    method: &str,
    params: &P,
) -> Result<R, McpError> {
    let data = query_addon(state, method, serialize_params(params)?).await?;
    deserialize_response(data)
}

/// Parse a string into an enum variant, returning McpError::invalid_params on mismatch.
fn parse_enum_param<T: Clone>(
    value: &str,
    field_name: &str,
    variants: &[(&str, T)],
) -> Result<T, McpError> {
    for (name, variant) in variants {
        if *name == value {
            return Ok(variant.clone());
        }
    }
    let valid: Vec<&str> = variants.iter().map(|(n, _)| *n).collect();
    Err(McpError::invalid_params(
        format!("Invalid {field_name} '{value}'. Valid: {}", valid.join(", ")),
        None,
    ))
}

/// Parse a list of strings into enum variants.
fn parse_enum_list<T: Clone>(
    values: &[String],
    field_name: &str,
    variants: &[(&str, T)],
) -> Result<Vec<T>, McpError> {
    values
        .iter()
        .map(|s| parse_enum_param(s, field_name, variants))
        .collect()
}

/// Estimate token usage, inject budget block, and serialize response to JSON string.
pub(crate) fn finalize_response(
    response: &mut serde_json::Value,
    budget_limit: u32,
    hard_cap: u32,
) -> Result<String, McpError> {
    let json_bytes = serde_json::to_vec(response).unwrap_or_default().len();
    let used = spectator_core::budget::estimate_tokens(json_bytes);
    inject_budget(response, used, budget_limit, hard_cap);
    serialize_response(response)
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
        // Build activity summary up front before params are borrowed further
        let activity_summary = crate::activity::snapshot_summary(&params);

        // 1. Parse detail level
        let detail = parse_detail(&params.detail)?;

        // 2. Build perspective param for addon query
        let perspective_param = build_perspective_param(&params)?;

        // 2b. Get current session config
        let config = get_config(&self.state).await;

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

        let raw_data: SnapshotResponse =
            query_and_deserialize(&self.state, "get_snapshot_data", &query_params).await?;

        // 4. Build perspective for spatial calculations
        let persp = build_perspective(&raw_data.perspective);

        // 5. Compute relative positions and filter by radius/visibility
        let mut entities_with_rel: Vec<_> = raw_data
            .entities
            .iter()
            .filter_map(|e| {
                let rel = if e.position.len() == 2 {
                    // 2D entity: use 2D bearing
                    bearing::relative_position_2d(
                        [persp.position[0], persp.position[1]],
                        [persp.forward[0], persp.forward[1]],
                        vec_to_array2(&e.position),
                        !e.visible,
                    )
                } else {
                    // 3D entity: use 3D bearing
                    bearing::relative_position(&persp, vec_to_array3(&e.position), !e.visible)
                };
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
            let scene_dimensions = {
                let s = self.state.lock().await;
                s.scene_dimensions
            };

            let new_index = if scene_dimensions.is_2d() {
                let indexed: Vec<IndexedEntity2D> = raw_data
                    .entities
                    .iter()
                    .map(|e| IndexedEntity2D {
                        path: e.path.clone(),
                        class: e.class.clone(),
                        position: vec_to_array2(&e.position),
                        groups: e.groups.clone(),
                    })
                    .collect();
                SpatialIndex::build_2d(indexed)
            } else {
                // 3D or mixed: use R-tree (2D entities in mixed scenes get Z=0 via vec_to_array3)
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
                SpatialIndex::build(indexed)
            };

            let snapshots: Vec<spectator_core::delta::EntitySnapshot> = raw_data
                .entities
                .iter()
                .map(snapshot::to_entity_snapshot)
                .collect();
            let mut state = self.state.lock().await;
            state.spatial_index = new_index;
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
            let result = serialize_response(&response);
            self.log_activity("query", &activity_summary, "spatial_snapshot").await;
            return result;
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

        let result = serialize_response(&response);
        self.log_activity("query", &activity_summary, "spatial_snapshot").await;
        result
    }

    /// Deep inspection of a single node — transform, physics, state, children,
    /// signals, script, and spatial context. The "tell me everything about this
    /// one thing" tool.
    #[tool(description = "Deep inspection of a single node. Returns transform, physics, state, children, signals, script, and spatial context. Use the 'include' parameter to select specific categories and reduce token usage. Default includes all categories.")]
    pub async fn spatial_inspect(
        &self,
        Parameters(params): Parameters<SpatialInspectParams>,
    ) -> Result<String, McpError> {
        let activity_summary = crate::activity::inspect_summary(&params.node);
        let config = get_config(&self.state).await;

        let include = parse_include(&params.include)?;

        let query_params = GetNodeInspectParams {
            path: params.node.clone(),
            include: include.clone(),
            expose_internals: config.expose_internals,
        };

        let raw_data: NodeInspectResponse =
            query_and_deserialize(&self.state, "get_node_inspect", &query_params).await?;

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

        let budget_limit = resolve_budget(None, 1500, config.token_hard_cap);
        let result = finalize_response(&mut response, budget_limit, config.token_hard_cap);
        self.log_activity("query", &activity_summary, "spatial_inspect").await;
        result
    }

    /// Navigate and query the Godot scene tree structure. Not spatial — this is
    /// about understanding the node hierarchy.
    #[tool(description = "Navigate the Godot scene tree. Actions: 'roots' (top-level nodes), 'children' (immediate children), 'subtree' (recursive tree with depth limit), 'ancestors' (parent chain to root), 'find' (search by name/class/group/script). Use 'include' to control per-node data.")]
    pub async fn scene_tree(
        &self,
        Parameters(params): Parameters<SceneTreeToolParams>,
    ) -> Result<String, McpError> {
        let activity_summary = crate::activity::scene_tree_summary(&params);
        let query_params = build_scene_tree_params(&params)?;

        let data = query_addon(&self.state, "get_scene_tree", serialize_params(&query_params)?)
            .await?;

        let config = get_config(&self.state).await;

        let budget_limit = resolve_budget(params.token_budget, 1500, config.token_hard_cap);
        let mut response = data;
        let result = finalize_response(&mut response, budget_limit, config.token_hard_cap);
        self.log_activity("query", &activity_summary, "scene_tree").await;
        result
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
        let activity_summary = crate::activity::action_summary(&params);
        let config = get_config(&self.state).await;

        let action_request = build_action_request(&params)?;
        let data = query_addon(
            &self.state,
            "execute_action",
            serialize_params(&action_request)?,
        )
        .await?;

        let mut response: serde_json::Value = data;
        let action_budget = resolve_budget(None, 500, config.token_hard_cap);

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
                    && let Ok(raw_data) = serde_json::from_value::<
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
                        let delta_result =
                            s.delta_engine.compute_delta(&current_snapshots, raw_data.frame);
                        let triggers = s.watch_engine.evaluate(
                            s.delta_engine.last_snapshot_map(),
                            &current_snapshots,
                            raw_data.frame,
                        );

                        // Update baseline
                        s.delta_engine
                            .store_snapshot(raw_data.frame, current_snapshots);

                        let delta_json = delta::build_delta_json(&delta_result, &triggers);
                        if let serde_json::Value::Object(ref mut map) = response {
                            map.insert("delta".into(), delta_json);
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

        let result = finalize_response(&mut response, action_budget, config.token_hard_cap);
        self.log_activity("action", &activity_summary, "spatial_action").await;
        result
    }

    /// Targeted spatial questions: nearest nodes, radius search, raycast line-of-sight,
    /// navigation path distance, or mutual relationship between two nodes.
    #[tool(description = "Targeted spatial questions. Query types: 'nearest' (K nearest nodes to a point/node, requires prior spatial_snapshot), 'radius' (all nodes within radius, requires prior spatial_snapshot), 'raycast' (line-of-sight check between two points/nodes), 'path_distance' (navmesh distance), 'relationship' (mutual spatial relationship between two nodes), 'area' (alias for radius).")]
    pub async fn spatial_query(
        &self,
        Parameters(params): Parameters<SpatialQueryParams>,
    ) -> Result<String, McpError> {
        let summary = format!("Query: {}", params.query_type);
        let result = handle_spatial_query(params, &self.state).await;
        self.log_activity("query", &summary, "spatial_query").await;
        result
    }

    /// See what changed since the last query. Returns moved entities, state
    /// changes, new/removed nodes, emitted signals, and watch triggers.
    #[tool(description = "See what changed since the last query. Returns moved entities, state changes, new/removed nodes, and watch triggers. Use after spatial_snapshot or spatial_action to see effects.")]
    pub async fn spatial_delta(
        &self,
        Parameters(params): Parameters<SpatialDeltaParams>,
    ) -> Result<String, McpError> {
        let result = delta::handle_spatial_delta(params, &self.state).await;
        self.log_activity("query", &crate::activity::delta_summary(), "spatial_delta").await;
        result
    }

    /// Subscribe to changes on nodes or groups with optional conditions.
    /// Watch triggers appear in spatial_delta responses.
    #[tool(description = "Subscribe to changes on nodes or groups. Actions: 'add' (subscribe with optional conditions like health < 20), 'remove' (by watch_id), 'list' (show active watches), 'clear' (remove all). Watch triggers appear in spatial_delta responses under 'watch_triggers'.")]
    pub async fn spatial_watch(
        &self,
        Parameters(params): Parameters<SpatialWatchParams>,
    ) -> Result<String, McpError> {
        let summary = crate::activity::watch_summary(&params);
        let result = watch::handle_spatial_watch(params, &self.state).await;
        let active_watches = self.state.lock().await.watch_engine.list().len() as u64;
        self.log_activity_with_meta(
            "watch",
            &summary,
            "spatial_watch",
            Some(serde_json::json!({ "active_watches": active_watches })),
        )
        .await;
        result
    }

    /// Configure tracking behavior — static patterns, state properties,
    /// clustering, bearing format, and token limits. Changes apply for the
    /// current session. Call with no parameters to see current config.
    #[tool(description = "Configure tracking behavior. Set static_patterns (glob patterns for static nodes like [\"walls/*\"]), state_properties (per-group/class property tracking like {\"enemies\": [\"health\"]}), cluster_by (group/class/proximity/none), bearing_format (cardinal/degrees/both), expose_internals (include non-exported vars), poll_interval (collection frequency), token_hard_cap (max tokens per response). Changes apply for the current session.")]
    pub async fn spatial_config(
        &self,
        Parameters(params): Parameters<SpatialConfigParams>,
    ) -> Result<String, McpError> {
        let summary = crate::activity::config_summary(&params);
        let result = handle_spatial_config(params, &self.state).await;
        self.log_activity("config", &summary, "spatial_config").await;
        result
    }

    /// Capture and analyze play session recordings.
    #[tool(description = "Capture and analyze play session recordings. \
        Capture: 'start' (begin recording), 'stop' (end recording), 'status' (check state), \
        'list' (saved recordings), 'delete' (remove by recording_id), 'markers' (list markers), \
        'add_marker' (agent marker). \
        Analysis: 'snapshot_at' (spatial state at frame/time, requires at_frame or at_time_ms), \
        'query_range' (search frame range with condition, requires node + from_frame + to_frame + condition), \
        'diff_frames' (compare two frames, requires frame_a + frame_b), \
        'find_event' (search events by type, requires event_type). \
        Analysis defaults to most recent recording if recording_id is omitted.")]
    pub async fn recording(
        &self,
        Parameters(params): Parameters<recording::RecordingParams>,
    ) -> Result<String, McpError> {
        let summary = crate::activity::recording_summary(&params);
        let result = recording::handle_recording(params, &self.state).await;
        self.log_activity("recording", &summary, "recording").await;
        result
    }
}
