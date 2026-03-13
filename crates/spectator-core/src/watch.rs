#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::delta::EntitySnapshot;

/// Unique watch identifier.
pub type WatchId = String;

/// A watch subscription.
#[derive(Debug, Clone, Serialize)]
pub struct Watch {
    pub id: WatchId,
    /// Node path or "group:<name>".
    pub node: String,
    /// Conditions that must be met for a trigger.
    pub conditions: Vec<WatchCondition>,
    /// What aspects to track.
    pub track: Vec<TrackCategory>,
}

/// A condition on a watch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchCondition {
    pub property: String,
    pub operator: ConditionOperator,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    Lt,
    Gt,
    Eq,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TrackCategory {
    Position,
    State,
    Signals,
    Physics,
    All,
}

/// A triggered watch result, included in delta responses.
#[derive(Debug, Clone, Serialize)]
pub struct WatchTrigger {
    pub watch_id: WatchId,
    pub node: String,
    pub trigger: String,
    pub frame: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_state: Option<serde_json::Value>,
}

/// The watch engine: manages subscriptions and evaluates conditions.
pub struct WatchEngine {
    watches: Vec<Watch>,
    next_id: u64,
}

impl WatchEngine {
    pub fn new() -> Self {
        Self {
            watches: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a new watch. Returns the watch ID.
    pub fn add(
        &mut self,
        node: String,
        conditions: Vec<WatchCondition>,
        track: Vec<TrackCategory>,
    ) -> Watch {
        let id = format!("w_{:03}", self.next_id);
        self.next_id += 1;

        let watch = Watch {
            id,
            node,
            conditions,
            track,
        };
        self.watches.push(watch.clone());
        watch
    }

    /// Remove a watch by ID. Returns true if found and removed.
    pub fn remove(&mut self, watch_id: &str) -> bool {
        let len_before = self.watches.len();
        self.watches.retain(|w| w.id != watch_id);
        self.watches.len() < len_before
    }

    /// List all active watches.
    pub fn list(&self) -> &[Watch] {
        &self.watches
    }

    /// Remove all watches. Returns the count removed.
    pub fn clear(&mut self) -> usize {
        let count = self.watches.len();
        self.watches.clear();
        count
    }

    /// Evaluate all watches against current entity state.
    /// `prev_state` is the previous entity map (for "changed" operator).
    /// `curr_state` is the current entity map.
    /// Returns all triggered watches.
    pub fn evaluate(
        &self,
        prev_state: &std::collections::HashMap<String, EntitySnapshot>,
        curr_state: &[EntitySnapshot],
        frame: u64,
    ) -> Vec<WatchTrigger> {
        let mut triggers = Vec::new();

        for watch in &self.watches {
            let matching_entities = self.resolve_watch_targets(&watch.node, curr_state);

            for entity in &matching_entities {
                for condition in &watch.conditions {
                    if let Some(trigger_msg) =
                        evaluate_condition(condition, entity, prev_state.get(&entity.path))
                    {
                        triggers.push(WatchTrigger {
                            watch_id: watch.id.clone(),
                            node: entity.path.clone(),
                            trigger: trigger_msg,
                            frame,
                            full_state: Some(serde_json::to_value(entity).unwrap_or_default()),
                        });
                    }
                }
            }
        }

        triggers
    }

    /// Get all node paths that are being watched (for ensuring they appear in deltas).
    /// Expands group watches into the matching entity paths.
    pub fn watched_paths(&self, all_entities: &[EntitySnapshot]) -> Vec<String> {
        let mut paths = Vec::new();
        for watch in &self.watches {
            let targets = self.resolve_watch_targets(&watch.node, all_entities);
            for entity in targets {
                if !paths.contains(&entity.path) {
                    paths.push(entity.path.clone());
                }
            }
        }
        paths
    }

    /// Returns serializable watch list for reconnection re-send.
    pub fn watches_for_reconnect(&self) -> &[Watch] {
        &self.watches
    }

    /// Resolve a watch target ("group:enemies" or "enemies/scout_02") to
    /// matching entities from the current state.
    fn resolve_watch_targets<'a>(
        &self,
        target: &str,
        entities: &'a [EntitySnapshot],
    ) -> Vec<&'a EntitySnapshot> {
        if let Some(group) = target.strip_prefix("group:") {
            entities
                .iter()
                .filter(|e| e.groups.iter().any(|g| g == group))
                .collect()
        } else {
            entities.iter().filter(|e| e.path == target).collect()
        }
    }
}

