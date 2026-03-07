use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

use spectator_core::{
    budget::{estimate_tokens, resolve_budget, SnapshotBudgetDefaults},
    delta::EntitySnapshot,
    index::{IndexedEntity, SpatialIndex},
    types::vec_to_array3,
};
use spectator_protocol::query::{DetailLevel, GetSnapshotDataParams, PerspectiveParam};

use crate::tcp::query_addon;

use super::{deserialize_response, inject_budget, serialize_params, serialize_response};
use super::snapshot::to_entity_snapshot;

/// MCP parameters for the spatial_delta tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialDeltaParams {
    /// Frame to diff against. If omitted, diffs against the last query.
    pub since_frame: Option<u64>,

    /// Perspective type: "camera" or "point". Default: "camera".
    #[serde(default = "default_perspective")]
    pub perspective: String,

    /// Max distance from perspective. Default: 50.0.
    #[serde(default = "default_radius")]
    pub radius: f64,

    /// Filter by group membership.
    pub groups: Option<Vec<String>>,

    /// Filter by node class.
    pub class_filter: Option<Vec<String>>,

    /// Soft token budget override.
    pub token_budget: Option<u32>,
}

fn default_perspective() -> String {
    "camera".to_string()
}
fn default_radius() -> f64 {
    50.0
}

pub async fn handle_spatial_delta(
    params: SpatialDeltaParams,
    state: &std::sync::Arc<tokio::sync::Mutex<crate::tcp::SessionState>>,
) -> Result<String, McpError> {
    // 1. Check we have a baseline
    {
        let s = state.lock().await;
        if !s.delta_engine.has_baseline() {
            return Err(McpError::invalid_params(
                "No baseline snapshot available. Call spatial_snapshot first, \
                 then spatial_delta to see what changed.",
                None,
            ));
        }
    }

    // 2. Build perspective param for addon query
    let perspective_param = match params.perspective.as_str() {
        "camera" => PerspectiveParam::Camera,
        "node" => {
            return Err(McpError::invalid_params(
                "Node perspective on delta requires focal_node (not yet supported — use 'camera' or 'point')",
                None,
            ));
        }
        _ => PerspectiveParam::Camera,
    };

    // 3. Query addon for current state
    let query_params = GetSnapshotDataParams {
        perspective: perspective_param,
        radius: params.radius,
        include_offscreen: true, // Delta needs all entities, not just visible
        groups: params.groups.clone().unwrap_or_default(),
        class_filter: params.class_filter.clone().unwrap_or_default(),
        detail: DetailLevel::Standard,
    };

    let raw_data: spectator_protocol::query::SnapshotResponse = {
        let data =
            query_addon(state, "get_snapshot_data", serialize_params(&query_params)?).await?;
        deserialize_response(data)?
    };

    // 4. Convert to entity snapshots
    let current_snapshots: Vec<EntitySnapshot> = raw_data
        .entities
        .iter()
        .map(to_entity_snapshot)
        .collect();

    // 5. Compute delta, evaluate watches, drain events, update baseline
    let (delta_result, watch_triggers, signals_emitted) = {
        let mut s = state.lock().await;

        // Compute delta against stored baseline
        let delta = s
            .delta_engine
            .compute_delta(&current_snapshots, raw_data.frame);

        // Evaluate watches (immutable borrows of two different fields)
        let triggers = s.watch_engine.evaluate(
            s.delta_engine.last_snapshot_map(),
            &current_snapshots,
            raw_data.frame,
        );

        // Drain buffered signal events
        let events = s.delta_engine.drain_events();

        // Update baseline with new state
        s.delta_engine
            .store_snapshot(raw_data.frame, current_snapshots.clone());

        // Rebuild spatial index
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
        s.spatial_index = SpatialIndex::build(indexed);

        (delta, triggers, events)
    };

    // 6. Build response — omit empty categories
    let mut response = serde_json::json!({
        "from_frame": delta_result.from_frame,
        "to_frame": delta_result.to_frame,
        "static_changed": false,
    });

    if let serde_json::Value::Object(ref mut map) = response {
        if !delta_result.moved.is_empty() {
            map.insert(
                "moved".into(),
                serde_json::to_value(&delta_result.moved).unwrap_or_default(),
            );
        }
        if !delta_result.state_changed.is_empty() {
            map.insert(
                "state_changed".into(),
                serde_json::to_value(&delta_result.state_changed).unwrap_or_default(),
            );
        }
        if !delta_result.entered.is_empty() {
            map.insert(
                "entered".into(),
                serde_json::to_value(&delta_result.entered).unwrap_or_default(),
            );
        }
        if !delta_result.exited.is_empty() {
            map.insert(
                "exited".into(),
                serde_json::to_value(&delta_result.exited).unwrap_or_default(),
            );
        }
        if !watch_triggers.is_empty() {
            map.insert(
                "watch_triggers".into(),
                serde_json::to_value(&watch_triggers).unwrap_or_default(),
            );
        }

        // Include buffered signal events
        let signal_entries: Vec<serde_json::Value> = signals_emitted
            .iter()
            .filter(|e| {
                matches!(
                    e.event_type,
                    spectator_core::delta::BufferedEventType::SignalEmitted
                )
            })
            .map(|e| {
                serde_json::json!({
                    "path": e.path,
                    "signal": e.data.get("signal").unwrap_or(&serde_json::json!("unknown")),
                    "args": e.data.get("args").unwrap_or(&serde_json::json!([])),
                    "frame": e.frame,
                })
            })
            .collect();
        if !signal_entries.is_empty() {
            map.insert("signals_emitted".into(), serde_json::json!(signal_entries));
        }
    }

    // 7. Budget
    let budget_limit = resolve_budget(params.token_budget, 1000, SnapshotBudgetDefaults::HARD_CAP);
    let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
    let used = estimate_tokens(json_bytes);
    inject_budget(&mut response, used, budget_limit);

    serialize_response(&response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_params_defaults() {
        let json = r#"{ }"#;
        let params: SpatialDeltaParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.perspective, "camera");
        assert_eq!(params.radius, 50.0);
        assert!(params.since_frame.is_none());
    }
}
