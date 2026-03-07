use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;

use spectator_core::{
    budget::estimate_tokens,
    watch::{ConditionOperator, TrackCategory, WatchCondition},
};

use super::{inject_budget, serialize_response};

/// MCP parameters for the spatial_watch tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialWatchParams {
    /// Action: "add", "remove", "list", "clear".
    #[schemars(description = "Action: add, remove, list, clear")]
    pub action: String,

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

    /// What to track: position, state, signals, physics, all.
    #[serde(default = "default_track")]
    pub track: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WatchConditionInput {
    pub property: String,
    /// Operator: "lt", "gt", "eq", "changed".
    pub operator: String,
    pub value: Option<serde_json::Value>,
}

fn default_track() -> Vec<String> {
    vec!["all".to_string()]
}

fn parse_operator(s: &str) -> Result<ConditionOperator, McpError> {
    match s {
        "lt" => Ok(ConditionOperator::Lt),
        "gt" => Ok(ConditionOperator::Gt),
        "eq" => Ok(ConditionOperator::Eq),
        "changed" => Ok(ConditionOperator::Changed),
        other => Err(McpError::invalid_params(
            format!("Unknown operator '{other}'. Valid: lt, gt, eq, changed"),
            None,
        )),
    }
}

fn parse_track(s: &str) -> Result<TrackCategory, McpError> {
    match s {
        "position" => Ok(TrackCategory::Position),
        "state" => Ok(TrackCategory::State),
        "signals" => Ok(TrackCategory::Signals),
        "physics" => Ok(TrackCategory::Physics),
        "all" => Ok(TrackCategory::All),
        other => Err(McpError::invalid_params(
            format!(
                "Unknown track category '{other}'. Valid: position, state, signals, physics, all"
            ),
            None,
        )),
    }
}

pub async fn handle_spatial_watch(
    params: SpatialWatchParams,
    state: &std::sync::Arc<tokio::sync::Mutex<crate::tcp::SessionState>>,
) -> Result<String, McpError> {
    match params.action.as_str() {
        "add" => {
            let spec = params.watch.ok_or_else(|| {
                McpError::invalid_params("'watch' specification is required for add action", None)
            })?;

            let conditions: Vec<WatchCondition> = spec
                .conditions
                .iter()
                .map(|c| {
                    Ok(WatchCondition {
                        property: c.property.clone(),
                        operator: parse_operator(&c.operator)?,
                        value: c.value.clone(),
                    })
                })
                .collect::<Result<Vec<_>, McpError>>()?;

            let track: Vec<TrackCategory> = spec
                .track
                .iter()
                .map(|t| parse_track(t))
                .collect::<Result<Vec<_>, McpError>>()?;

            let watch = {
                let mut s = state.lock().await;
                s.watch_engine.add(spec.node, conditions, track)
            };

            let conditions_desc = if watch.conditions.is_empty() {
                "none".to_string()
            } else {
                watch
                    .conditions
                    .iter()
                    .map(|c| {
                        let val = c
                            .value
                            .as_ref()
                            .map(|v| v.to_string())
                            .unwrap_or_default();
                        format!("{} {:?} {}", c.property, c.operator, val)
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };

            let mut response = serde_json::json!({
                "watch_id": watch.id,
                "watching": watch.node,
                "conditions": conditions_desc,
                "tracking": watch.track,
            });

            let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
            let used = estimate_tokens(json_bytes);
            inject_budget(&mut response, used, 200);

            serialize_response(&response)
        }
        "remove" => {
            let watch_id = params.watch_id.ok_or_else(|| {
                McpError::invalid_params("'watch_id' is required for remove action", None)
            })?;

            let removed = {
                let mut s = state.lock().await;
                s.watch_engine.remove(&watch_id)
            };

            let mut response = serde_json::json!({
                "result": if removed { "ok" } else { "not_found" },
                "removed": if removed { 1 } else { 0 },
            });

            let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
            let used = estimate_tokens(json_bytes);
            inject_budget(&mut response, used, 200);

            serialize_response(&response)
        }
        "list" => {
            let watches = {
                let s = state.lock().await;
                s.watch_engine
                    .list()
                    .iter()
                    .map(|w| {
                        let conditions_desc = if w.conditions.is_empty() {
                            "none".to_string()
                        } else {
                            w.conditions
                                .iter()
                                .map(|c| {
                                    let val = c
                                        .value
                                        .as_ref()
                                        .map(|v| v.to_string())
                                        .unwrap_or_default();
                                    format!("{} {:?} {}", c.property, c.operator, val)
                                })
                                .collect::<Vec<_>>()
                                .join(", ")
                        };
                        serde_json::json!({
                            "id": w.id,
                            "node": w.node,
                            "conditions": conditions_desc,
                            "tracking": w.track,
                        })
                    })
                    .collect::<Vec<_>>()
            };

            let mut response = serde_json::json!({
                "watches": watches,
            });

            let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
            let used = estimate_tokens(json_bytes);
            inject_budget(&mut response, used, 200);

            serialize_response(&response)
        }
        "clear" => {
            let removed = {
                let mut s = state.lock().await;
                s.watch_engine.clear()
            };

            let mut response = serde_json::json!({
                "result": "ok",
                "removed": removed,
            });

            let json_bytes = serde_json::to_vec(&response).unwrap_or_default().len();
            let used = estimate_tokens(json_bytes);
            inject_budget(&mut response, used, 200);

            serialize_response(&response)
        }
        other => Err(McpError::invalid_params(
            format!("Unknown action '{other}'. Valid: add, remove, list, clear"),
            None,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_operator_valid() {
        assert!(matches!(parse_operator("lt"), Ok(ConditionOperator::Lt)));
        assert!(matches!(parse_operator("gt"), Ok(ConditionOperator::Gt)));
        assert!(matches!(parse_operator("eq"), Ok(ConditionOperator::Eq)));
        assert!(matches!(
            parse_operator("changed"),
            Ok(ConditionOperator::Changed)
        ));
    }

    #[test]
    fn parse_operator_invalid() {
        assert!(parse_operator("invalid").is_err());
    }

    #[test]
    fn parse_track_valid() {
        assert!(matches!(parse_track("all"), Ok(TrackCategory::All)));
        assert!(matches!(
            parse_track("position"),
            Ok(TrackCategory::Position)
        ));
    }

    #[test]
    fn parse_track_invalid() {
        assert!(parse_track("everything").is_err());
    }
}
