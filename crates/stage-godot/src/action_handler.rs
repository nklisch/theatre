use godot::builtin::{Array, GString, StringName, VarDictionary, Variant, Vector2, Vector3};
use godot::classes::{
    Input, InputEvent, InputEventKey, InputEventMouseButton, InputMap, Node, Node2D, Node3D,
    PackedScene, ResourceLoader, SceneTree,
};
use godot::global::{Key, MouseButton};
use godot::obj::Gd;
use godot::prelude::*;

use stage_protocol::query::{ActionRequest, ActionResponse};

use crate::collector::StageCollector;

/// Result of an action execution.
/// `Ok(Some(resp))` = action completed synchronously.
/// `Ok(None)` = response will be deferred (advance_frames).
/// `Err(msg)` = action failed.
pub enum ActionResult {
    Done(ActionResponse),
    Pending,
}

/// Execute an action against the Godot scene tree.
/// Called from the query handler on the main thread.
pub fn execute_action(
    request: &ActionRequest,
    collector: &StageCollector,
    request_id: &str,
) -> Result<ActionResult, String> {
    match request {
        ActionRequest::Pause { paused } => {
            execute_pause(*paused, collector).map(ActionResult::Done)
        }
        ActionRequest::AdvanceFrames { frames } => {
            execute_advance_frames(*frames, collector, request_id)
        }
        ActionRequest::AdvanceTime { seconds } => {
            execute_advance_time(*seconds, collector, request_id)
        }
        ActionRequest::Teleport {
            path,
            position,
            rotation_deg,
        } => execute_teleport(path, position, *rotation_deg, collector).map(ActionResult::Done),
        ActionRequest::SetProperty {
            path,
            property,
            value,
        } => execute_set_property(path, property, value, collector).map(ActionResult::Done),
        ActionRequest::CallMethod { path, method, args } => {
            execute_call_method(path, method, args, collector).map(ActionResult::Done)
        }
        ActionRequest::EmitSignal { path, signal, args } => {
            execute_emit_signal(path, signal, args, collector).map(ActionResult::Done)
        }
        ActionRequest::SpawnNode {
            scene_path,
            parent,
            name,
            position,
        } => execute_spawn_node(
            scene_path,
            parent,
            name.as_deref(),
            position.as_deref(),
            collector,
        )
        .map(ActionResult::Done),
        ActionRequest::RemoveNode { path } => {
            execute_remove_node(path, collector).map(ActionResult::Done)
        }
        ActionRequest::ActionPress {
            action_name,
            strength,
        } => execute_action_press(action_name, *strength, collector).map(ActionResult::Done),
        ActionRequest::ActionRelease { action_name } => {
            execute_action_release(action_name, collector).map(ActionResult::Done)
        }
        ActionRequest::InjectKey {
            keycode,
            pressed,
            echo,
        } => execute_inject_key(keycode, *pressed, *echo, collector).map(ActionResult::Done),
        ActionRequest::InjectMouseButton {
            button,
            pressed,
            position,
        } => execute_inject_mouse_button(button, *pressed, position.as_deref(), collector)
            .map(ActionResult::Done),
    }
}

fn get_frame(collector: &StageCollector) -> u64 {
    collector.get_frame_info().frame
}

fn get_scene_tree(collector: &StageCollector) -> Result<Gd<SceneTree>, String> {
    collector
        .base()
        .get_tree()
        .ok_or_else(|| "Not in scene tree".to_string())
}

fn resolve_node(collector: &StageCollector, path: &str) -> Result<Gd<Node>, String> {
    collector.resolve_node_public(path)
}

fn execute_pause(paused: bool, collector: &StageCollector) -> Result<ActionResponse, String> {
    let mut tree = get_scene_tree(collector)?;
    tree.set_pause(paused);
    Ok(ActionResponse {
        action: "pause".into(),
        result: "ok".into(),
        details: serde_json::Map::from_iter([("paused".into(), serde_json::Value::Bool(paused))]),
        frame: get_frame(collector),
    })
}

