use stage_core::{delta::EntitySnapshot, types::vec_to_array3};
use stage_protocol::query::EntityData;

/// Convert protocol EntityData to a delta-compatible EntitySnapshot.
pub fn to_entity_snapshot(e: &EntityData) -> EntitySnapshot {
    EntitySnapshot {
        path: e.path.clone(),
        class: e.class.clone(),
        position: vec_to_array3(&e.position),
        rotation_deg: vec_to_array3(&e.rotation_deg),
        velocity: vec_to_array3(&e.velocity),
        groups: e.groups.clone(),
        state: e.state.clone(),
        visible: e.visible,
    }
}
