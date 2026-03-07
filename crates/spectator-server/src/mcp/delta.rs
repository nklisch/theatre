use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

use spectator_core::{
    budget::resolve_budget,
    delta::{DeltaResult, EntitySnapshot},
    index::{IndexedEntity, SpatialIndex},
    types::vec_to_array3,
    watch::WatchTrigger,
};
use spectator_protocol::query::{DetailLevel, GetSnapshotDataParams, PerspectiveParam};

use crate::tcp::get_config;

use super::{finalize_response, query_and_deserialize};
use super::snapshot::to_entity_snapshot;

/// MCP parameters for the spatial_delta tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialDeltaParams {
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

/// Build the shared delta JSON object (from_frame, to_frame, and the 5 optional
/// change categories). Used by both spatial_delta and spatial_action return_delta.
pub fn build_delta_json(delta: &DeltaResult, watch_triggers: &[WatchTrigger]) -> serde_json::Value {
    let mut out = serde_json::json!({
        "from_frame": delta.from_frame,
        "to_frame": delta.to_frame,
    });
    if let serde_json::Value::Object(ref mut map) = out {
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
        if !watch_triggers.is_empty() {
            map.insert(
                "watch_triggers".into(),
                serde_json::to_value(watch_triggers).unwrap_or_default(),
            );
        }
    }
    out
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

    // 3. Get config and query addon for current state
    let config = get_config(state).await;

    let query_params = GetSnapshotDataParams {
        perspective: perspective_param,
        radius: params.radius,
        include_offscreen: true, // Delta needs all entities, not just visible
        groups: params.groups.clone().unwrap_or_default(),
        class_filter: params.class_filter.clone().unwrap_or_default(),
        detail: DetailLevel::Standard,
        expose_internals: config.expose_internals,
    };

    let raw_data: spectator_protocol::query::SnapshotResponse =
        query_and_deserialize(state, "get_snapshot_data", &query_params).await?;

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

    // 6. Build response using shared helper, then add delta-only fields
    let mut response = build_delta_json(&delta_result, &watch_triggers);
    if let serde_json::Value::Object(ref mut map) = response {
        map.insert("static_changed".into(), serde_json::json!(false));

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
    let budget_limit = resolve_budget(params.token_budget, 1000, config.token_hard_cap);
    finalize_response(&mut response, budget_limit, config.token_hard_cap)
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
    }

    #[test]
    fn build_delta_json_empty() {
        use spectator_core::delta::DeltaResult;
        let delta = DeltaResult {
            from_frame: 1,
            to_frame: 2,
            moved: vec![],
            state_changed: vec![],
            entered: vec![],
            exited: vec![],
        };
        let json = build_delta_json(&delta, &[]);
        assert_eq!(json["from_frame"], 1);
        assert_eq!(json["to_frame"], 2);
        assert!(json.get("moved").is_none());
        assert!(json.get("watch_triggers").is_none());
    }
}