fn execute_advance_frames(
    frames: u32,
    collector: &StageCollector,
    request_id: &str,
) -> Result<ActionResult, String> {
    let mut tree = get_scene_tree(collector)?;
    if !tree.is_paused() {
        return Err(
            "not_paused: Cannot advance frames while unpaused. Use pause action first.".into(),
        );
    }
    // Store advance state — tcp_server.poll() will drive it
    collector.set_advance_state(frames, Some(request_id.to_string()));
    // Unpause to allow physics ticks
    tree.set_pause(false);
    Ok(ActionResult::Pending)
}

fn execute_advance_time(
    seconds: f64,
    collector: &StageCollector,
    request_id: &str,
) -> Result<ActionResult, String> {
    let tree = get_scene_tree(collector)?;
    if !tree.is_paused() {
        return Err(
            "not_paused: Cannot advance time while unpaused. Use pause action first.".into(),
        );
    }
    let tps = godot::classes::Engine::singleton().get_physics_ticks_per_second() as f64;
    let frames = (seconds * tps).round() as u32;
    // Delegate to advance_frames logic
    execute_advance_frames(frames, collector, request_id)
}

fn execute_teleport(
    path: &str,
    position: &[f64],
    rotation_deg: Option<f64>,
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let node = resolve_node(collector, path)?;
    let mut details = serde_json::Map::new();

    if let Ok(mut node3d) = node.clone().try_cast::<Node3D>() {
        let prev = node3d.get_global_position();
        details.insert(
            "previous_position".into(),
            serde_json::json!([prev.x, prev.y, prev.z]),
        );

        let new_pos = match position.len() {
            3 => Vector3::new(position[0] as f32, position[1] as f32, position[2] as f32),
            2 => Vector3::new(position[0] as f32, 0.0, position[1] as f32),
            _ => {
                return Err(format!(
                    "Invalid position: expected 2 or 3 components, got {}",
                    position.len()
                ));
            }
        };
        node3d.set_global_position(new_pos);
        details.insert(
            "new_position".into(),
            serde_json::json!([new_pos.x, new_pos.y, new_pos.z]),
        );

        if let Some(rot) = rotation_deg {
            let mut euler = node3d.get_rotation_degrees();
            euler.y = rot as f32;
            node3d.set_rotation_degrees(euler);
            details.insert("rotation_deg".into(), serde_json::json!(rot));
        }
    } else if let Ok(mut node2d) = node.try_cast::<Node2D>() {
        let prev = node2d.get_global_position();
        details.insert(
            "previous_position".into(),
            serde_json::json!([prev.x, prev.y]),
        );

        let new_pos = match position.len() {
            2 => Vector2::new(position[0] as f32, position[1] as f32),
            _ => {
                return Err(format!(
                    "Invalid 2D position: expected 2 components, got {}",
                    position.len()
                ));
            }
        };
        node2d.set_global_position(new_pos);
        details.insert(
            "new_position".into(),
            serde_json::json!([new_pos.x, new_pos.y]),
        );

        if let Some(rot) = rotation_deg {
            node2d.set_rotation_degrees(rot as f32);
            details.insert("rotation_deg".into(), serde_json::json!(rot));
        }
    } else {
        return Err(format!(
            "Node '{path}' is not a Node3D or Node2D — cannot teleport"
        ));
    }

    Ok(ActionResponse {
        action: "teleport".into(),
        result: "ok".into(),
        details,
        frame: get_frame(collector),
    })
}

fn execute_set_property(
    path: &str,
    property: &str,
    value: &serde_json::Value,
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let mut node = resolve_node(collector, path)?;

    // Read previous value
    let prev_variant = node.get(property);
    let prev_json = crate::collector::variant_to_json(&prev_variant);

    // Convert JSON value to Variant and set
    let new_variant = json_to_variant(value)?;
    node.set(property, &new_variant);

    let mut details = serde_json::Map::new();
    details.insert("property".into(), serde_json::json!(property));
    if let Some(prev) = prev_json {
        details.insert("previous_value".into(), prev);
    }
    details.insert("new_value".into(), value.clone());

    Ok(ActionResponse {
        action: "set_property".into(),
        result: "ok".into(),
        details,
        frame: get_frame(collector),
    })
}