impl Default for WatchEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluate a single condition against an entity.
/// Returns Some(trigger_message) if condition is met, None otherwise.
fn evaluate_condition(
    condition: &WatchCondition,
    entity: &EntitySnapshot,
    prev: Option<&EntitySnapshot>,
) -> Option<String> {
    let current_value = entity.state.get(&condition.property)?;

    match condition.operator {
        ConditionOperator::Changed => {
            let prev_value = prev.and_then(|p| p.state.get(&condition.property));
            match prev_value {
                Some(old) if !crate::delta::values_equal(old, current_value) => Some(format!(
                    "{} changed from {} to {}",
                    condition.property, old, current_value
                )),
                None => Some(format!(
                    "{} appeared with value {}",
                    condition.property, current_value
                )),
                _ => None,
            }
        }
        ConditionOperator::Lt => {
            let threshold = condition.value.as_ref()?.as_f64()?;
            let current = current_value.as_f64()?;
            if current < threshold {
                Some(format!(
                    "{} dropped to {} (threshold: < {})",
                    condition.property, current, threshold
                ))
            } else {
                None
            }
        }
        ConditionOperator::Gt => {
            let threshold = condition.value.as_ref()?.as_f64()?;
            let current = current_value.as_f64()?;
            if current > threshold {
                Some(format!(
                    "{} rose to {} (threshold: > {})",
                    condition.property, current, threshold
                ))
            } else {
                None
            }
        }
        ConditionOperator::Eq => {
            let target = condition.value.as_ref()?;
            if current_value == target {
                Some(format!("{} equals {}", condition.property, current_value))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_entity(path: &str, health: f64) -> EntitySnapshot {
        EntitySnapshot {
            path: path.to_string(),
            class: "CharacterBody3D".to_string(),
            position: [0.0, 0.0, 0.0],
            rotation_deg: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            groups: vec!["enemies".to_string()],
            state: [("health".to_string(), serde_json::json!(health))]
                .into_iter()
                .collect(),
            visible: true,
        }
    }

    #[test]
    fn add_and_list_watch() {
        let mut engine = WatchEngine::new();
        engine.add(
            "enemies/scout_02".to_string(),
            vec![],
            vec![TrackCategory::All],
        );
        let watches = engine.list();
        assert_eq!(watches.len(), 1);
        assert_eq!(watches[0].id, "w_001");
        assert_eq!(watches[0].node, "enemies/scout_02");
    }

    #[test]
    fn remove_watch() {
        let mut engine = WatchEngine::new();
        engine.add("node_a".to_string(), vec![], vec![TrackCategory::All]);
        engine.add("node_b".to_string(), vec![], vec![TrackCategory::All]);

        let removed = engine.remove("w_001");
        assert!(removed);
        assert_eq!(engine.list().len(), 1);
        assert_eq!(engine.list()[0].id, "w_002");

        let not_found = engine.remove("w_999");
        assert!(!not_found);
    }

    #[test]
    fn clear_all_watches() {
        let mut engine = WatchEngine::new();
        engine.add("a".to_string(), vec![], vec![TrackCategory::All]);
        engine.add("b".to_string(), vec![], vec![TrackCategory::All]);
        engine.add("c".to_string(), vec![], vec![TrackCategory::All]);

        let count = engine.clear();
        assert_eq!(count, 3);
        assert!(engine.list().is_empty());
    }

    #[test]
    fn evaluate_lt_condition_triggers() {
        let mut engine = WatchEngine::new();
        engine.add(
            "enemies/scout".to_string(),
            vec![WatchCondition {
                property: "health".to_string(),
                operator: ConditionOperator::Lt,
                value: Some(serde_json::json!(20.0)),
            }],
            vec![TrackCategory::State],
        );

        let entity = test_entity("enemies/scout", 15.0);
        let triggers = engine.evaluate(&HashMap::new(), &[entity], 10);
        assert_eq!(triggers.len(), 1);
        assert!(triggers[0].trigger.contains("dropped to"));
    }

    #[test]
    fn evaluate_lt_condition_no_trigger() {
        let mut engine = WatchEngine::new();
        engine.add(
            "enemies/scout".to_string(),
            vec![WatchCondition {
                property: "health".to_string(),
                operator: ConditionOperator::Lt,
                value: Some(serde_json::json!(20.0)),
            }],
            vec![TrackCategory::State],
        );

        let entity = test_entity("enemies/scout", 80.0);
        let triggers = engine.evaluate(&HashMap::new(), &[entity], 10);
        assert!(triggers.is_empty());
    }

    #[test]
    fn evaluate_gt_condition() {
        let mut engine = WatchEngine::new();
        engine.add(
            "player".to_string(),
            vec![WatchCondition {
                property: "health".to_string(),
                operator: ConditionOperator::Gt,
                value: Some(serde_json::json!(10.0)),
            }],
            vec![TrackCategory::State],
        );

        let entity = EntitySnapshot {
            path: "player".to_string(),
            class: "CharacterBody3D".to_string(),
            position: [0.0, 0.0, 0.0],
            rotation_deg: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            groups: vec![],
            state: [("health".to_string(), serde_json::json!(12.0))]
                .into_iter()
                .collect(),
            visible: true,
        };

        let triggers = engine.evaluate(&HashMap::new(), &[entity], 10);
        assert_eq!(triggers.len(), 1);
        assert!(triggers[0].trigger.contains("rose to"));
    }

    #[test]
    fn evaluate_eq_condition() {
        let mut engine = WatchEngine::new();
        engine.add(
            "enemy".to_string(),
            vec![WatchCondition {
                property: "state".to_string(),
                operator: ConditionOperator::Eq,
                value: Some(serde_json::json!("alert")),
            }],
            vec![TrackCategory::State],
        );

        let entity = EntitySnapshot {
            path: "enemy".to_string(),
            class: "CharacterBody3D".to_string(),
            position: [0.0, 0.0, 0.0],
            rotation_deg: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            groups: vec![],
            state: [("state".to_string(), serde_json::json!("alert"))]
                .into_iter()
                .collect(),
            visible: true,
        };

        let triggers = engine.evaluate(&HashMap::new(), &[entity], 10);
        assert_eq!(triggers.len(), 1);
        assert!(triggers[0].trigger.contains("equals"));
    }

    #[test]
    fn evaluate_changed_condition() {
        let mut engine = WatchEngine::new();
        engine.add(
            "enemy".to_string(),
            vec![WatchCondition {
                property: "health".to_string(),
                operator: ConditionOperator::Changed,
                value: None,
            }],
            vec![TrackCategory::State],
        );

        let prev = test_entity("enemy", 80.0);
        let curr = test_entity("enemy", 15.0);

        let mut prev_map = HashMap::new();
        prev_map.insert("enemy".to_string(), prev);

        let triggers = engine.evaluate(&prev_map, &[curr], 10);
        assert_eq!(triggers.len(), 1);
        assert!(triggers[0].trigger.contains("changed from"));
    }

    #[test]
    fn group_watch_matches_all_members() {
        let mut engine = WatchEngine::new();
        engine.add(
            "group:enemies".to_string(),
            vec![WatchCondition {
                property: "health".to_string(),
                operator: ConditionOperator::Lt,
                value: Some(serde_json::json!(20.0)),
            }],
            vec![TrackCategory::State],
        );

        let e1 = test_entity("enemies/scout_01", 15.0);
        let e2 = test_entity("enemies/scout_02", 10.0);
        let e3 = test_entity("enemies/scout_03", 80.0);

        let triggers = engine.evaluate(&HashMap::new(), &[e1, e2, e3], 10);
        assert_eq!(triggers.len(), 2); // scout_01 and scout_02 triggered
    }

    #[test]
    fn node_watch_matches_exact_path() {
        let mut engine = WatchEngine::new();
        engine.add(
            "enemies/scout_02".to_string(),
            vec![WatchCondition {
                property: "health".to_string(),
                operator: ConditionOperator::Lt,
                value: Some(serde_json::json!(20.0)),
            }],
            vec![TrackCategory::State],
        );

        let e1 = test_entity("enemies/scout_01", 15.0); // low health but wrong path
        let e2 = test_entity("enemies/scout_02", 10.0); // matches

        let triggers = engine.evaluate(&HashMap::new(), &[e1, e2], 10);
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].node, "enemies/scout_02");
    }

    #[test]
    fn watched_paths_expands_groups() {
        let mut engine = WatchEngine::new();
        engine.add(
            "group:enemies".to_string(),
            vec![],
            vec![TrackCategory::All],
        );

        let entities = vec![
            test_entity("enemies/a", 100.0),
            test_entity("enemies/b", 80.0),
            test_entity("enemies/c", 60.0),
        ];

        let paths = engine.watched_paths(&entities);
        assert_eq!(paths.len(), 3);
    }
}
