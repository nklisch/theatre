use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::Position3;

// -----------------------------------------------------------------------
// Change detection thresholds (from SPEC.md)
// -----------------------------------------------------------------------

/// Position change below this is suppressed (world units).
pub const POSITION_THRESHOLD: f64 = 0.01;
/// Rotation change below this is suppressed (degrees).
pub const ROTATION_THRESHOLD: f64 = 0.1;
/// Float property change below this is suppressed.
pub const FLOAT_THRESHOLD: f64 = 0.001;

// -----------------------------------------------------------------------
// Stored entity state
// -----------------------------------------------------------------------

/// Minimal snapshot of one entity's state, stored between queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub path: String,
    pub class: String,
    pub position: Position3,
    pub rotation_deg: [f64; 3],
    pub velocity: [f64; 3],
    pub groups: Vec<String>,
    /// Exported variable state.
    pub state: serde_json::Map<String, serde_json::Value>,
    pub visible: bool,
}

/// Event buffered between delta queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferedEvent {
    pub event_type: BufferedEventType,
    pub path: String,
    pub frame: u64,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BufferedEventType {
    SignalEmitted,
    NodeEntered,
    NodeExited,
}

// -----------------------------------------------------------------------
// Delta engine
// -----------------------------------------------------------------------

/// The delta engine: stores last-query state, computes diffs.
pub struct DeltaEngine {
    /// Entity state at the time of the last snapshot/delta.
    last_snapshot: HashMap<String, EntitySnapshot>,
    /// Frame number of the last stored snapshot.
    last_frame: u64,
    /// Buffered events received between queries (from push events).
    event_buffer: Vec<BufferedEvent>,
}

impl DeltaEngine {
    pub fn new() -> Self {
        Self {
            last_snapshot: HashMap::new(),
            last_frame: 0,
            event_buffer: Vec::new(),
        }
    }

    /// Store a new snapshot. Called after every `spatial_snapshot` or
    /// `spatial_delta` query completes.
    pub fn store_snapshot(&mut self, frame: u64, entities: Vec<EntitySnapshot>) {
        self.last_snapshot.clear();
        for entity in entities {
            self.last_snapshot.insert(entity.path.clone(), entity);
        }
        self.last_frame = frame;
        self.event_buffer.clear();
    }

    /// Push an event into the buffer (from addon push events).
    pub fn push_event(&mut self, event: BufferedEvent) {
        self.event_buffer.push(event);
    }

    /// Returns the frame of the last stored snapshot.
    pub fn last_frame(&self) -> u64 {
        self.last_frame
    }

    /// Returns true if we have a stored snapshot to diff against.
    pub fn has_baseline(&self) -> bool {
        self.last_frame > 0
    }

    /// Returns a reference to the stored snapshot map.
    /// Used by the watch engine to check "changed" conditions.
    pub fn last_snapshot_map(&self) -> &HashMap<String, EntitySnapshot> {
        &self.last_snapshot
    }

    /// Drain the buffered events (consumed by delta computation).
    pub fn drain_events(&mut self) -> Vec<BufferedEvent> {
        std::mem::take(&mut self.event_buffer)
    }

    /// Compute changes between stored snapshot and a new set of entities.
    /// Returns categorized changes.
    pub fn compute_delta(
        &self,
        current_entities: &[EntitySnapshot],
        current_frame: u64,
    ) -> DeltaResult {
        let mut moved = Vec::new();
        let mut state_changed = Vec::new();
        let mut entered = Vec::new();
        let mut exited = Vec::new();

        // Build a set of current paths for exit detection
        let current_paths: std::collections::HashSet<&str> =
            current_entities.iter().map(|e| e.path.as_str()).collect();

        // Check each current entity against stored state
        for entity in current_entities {
            match self.last_snapshot.get(&entity.path) {
                Some(prev) => {
                    // Check movement
                    if let Some(movement) = detect_movement(prev, entity) {
                        moved.push(movement);
                    }
                    // Check state changes
                    if let Some(changes) = detect_state_changes(prev, entity) {
                        state_changed.push(changes);
                    }
                }
                None => {
                    // New entity — entered
                    entered.push(EnteredEntity {
                        path: entity.path.clone(),
                        class: entity.class.clone(),
                        position: entity.position,
                    });
                }
            }
        }

        // Check for exited entities (were in last snapshot, not in current)
        for path in self.last_snapshot.keys() {
            if !current_paths.contains(path.as_str()) {
                exited.push(ExitedEntity {
                    path: path.clone(),
                    reason: "removed".to_string(),
                });
            }
        }

        DeltaResult {
            from_frame: self.last_frame,
            to_frame: current_frame,
            moved,
            state_changed,
            entered,
            exited,
        }
    }
}