fn execute_call_method(
    path: &str,
    method: &str,
    args: &[serde_json::Value],
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let mut node = resolve_node(collector, path)?;

    if !node.has_method(method) {
        return Err(format!("Method '{method}' not found on node '{path}'"));
    }

    let variant_args: Vec<Variant> = args.iter().map(json_to_variant).collect::<Result<_, _>>()?;

    let arr: Array<Variant> = variant_args.into_iter().collect();
    let result = node.callv(method, &arr);
    let result_json = crate::collector::variant_to_json(&result);

    let mut details = serde_json::Map::new();
    details.insert("method".into(), serde_json::json!(method));
    details.insert(
        "return_value".into(),
        result_json.unwrap_or(serde_json::Value::Null),
    );

    Ok(ActionResponse {
        action: "call_method".into(),
        result: "ok".into(),
        details,
        frame: get_frame(collector),
    })
}

fn execute_emit_signal(
    path: &str,
    signal: &str,
    args: &[serde_json::Value],
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let mut node = resolve_node(collector, path)?;

    let variant_args: Vec<Variant> = args.iter().map(json_to_variant).collect::<Result<_, _>>()?;

    node.emit_signal(signal, &variant_args);

    Ok(ActionResponse {
        action: "emit_signal".into(),
        result: "ok".into(),
        details: serde_json::Map::from_iter([("signal".into(), serde_json::json!(signal))]),
        frame: get_frame(collector),
    })
}

fn execute_spawn_node(
    scene_path: &str,
    parent_path: &str,
    name: Option<&str>,
    position: Option<&[f64]>,
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let mut parent = resolve_node(collector, parent_path)?;

    let mut loader = ResourceLoader::singleton();
    let resource = loader
        .load(scene_path)
        .ok_or_else(|| format!("Could not load scene: {scene_path}"))?;
    let packed = resource
        .try_cast::<PackedScene>()
        .map_err(|_| format!("Resource '{scene_path}' is not a PackedScene"))?;

    let mut instance = packed
        .instantiate()
        .ok_or_else(|| format!("Failed to instantiate scene: {scene_path}"))?;

    if let Some(n) = name {
        instance.set_name(n);
    }

    // Set position if provided (must be done after add_child for global coords)
    let node_name = instance.get_name().to_string();
    parent.add_child(&instance);

    if let Some(pos) = position {
        if let Ok(mut n3d) = instance.clone().try_cast::<Node3D>() {
            if pos.len() >= 3 {
                n3d.set_global_position(Vector3::new(pos[0] as f32, pos[1] as f32, pos[2] as f32));
            }
        } else if let Ok(mut n2d) = instance.try_cast::<Node2D>()
            && pos.len() >= 2
        {
            n2d.set_global_position(Vector2::new(pos[0] as f32, pos[1] as f32));
        }
    }

    let node_path = format!("{parent_path}/{node_name}");

    Ok(ActionResponse {
        action: "spawn_node".into(),
        result: "ok".into(),
        details: serde_json::Map::from_iter([
            ("scene_path".into(), serde_json::json!(scene_path)),
            ("node_path".into(), serde_json::json!(node_path)),
        ]),
        frame: get_frame(collector),
    })
}

fn execute_remove_node(path: &str, collector: &StageCollector) -> Result<ActionResponse, String> {
    let mut node = resolve_node(collector, path)?;
    let class = node.get_class().to_string();
    node.queue_free();

    Ok(ActionResponse {
        action: "remove_node".into(),
        result: "ok".into(),
        details: serde_json::Map::from_iter([
            ("removed_path".into(), serde_json::json!(path)),
            ("removed_class".into(), serde_json::json!(class)),
        ]),
        frame: get_frame(collector),
    })
}

fn execute_action_press(
    action_name: &str,
    strength: f32,
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let mut input = Input::singleton();
    let sn = StringName::from(action_name);

    // Validate the action exists in the InputMap
    if !InputMap::singleton().has_action(&sn) {
        return Err(format!("Unknown InputMap action: '{action_name}'"));
    }

    input.action_press_ex(&sn).strength(strength).done();

    let mut details = serde_json::Map::new();
    details.insert("action_name".into(), serde_json::json!(action_name));
    details.insert("strength".into(), serde_json::json!(strength));

    Ok(ActionResponse {
        action: "action_press".into(),
        result: "ok".into(),
        details,
        frame: get_frame(collector),
    })
}

