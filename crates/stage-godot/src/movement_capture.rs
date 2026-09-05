//! Main-thread, opt-in movement evidence for the existing spatial recorder.
use std::collections::{BTreeMap, HashMap};

use godot::classes::{CharacterBody3D, Input, InputMap, Node};
use godot::prelude::*;
use stage_protocol::dashcam::DashcamConfig;
use stage_protocol::recording::{MAX_SLIDE_CONTACT_NORMALS, MovementFrameData};

/// Resolve with Godot before any recorder mutation. Store canonical absolute paths,
/// but leave existing scene-relative recording entity paths unchanged.
pub fn validate_targets(owner: &Gd<Node>, config: &mut DashcamConfig) -> Result<(), String> {
    for action in &config.input_actions {
        if !InputMap::singleton().has_action(action.as_str()) {
            return Err(format!("Unknown InputMap action '{action}'"));
        }
    }
    if config.movement_nodes.is_empty() {
        return Ok(());
    }
    let tree = owner
        .get_tree_or_null()
        .ok_or("Movement capture requires a scene tree")?;
    let scene = tree
        .get_current_scene()
        .ok_or("Movement capture requires a current scene")?;
    let mut paths = Vec::new();
    for path in &config.movement_nodes {
        let body = scene
            .try_get_node_as::<CharacterBody3D>(path.as_str())
            .ok_or_else(|| format!("Movement target '{path}' must resolve to a CharacterBody3D"))?;
        let node = body.clone().upcast::<Node>();
        if node != scene && !scene.is_ancestor_of(&node) {
            return Err(format!(
                "Movement target '{path}' is outside the current scene captured by the recorder"
            ));
        }
        let canonical = body.get_path().to_string();
        if paths.contains(&canonical) {
            return Err(format!("Duplicate movement target '{path}'"));
        }
        paths.push(canonical);
    }
    config.movement_nodes = paths;
    Ok(())
}

pub fn capture(owner: &Gd<Node>, config: &DashcamConfig) -> HashMap<String, MovementFrameData> {
    let mut samples = HashMap::new();
    if config.movement_nodes.is_empty() {
        return samples;
    }
    let Some(tree) = owner.get_tree_or_null() else {
        return samples;
    };
    let Some(scene) = tree.get_current_scene() else {
        return samples;
    };
    let input = Input::singleton();
    let actions: BTreeMap<String, f32> = config
        .input_actions
        .iter()
        .filter(|action| InputMap::singleton().has_action(action.as_str()))
        .map(|action| (action.clone(), input.get_action_strength(action.as_str())))
        .collect();
    for path in &config.movement_nodes {
        // A selected node can be freed or replaced after configuration. Absence of
        // movement evidence is preferable to claiming stale contact facts.
        let Some(body) = scene.try_get_node_as::<CharacterBody3D>(path.as_str()) else {
            continue;
        };
        let mut normals = Vec::new();
        let mut truncated = false;
        for index in 0..body.get_slide_collision_count() {
            let Some(collision) = body.get_slide_collision(index) else {
                continue;
            };
            for contact in 0..collision.get_collision_count() {
                if normals.len() == MAX_SLIDE_CONTACT_NORMALS {
                    truncated = true;
                    break;
                }
                normals.push(vector(
                    collision.get_normal_ex().collision_index(contact).done(),
                ));
            }
            if truncated {
                break;
            }
        }
        samples.insert(
            scene
                .get_path_to(&body.clone().upcast::<Node>())
                .to_string(),
            MovementFrameData {
                input_actions: actions.clone(),
                on_floor: body.is_on_floor(),
                on_wall: body.is_on_wall(),
                on_ceiling: body.is_on_ceiling(),
                floor_normal: body.is_on_floor().then(|| vector(body.get_floor_normal())),
                real_velocity: vector(body.get_real_velocity()),
                slide_contact_normals: normals,
                slide_contacts_truncated: truncated,
            },
        );
    }
    samples
}

fn vector(value: Vector3) -> [f64; 3] {
    [value.x as f64, value.y as f64, value.z as f64]
}
