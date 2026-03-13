use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

use spectator_core::watch::{ConditionOperator, TrackCategory, WatchCondition};

use super::budget_context;
use super::finalize_response;

/// Watch action to perform.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WatchAction {
    /// Subscribe to changes on a node or group.
    Add,
    /// Unsubscribe a watch by ID.
    Remove,
    /// List all active watches.
    List,
    /// Remove all watches.
    Clear,
}

/// MCP parameters for the spatial_watch tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialWatchParams {
    /// Watch action: add, remove, list, or clear.
    pub action: WatchAction,

    /// For "add": watch specification.
    pub watch: Option<WatchSpec>,

    /// For "remove": watch ID to remove.
    pub watch_id: Option<String>,
}

/// Watch specification for the "add" action.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WatchSpec {
    /// Node path or "group:<name>".
    pub node: String,

    /// Conditions for triggering.
    #[serde(default)]
    pub conditions: Vec<WatchConditionInput>,

    /// What to track.
    #[serde(default = "default_track")]
    pub track: Vec<TrackCategory>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WatchConditionInput {
    pub property: String,
    /// Comparison operator: lt (less than), gt (greater than), eq (equals), changed (any change).
    pub operator: ConditionOperator,
    pub value: Option<serde_json::Value>,
}

fn default_track() -> Vec<TrackCategory> {
    vec![TrackCategory::All]
}

fn format_conditions(conditions: &[WatchCondition]) -> String {
    if conditions.is_empty() {
        "none".to_string()
    } else {
        conditions
            .iter()
            .map(|c| {
                let val = c.value.as_ref().map(|v| v.to_string()).unwrap_or_default();
                format!("{} {:?} {}", c.property, c.operator, val)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub async fn handle_spatial_watch(
    params: SpatialWatchParams,
    state: &std::sync::Arc<tokio::sync::Mutex<crate::tcp::SessionState>>,
) -> Result<String, McpError> {
    let bctx = budget_context(state).await;
    let hard_cap = bctx.hard_cap;

    match params.action {
        WatchAction::Add => {
            let spec = params.watch.ok_or_else(|| {
                McpError::invalid_params("'watch' specification is required for add action", None)
            })?;

            let conditions: Vec<WatchCondition> = spec
                .conditions
                .into_iter()
                .map(|c| WatchCondition {
                    property: c.property,
                    operator: c.operator,
                    value: c.value,
                })
                .collect();

            let track = spec.track;

            let watch = {
                let mut s = state.lock().await;
                s.watch_engine.add(spec.node, conditions, track)
            };

            let mut response = serde_json::json!({
                "watch_id": watch.id,
                "node": watch.node,
                "conditions": format_conditions(&watch.conditions),
                "track": watch.track,
            });

            finalize_response(&mut response, 200, hard_cap)
        }
        WatchAction::Remove => {
            let watch_id = params.watch_id.ok_or_else(|| {
                McpError::invalid_params("'watch_id' is required for remove action", None)
            })?;

            let removed = {
                let mut s = state.lock().await;
                s.watch_engine.remove(&watch_id)
            };

            let mut response = serde_json::json!({
                "result": if removed { "ok" } else { "not_found" },
                "watch_id": watch_id,
            });

            finalize_response(&mut response, 200, hard_cap)
        }
        WatchAction::List => {
            let watches = {
                let s = state.lock().await;
                s.watch_engine
                    .list()
                    .iter()
                    .map(|w| {
                        serde_json::json!({
                            "watch_id": w.id,
                            "node": w.node,
                            "conditions": format_conditions(&w.conditions),
                            "track": w.track,
                        })
                    })
                    .collect::<Vec<_>>()
            };

            let mut response = serde_json::json!({
                "watches": watches,
            });

            finalize_response(&mut response, 200, hard_cap)
        }
        WatchAction::Clear => {
            let removed = {
                let mut s = state.lock().await;
                s.watch_engine.clear()
            };

            let mut response = serde_json::json!({
                "result": "ok",
                "removed": removed,
            });

            finalize_response(&mut response, 200, hard_cap)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operator_deserialize_valid() {
        let op: ConditionOperator = serde_json::from_str(r#""lt""#).unwrap();
        assert!(matches!(op, ConditionOperator::Lt));
        let op: ConditionOperator = serde_json::from_str(r#""changed""#).unwrap();
        assert!(matches!(op, ConditionOperator::Changed));
    }

    #[test]
    fn operator_deserialize_invalid() {
        assert!(serde_json::from_str::<ConditionOperator>(r#""invalid""#).is_err());
    }

    #[test]
    fn track_deserialize_valid() {
        let t: TrackCategory = serde_json::from_str(r#""all""#).unwrap();
        assert!(matches!(t, TrackCategory::All));
        let t: TrackCategory = serde_json::from_str(r#""position""#).unwrap();
        assert!(matches!(t, TrackCategory::Position));
    }

    #[test]
    fn track_deserialize_invalid() {
        assert!(serde_json::from_str::<TrackCategory>(r#""everything""#).is_err());
    }

    #[test]
    fn format_conditions_empty() {
        assert_eq!(format_conditions(&[]), "none");
    }

    #[test]
    fn format_conditions_single() {
        let conds = vec![WatchCondition {
            property: "health".to_string(),
            operator: ConditionOperator::Lt,
            value: Some(serde_json::json!(20.0)),
        }];
        let s = format_conditions(&conds);
        assert!(s.contains("health"));
        assert!(s.contains("20"));
    }
}