fn execute_action_release(
    action_name: &str,
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let mut input = Input::singleton();
    let sn = StringName::from(action_name);

    if !InputMap::singleton().has_action(&sn) {
        return Err(format!("Unknown InputMap action: '{action_name}'"));
    }

    input.action_release(&sn);

    let mut details = serde_json::Map::new();
    details.insert("action_name".into(), serde_json::json!(action_name));

    Ok(ActionResponse {
        action: "action_release".into(),
        result: "ok".into(),
        details,
        frame: get_frame(collector),
    })
}

fn execute_inject_key(
    keycode: &str,
    pressed: bool,
    echo: bool,
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let key = parse_key(keycode)?;

    let mut event = InputEventKey::new_gd();
    event.set_keycode(key);
    event.set_pressed(pressed);
    event.set_echo(echo);

    let input_event: Gd<InputEvent> = event.upcast();
    Input::singleton().parse_input_event(&input_event);

    let mut details = serde_json::Map::new();
    details.insert("keycode".into(), serde_json::json!(keycode));
    details.insert("pressed".into(), serde_json::json!(pressed));
    if echo {
        details.insert("echo".into(), serde_json::json!(true));
    }

    Ok(ActionResponse {
        action: "inject_key".into(),
        result: "ok".into(),
        details,
        frame: get_frame(collector),
    })
}

fn execute_inject_mouse_button(
    button: &str,
    pressed: bool,
    position: Option<&[f64]>,
    collector: &StageCollector,
) -> Result<ActionResponse, String> {
    let button_index = parse_mouse_button(button)?;

    let mut event = InputEventMouseButton::new_gd();
    event.set_button_index(button_index);
    event.set_pressed(pressed);

    if let Some(pos) = position {
        if pos.len() >= 2 {
            event.set_position(Vector2::new(pos[0] as f32, pos[1] as f32));
        }
    }

    let input_event: Gd<InputEvent> = event.upcast();
    Input::singleton().parse_input_event(&input_event);

    let mut details = serde_json::Map::new();
    details.insert("button".into(), serde_json::json!(button));
    details.insert("pressed".into(), serde_json::json!(pressed));
    if let Some(pos) = position {
        details.insert("position".into(), serde_json::json!(pos));
    }

    Ok(ActionResponse {
        action: "inject_mouse_button".into(),
        result: "ok".into(),
        details,
        frame: get_frame(collector),
    })
}

fn parse_key(name: &str) -> Result<Key, String> {
    let normalized = name.to_uppercase();
    let name = normalized.strip_prefix("KEY_").unwrap_or(&normalized);
    match name {
        "A" => Ok(Key::A),
        "B" => Ok(Key::B),
        "C" => Ok(Key::C),
        "D" => Ok(Key::D),
        "E" => Ok(Key::E),
        "F" => Ok(Key::F),
        "G" => Ok(Key::G),
        "H" => Ok(Key::H),
        "I" => Ok(Key::I),
        "J" => Ok(Key::J),
        "K" => Ok(Key::K),
        "L" => Ok(Key::L),
        "M" => Ok(Key::M),
        "N" => Ok(Key::N),
        "O" => Ok(Key::O),
        "P" => Ok(Key::P),
        "Q" => Ok(Key::Q),
        "R" => Ok(Key::R),
        "S" => Ok(Key::S),
        "T" => Ok(Key::T),
        "U" => Ok(Key::U),
        "V" => Ok(Key::V),
        "W" => Ok(Key::W),
        "X" => Ok(Key::X),
        "Y" => Ok(Key::Y),
        "Z" => Ok(Key::Z),
        "0" => Ok(Key::KEY_0),
        "1" => Ok(Key::KEY_1),
        "2" => Ok(Key::KEY_2),
        "3" => Ok(Key::KEY_3),
        "4" => Ok(Key::KEY_4),
        "5" => Ok(Key::KEY_5),
        "6" => Ok(Key::KEY_6),
        "7" => Ok(Key::KEY_7),
        "8" => Ok(Key::KEY_8),
        "9" => Ok(Key::KEY_9),
        "SPACE" => Ok(Key::SPACE),
        "ENTER" | "RETURN" => Ok(Key::ENTER),
        "ESCAPE" | "ESC" => Ok(Key::ESCAPE),
        "TAB" => Ok(Key::TAB),
        "BACKSPACE" => Ok(Key::BACKSPACE),
        "DELETE" => Ok(Key::DELETE),
        "UP" => Ok(Key::UP),
        "DOWN" => Ok(Key::DOWN),
        "LEFT" => Ok(Key::LEFT),
        "RIGHT" => Ok(Key::RIGHT),
        "SHIFT" => Ok(Key::SHIFT),
        "CTRL" | "CONTROL" => Ok(Key::CTRL),
        "ALT" => Ok(Key::ALT),
        "F1" => Ok(Key::F1),
        "F2" => Ok(Key::F2),
        "F3" => Ok(Key::F3),
        "F4" => Ok(Key::F4),
        "F5" => Ok(Key::F5),
        "F6" => Ok(Key::F6),
        "F7" => Ok(Key::F7),
        "F8" => Ok(Key::F8),
        "F9" => Ok(Key::F9),
        "F10" => Ok(Key::F10),
        "F11" => Ok(Key::F11),
        "F12" => Ok(Key::F12),
        _ => Err(format!(
            "Unknown key: '{name}'. Use Godot key names: A-Z, 0-9, SPACE, ENTER, ESCAPE, UP, DOWN, LEFT, RIGHT, SHIFT, CTRL, ALT, TAB, BACKSPACE, DELETE, F1-F12."
        )),
    }
}