impl Default for DeltaEngine {
    fn default() -> Self {
        Self::new()
    }
}

// -----------------------------------------------------------------------
// Delta result types
// -----------------------------------------------------------------------

/// Result of a delta computation.
#[derive(Debug, Clone, Serialize)]
pub struct DeltaResult {
    pub from_frame: u64,
    pub to_frame: u64,
    pub moved: Vec<MovedEntity>,
    pub state_changed: Vec<StateChange>,
    pub entered: Vec<EnteredEntity>,
    pub exited: Vec<ExitedEntity>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MovedEntity {
    pub path: String,
    /// New position.
    pub pos: Position3,
    /// Position delta (new - old).
    pub delta_pos: Position3,
    /// Distance from focal point (filled in by server, not by engine).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dist_to_focal: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StateChange {
    pub path: String,
    /// Map of property_name → [old_value, new_value].
    pub changes: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnteredEntity {
    pub path: String,
    pub class: String,
    pub position: Position3,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExitedEntity {
    pub path: String,
    pub reason: String,
}

// -----------------------------------------------------------------------
// Change detection helpers
// -----------------------------------------------------------------------

/// Detect movement beyond threshold.
fn detect_movement(prev: &EntitySnapshot, curr: &EntitySnapshot) -> Option<MovedEntity> {
    let dx = curr.position[0] - prev.position[0];
    let dy = curr.position[1] - prev.position[1];
    let dz = curr.position[2] - prev.position[2];
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();

    if dist < POSITION_THRESHOLD {
        return None;
    }

    Some(MovedEntity {
        path: curr.path.clone(),
        pos: curr.position,
        delta_pos: [dx, dy, dz],
        dist_to_focal: None, // Filled in by the server layer
    })
}

/// Detect state property changes beyond thresholds.
fn detect_state_changes(prev: &EntitySnapshot, curr: &EntitySnapshot) -> Option<StateChange> {
    let mut changes = serde_json::Map::new();

    // Check properties in current state
    for (key, new_val) in &curr.state {
        match prev.state.get(key) {
            Some(old_val) => {
                if !values_equal(old_val, new_val) {
                    changes.insert(
                        key.clone(),
                        serde_json::json!([old_val, new_val]),
                    );
                }
            }
            None => {
                // New property appeared
                changes.insert(
                    key.clone(),
                    serde_json::json!([null, new_val]),
                );
            }
        }
    }

    // Check for removed properties
    for (key, old_val) in &prev.state {
        if !curr.state.contains_key(key) {
            changes.insert(
                key.clone(),
                serde_json::json!([old_val, null]),
            );
        }
    }

    if changes.is_empty() {
        None
    } else {
        Some(StateChange {
            path: curr.path.clone(),
            changes,
        })
    }
}

/// Compare two JSON values with float thresholds.
pub(crate) fn values_equal(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    match (a, b) {
        (serde_json::Value::Number(a), serde_json::Value::Number(b)) => {
            match (a.as_f64(), b.as_f64()) {
                (Some(fa), Some(fb)) => (fa - fb).abs() < FLOAT_THRESHOLD,
                _ => a == b,
            }
        }
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(path: &str, pos: Position3, state: &[(&str, serde_json::Value)]) -> EntitySnapshot {
        EntitySnapshot {
            path: path.to_string(),
            class: "CharacterBody3D".to_string(),
            position: pos,
            rotation_deg: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            groups: vec!["enemies".to_string()],
            state: state.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
            visible: true,
        }
    }

    #[test]
    fn detect_movement_above_threshold() {
        let prev = entity("enemy", [0.0, 0.0, 0.0], &[]);
        let curr = entity("enemy", [1.0, 0.0, 0.0], &[]);

        let mut engine = DeltaEngine::new();
        engine.store_snapshot(1, vec![prev]);

        let delta = engine.compute_delta(&[curr], 2);
        assert_eq!(delta.moved.len(), 1);
        assert_eq!(delta.moved[0].path, "enemy");
        assert!((delta.moved[0].delta_pos[0] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn suppress_movement_below_threshold() {
        let prev = entity("enemy", [0.0, 0.0, 0.0], &[]);
        let curr = entity("enemy", [0.005, 0.0, 0.0], &[]);

        let mut engine = DeltaEngine::new();
        engine.store_snapshot(1, vec![prev]);

        let delta = engine.compute_delta(&[curr], 2);
        assert!(delta.moved.is_empty());
    }

    #[test]
    fn detect_state_change() {
        let prev = entity("enemy", [0.0, 0.0, 0.0], &[("health", serde_json::json!(100.0))]);
        let curr = entity("enemy", [0.0, 0.0, 0.0], &[("health", serde_json::json!(80.0))]);

        let mut engine = DeltaEngine::new();
        engine.store_snapshot(1, vec![prev]);

        let delta = engine.compute_delta(&[curr], 2);
        assert_eq!(delta.state_changed.len(), 1);
        assert_eq!(delta.state_changed[0].path, "enemy");
        assert!(delta.state_changed[0].changes.contains_key("health"));
    }

    #[test]
    fn suppress_float_change_below_threshold() {
        let prev = entity("enemy", [0.0, 0.0, 0.0], &[("speed", serde_json::json!(1.0))]);
        let curr = entity("enemy", [0.0, 0.0, 0.0], &[("speed", serde_json::json!(1.0005))]);

        let mut engine = DeltaEngine::new();
        engine.store_snapshot(1, vec![prev]);

        let delta = engine.compute_delta(&[curr], 2);
        assert!(delta.state_changed.is_empty());
    }

    #[test]
    fn detect_entered_entity() {
        let existing = entity("enemy_a", [0.0, 0.0, 0.0], &[]);
        let new_entity = entity("enemy_b", [5.0, 0.0, 0.0], &[]);

        let mut engine = DeltaEngine::new();
        engine.store_snapshot(1, vec![existing.clone()]);

        let delta = engine.compute_delta(&[existing, new_entity], 2);
        assert_eq!(delta.entered.len(), 1);
        assert_eq!(delta.entered[0].path, "enemy_b");
    }

    #[test]
    fn detect_exited_entity() {
        let a = entity("enemy_a", [0.0, 0.0, 0.0], &[]);
        let b = entity("enemy_b", [5.0, 0.0, 0.0], &[]);

        let mut engine = DeltaEngine::new();
        engine.store_snapshot(1, vec![a.clone(), b]);

        // Only a remains
        let delta = engine.compute_delta(&[a], 2);
        assert_eq!(delta.exited.len(), 1);
        assert_eq!(delta.exited[0].path, "enemy_b");
    }

    #[test]
    fn store_and_compute_full_cycle() {
        let e1 = entity("a", [0.0, 0.0, 0.0], &[("hp", serde_json::json!(100.0))]);
        let e2 = entity("b", [10.0, 0.0, 0.0], &[]);

        let mut engine = DeltaEngine::new();
        assert!(!engine.has_baseline());

        engine.store_snapshot(1, vec![e1, e2]);
        assert!(engine.has_baseline());
        assert_eq!(engine.last_frame(), 1);

        let curr_a = entity("a", [2.0, 0.0, 0.0], &[("hp", serde_json::json!(50.0))]);
        let curr_c = entity("c", [20.0, 0.0, 0.0], &[]);

        let delta = engine.compute_delta(&[curr_a, curr_c], 5);
        assert_eq!(delta.from_frame, 1);
        assert_eq!(delta.to_frame, 5);
        assert_eq!(delta.moved.len(), 1); // a moved
        assert_eq!(delta.state_changed.len(), 1); // a hp changed
        assert_eq!(delta.entered.len(), 1); // c entered
        assert_eq!(delta.exited.len(), 1); // b exited
    }

    #[test]
    fn event_buffer_drain() {
        let mut engine = DeltaEngine::new();
        engine.push_event(BufferedEvent {
            event_type: BufferedEventType::SignalEmitted,
            path: "player".to_string(),
            frame: 1,
            data: serde_json::json!({"signal": "hit"}),
        });
        engine.push_event(BufferedEvent {
            event_type: BufferedEventType::SignalEmitted,
            path: "enemy".to_string(),
            frame: 2,
            data: serde_json::json!({"signal": "died"}),
        });

        let events = engine.drain_events();
        assert_eq!(events.len(), 2);

        let empty = engine.drain_events();
        assert!(empty.is_empty());
    }

    #[test]
    fn values_equal_float_threshold() {
        // Within threshold — equal
        assert!(values_equal(&serde_json::json!(1.0), &serde_json::json!(1.0005)));
        // Outside threshold — not equal
        assert!(!values_equal(&serde_json::json!(1.0), &serde_json::json!(1.002)));
    }

    #[test]
    fn values_equal_string() {
        assert!(!values_equal(
            &serde_json::json!("patrol"),
            &serde_json::json!("alert")
        ));
        assert!(values_equal(
            &serde_json::json!("patrol"),
            &serde_json::json!("patrol")
        ));
    }
}
