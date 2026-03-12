use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

use spectator_core::{
    delta::{DeltaResult, EntitySnapshot},
    watch::WatchTrigger,
};
use spectator_protocol::query::{DetailLevel, GetSnapshotDataParams, PerspectiveParam};

use super::defaults::{default_perspective, default_radius};
use super::snapshot::to_entity_snapshot;
use super::{
    budget_context, finalize_response, insert_if_nonempty, query_and_deserialize,
    update_spatial_state,
};

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

/// Build the shared delta JSON object (from_frame, to_frame, and the 5 optional
/// change categories). Used by both spatial_delta and spatial_action return_delta.
pub fn build_delta_json(
    delta: &DeltaResult,
    watch_triggers: &[WatchTrigger],
) -> Result<serde_json::Value, McpError> {
    let mut out = serde_json::json!({
        "from_frame": delta.from_frame,
        "to_frame": delta.to_frame,
    });
    if let serde_json::Value::Object(ref mut map) = out {
        insert_if_nonempty(map, "moved", &delta.moved)?;
        insert_if_nonempty(map, "state_changed", &delta.state_changed)?;
        insert_if_nonempty(map, "entered", &delta.entered)?;
        insert_if_nonempty(map, "exited", &delta.exited)?;
        insert_if_nonempty(map, "watch_triggers", watch_triggers)?;
    }
    Ok(out)
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
    let bctx = budget_context(state).await;

    let query_params = GetSnapshotDataParams {
        perspective: perspective_param,
        radius: params.radius,
        include_offscreen: true, // Delta needs all entities, not just visible
        groups: params.groups.clone().unwrap_or_default(),
        class_filter: params.class_filter.clone().unwrap_or_default(),
        detail: DetailLevel::Standard,
        expose_internals: bctx.expose_internals,
    };

    let raw_data: spectator_protocol::query::SnapshotResponse =
        query_and_deserialize(state, "get_snapshot_data", &query_params).await?;

    // 4. Convert to entity snapshots
    let current_snapshots: Vec<EntitySnapshot> =
        raw_data.entities.iter().map(to_entity_snapshot).collect();

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

        // Rebuild spatial index and update baseline (respects scene_dimensions for 2D/3D)
        update_spatial_state(&mut s, &raw_data);

        (delta, triggers, events)
    };

    // 6. Build response using shared helper, then add delta-only fields
    let mut response = build_delta_json(&delta_result, &watch_triggers)?;
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
    let budget_limit = bctx.resolve(params.token_budget, 1000);
    finalize_response(&mut response, budget_limit, bctx.hard_cap)
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
        let json = build_delta_json(&delta, &[]).unwrap();
        assert_eq!(json["from_frame"], 1);
        assert_eq!(json["to_frame"], 2);
        assert!(json.get("moved").is_none());
        assert!(json.get("watch_triggers").is_none());
    }
}