fn parse_mouse_button(name: &str) -> Result<MouseButton, String> {
    match name.to_lowercase().as_str() {
        "left" => Ok(MouseButton::LEFT),
        "right" => Ok(MouseButton::RIGHT),
        "middle" => Ok(MouseButton::MIDDLE),
        "wheel_up" => Ok(MouseButton::WHEEL_UP),
        "wheel_down" => Ok(MouseButton::WHEEL_DOWN),
        _ => Err(format!(
            "Unknown mouse button: '{name}'. Use: left, right, middle, wheel_up, wheel_down."
        )),
    }
}

/// Convert a JSON value to a Godot Variant.
/// Supports: null, bool, int, float, string, Vector2 (2-elem array),
/// Vector3 (3-elem array), generic array, dict.
pub fn json_to_variant(value: &serde_json::Value) -> Result<Variant, String> {
    match value {
        serde_json::Value::Null => Ok(Variant::nil()),
        serde_json::Value::Bool(b) => Ok(b.to_variant()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.to_variant())
            } else if let Some(f) = n.as_f64() {
                Ok(f.to_variant())
            } else {
                Err(format!("Unsupported number: {n}"))
            }
        }
        serde_json::Value::String(s) => Ok(GString::from(s.as_str()).to_variant()),
        serde_json::Value::Array(arr) => {
            // Detect Vector2/Vector3 from 2/3-element numeric arrays
            if arr.len() == 2 && arr.iter().all(|v| v.is_number()) {
                let x = arr[0].as_f64().unwrap_or(0.0) as f32;
                let y = arr[1].as_f64().unwrap_or(0.0) as f32;
                return Ok(Vector2::new(x, y).to_variant());
            }
            if arr.len() == 3 && arr.iter().all(|v| v.is_number()) {
                let x = arr[0].as_f64().unwrap_or(0.0) as f32;
                let y = arr[1].as_f64().unwrap_or(0.0) as f32;
                let z = arr[2].as_f64().unwrap_or(0.0) as f32;
                return Ok(Vector3::new(x, y, z).to_variant());
            }
            // Generic array
            let godot_array: Array<Variant> = arr
                .iter()
                .map(json_to_variant)
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .collect();
            Ok(godot_array.to_variant())
        }
        serde_json::Value::Object(map) => {
            let mut dict = VarDictionary::new();
            for (k, v) in map {
                let key = GString::from(k.as_str()).to_variant();
                let val = json_to_variant(v)?;
                dict.set(key, val);
            }
            Ok(dict.to_variant())
        }
    }
}
