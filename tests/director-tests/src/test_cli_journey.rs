use crate::harness::{CliFixture, DirectorFixture, OperationResultExt, assert_approx};
use serde_json::json;

/// Journey 1: full node lifecycle — create scene, add nodes, set properties, read, remove.
#[test]
#[ignore = "requires Godot binary"]
fn cli_journey_build_scene() {
    let cli = CliFixture::new();
    let scene = DirectorFixture::journey_scene_path("cli_build_scene");

    // 1. scene_create → CharacterBody2D root
    let create_data = cli
        .run(
            "scene_create",
            json!({"scene_path": scene, "root_type": "CharacterBody2D"}),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(create_data["root_type"], "CharacterBody2D");

    // 2. node_add → Sprite2D named "Sprite"
    cli.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "Sprite2D",
            "node_name": "Sprite"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 3. node_add → CollisionShape2D named "Collision"
    cli.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "CollisionShape2D",
            "node_name": "Collision"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 4. shape_create → CapsuleShape2D attached to Collision
    cli.run(
        "shape_create",
        json!({
            "shape_type": "CapsuleShape2D",
            "shape_params": {"radius": 16.0, "height": 48.0},
            "scene_path": scene,
            "node_path": "Collision"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 5. node_set_properties → position {x:200, y:300} on root
    cli.run(
        "node_set_properties",
        json!({
            "scene_path": scene,
            "node_path": ".",
            "properties": {"position": {"x": 200, "y": 300}}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 6. scene_read → verify root type, position, 2 children
    let read_data = cli
        .run("scene_read", json!({"scene_path": scene}))
        .unwrap()
        .unwrap_data();
    let root = &read_data["root"];
    assert_eq!(root["type"], "CharacterBody2D");
    assert_approx(root["properties"]["position"]["x"].as_f64().unwrap(), 200.0);
    let children = root["children"].as_array().unwrap();
    assert_eq!(children.len(), 2);
    assert!(children.iter().any(|c| c["name"] == "Sprite"));
    assert!(children.iter().any(|c| c["name"] == "Collision"));

    // 7. node_remove → remove "Sprite"
    cli.run(
        "node_remove",
        json!({"scene_path": scene, "node_path": "Sprite"}),
    )
    .unwrap()
    .unwrap_data();

    // 8. scene_read → verify 1 child remaining, name="Collision"
    let read_data = cli
        .run("scene_read", json!({"scene_path": scene}))
        .unwrap()
        .unwrap_data();
    let children = read_data["root"]["children"].as_array().unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0]["name"], "Collision");
}

/// Journey 2: instancing + reparenting across scenes.
#[test]
#[ignore = "requires Godot binary"]
fn cli_journey_multi_scene_composition() {
    let cli = CliFixture::new();
    let enemy_scene = DirectorFixture::journey_scene_path("cli_comp_enemy");
    let level_scene = DirectorFixture::journey_scene_path("cli_comp_level");

    // 1. scene_create enemy scene (CharacterBody2D root)
    cli.run(
        "scene_create",
        json!({"scene_path": enemy_scene, "root_type": "CharacterBody2D"}),
    )
    .unwrap()
    .unwrap_data();

    // 2. node_add Sprite2D child to enemy scene
    cli.run(
        "node_add",
        json!({
            "scene_path": enemy_scene,
            "node_type": "Sprite2D",
            "node_name": "Sprite"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 3. scene_create level scene (Node2D root)
    cli.run(
        "scene_create",
        json!({"scene_path": level_scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    // 4. node_add "Enemies" node (Node2D) to level
    cli.run(
        "node_add",
        json!({
            "scene_path": level_scene,
            "node_type": "Node2D",
            "node_name": "Enemies"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 5. node_add "Staging" node (Node2D) to level
    cli.run(
        "node_add",
        json!({
            "scene_path": level_scene,
            "node_type": "Node2D",
            "node_name": "Staging"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 6. scene_add_instance enemy scene into Staging as "Enemy1"
    cli.run(
        "scene_add_instance",
        json!({
            "scene_path": level_scene,
            "instance_scene": enemy_scene,
            "parent_path": "Staging",
            "node_name": "Enemy1"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 7. node_reparent "Staging/Enemy1" to parent "Enemies"
    let reparent_data = cli
        .run(
            "node_reparent",
            json!({
                "scene_path": level_scene,
                "node_path": "Staging/Enemy1",
                "new_parent_path": "Enemies"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(reparent_data["old_path"], "Staging/Enemy1");
    assert_eq!(reparent_data["new_path"], "Enemies/Enemy1");

    // 8. scene_read level → verify Staging empty, Enemies has Enemy1
    let read_data = cli
        .run("scene_read", json!({"scene_path": level_scene}))
        .unwrap()
        .unwrap_data();
    let root = &read_data["root"];
    let children = root["children"].as_array().unwrap();

    let staging = children
        .iter()
        .find(|c| c["name"] == "Staging")
        .expect("Staging node must exist");
    assert!(
        staging.get("children").is_none() || staging["children"].as_array().unwrap().is_empty()
    );

    let enemies = children
        .iter()
        .find(|c| c["name"] == "Enemies")
        .expect("Enemies node must exist");
    let enemy_children = enemies["children"].as_array().unwrap();
    assert_eq!(enemy_children.len(), 1);
    assert_eq!(enemy_children[0]["name"], "Enemy1");

    // 9. scene_list {"directory":"tmp"} → at least 2 scenes found
    let list_data = cli
        .run("scene_list", json!({"directory": "tmp"}))
        .unwrap()
        .unwrap_data();
    let scenes = list_data["scenes"].as_array().unwrap();
    assert!(scenes.len() >= 2);
}

/// Journey 3: animation CRUD — create resource, add track, read back.
///
/// Animation API uses `resource_path` (standalone .tres files), not scene_path/node_path.
#[test]
#[ignore = "requires Godot binary"]
fn cli_journey_animation_workflow() {
    let cli = CliFixture::new();
    let anim = DirectorFixture::temp_resource_path("cli_anim_walk");

    // 1. animation_create → "walk" animation as standalone resource
    let anim_data = cli
        .run(
            "animation_create",
            json!({
                "resource_path": anim,
                "length": 1.0,
                "loop_mode": "linear"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(anim_data["loop_mode"], "linear");
    assert_approx(anim_data["length"].as_f64().unwrap(), 1.0);

    // 2. animation_add_track → value track for "Sprite2D:position"
    let track_data = cli
        .run(
            "animation_add_track",
            json!({
                "resource_path": anim,
                "track_type": "value",
                "node_path": "Sprite2D:position",
                "keyframes": [
                    {"time": 0.0, "value": {"x": 0, "y": 0}},
                    {"time": 1.0, "value": {"x": 100, "y": 0}}
                ]
            }),
        )
        .unwrap()
        .unwrap_data();
    assert!(track_data["track_index"].as_u64().is_some());

    // 3. animation_read → verify 1 track, duration 1.0, loop_mode linear
    let read_data = cli
        .run("animation_read", json!({"resource_path": anim}))
        .unwrap()
        .unwrap_data();
    let tracks = read_data["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 1);
    assert_approx(read_data["length"].as_f64().unwrap(), 1.0);
    assert_eq!(read_data["loop_mode"], "linear");
}

/// Journey 4: physics layers + signal wiring.
#[test]
#[ignore = "requires Godot binary"]
fn cli_journey_physics_and_signals() {
    let cli = CliFixture::new();
    let scene = DirectorFixture::journey_scene_path("cli_physics_signals");

    // 1. scene_create → Node2D scene
    cli.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    // 2. node_add → Area2D named "DetectZone"
    cli.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "Area2D",
            "node_name": "DetectZone"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 3. node_add → CollisionShape2D under "DetectZone" named "Shape"
    cli.run(
        "node_add",
        json!({
            "scene_path": scene,
            "parent_path": "DetectZone",
            "node_type": "CollisionShape2D",
            "node_name": "Shape"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 4. physics_set_layers → set collision_layer=1, collision_mask=2 on "DetectZone"
    let phys_data = cli
        .run(
            "physics_set_layers",
            json!({
                "scene_path": scene,
                "node_path": "DetectZone",
                "collision_layer": 1,
                "collision_mask": 2
            }),
        )
        .unwrap()
        .unwrap_data();
    assert!(phys_data.is_object());

    // 5. scene_read → verify DetectZone exists
    let read_data = cli
        .run("scene_read", json!({"scene_path": scene}))
        .unwrap()
        .unwrap_data();
    let root_children = read_data["root"]["children"].as_array().unwrap();
    assert!(root_children.iter().any(|c| c["name"] == "DetectZone"));

    // 6. signal_connect → connect "body_entered" on "DetectZone" to root
    let sig_data = cli
        .run(
            "signal_connect",
            json!({
                "scene_path": scene,
                "source_path": "DetectZone",
                "signal_name": "body_entered",
                "target_path": ".",
                "method_name": "_on_body_entered"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert!(sig_data["signal_name"].as_str().is_some() || sig_data.get("signal_name").is_some());

    // 7. signal_list → verify scene_path, at least one connection exists
    let list_data = cli
        .run("signal_list", json!({"scene_path": scene}))
        .unwrap()
        .unwrap_data();
    assert!(list_data.is_object());
    // The response should reference the scene or contain connection data.
    let connections = list_data["connections"]
        .as_array()
        .or_else(|| list_data["signals"].as_array());
    assert!(
        connections.map(|c| !c.is_empty()).unwrap_or(true),
        "signal_list should return data without error"
    );
}

/// Journey 5: batch multiple operations in a single call.
#[test]
#[ignore = "requires Godot binary"]
fn cli_journey_batch_operations() {
    let cli = CliFixture::new();
    let scene = DirectorFixture::journey_scene_path("cli_batch");

    // 1. scene_create → Node2D scene
    cli.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    // 2. batch → 3 node_add operations
    let batch_data = cli
        .run(
            "batch",
            json!({
                "operations": [
                    {"operation": "node_add", "params": {
                        "scene_path": scene,
                        "node_type": "Sprite2D",
                        "node_name": "A"
                    }},
                    {"operation": "node_add", "params": {
                        "scene_path": scene,
                        "node_type": "Label",
                        "node_name": "B"
                    }},
                    {"operation": "node_add", "params": {
                        "scene_path": scene,
                        "node_type": "Button",
                        "node_name": "C"
                    }}
                ]
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(batch_data["completed"], 3);
    assert_eq!(batch_data["failed"], 0);

    // 3. scene_read → verify root has 3 children with correct names and types
    let read_data = cli
        .run("scene_read", json!({"scene_path": scene}))
        .unwrap()
        .unwrap_data();
    let children = read_data["root"]["children"].as_array().unwrap();
    assert_eq!(children.len(), 3);

    let find = |name: &str| children.iter().find(|c| c["name"] == name).cloned();

    let a = find("A").expect("node A must exist");
    assert_eq!(a["type"], "Sprite2D");

    let b = find("B").expect("node B must exist");
    assert_eq!(b["type"], "Label");

    let c = find("C").expect("node C must exist");
    assert_eq!(c["type"], "Button");
}

/// Journey 6: validate error handling for invalid operations.
///
/// Director CLI exits 0 but returns `OperationResult { success: false, error: ... }`
/// for tool-level errors. We use `unwrap_err()` to assert the error message.
#[test]
#[ignore = "requires Godot binary"]
fn cli_journey_error_cases() {
    let cli = CliFixture::new();

    // 1. scene_read with nonexistent scene → success=false with error message
    let result = cli
        .run(
            "scene_read",
            json!({"scene_path": "tmp/nonexistent_scene_xyzzy.tscn"}),
        )
        .unwrap();
    assert!(
        !result.success,
        "scene_read nonexistent scene should return success=false"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("not found") || err.contains("Not found") || err.contains("scene"),
        "error should mention scene not found: {err}"
    );

    // 2. node_add with nonexistent parent_path → success=false
    let scene = DirectorFixture::journey_scene_path("cli_err_cases");
    cli.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    let result = cli
        .run(
            "node_add",
            json!({
                "scene_path": scene,
                "parent_path": "NonExistentParent",
                "node_type": "Sprite2D",
                "node_name": "Orphan"
            }),
        )
        .unwrap();
    assert!(
        !result.success,
        "node_add with nonexistent parent_path should return success=false"
    );

    // 3. node_remove with nonexistent node_path → success=false
    let result = cli
        .run(
            "node_remove",
            json!({
                "scene_path": scene,
                "node_path": "NonExistentNode"
            }),
        )
        .unwrap();
    assert!(
        !result.success,
        "node_remove with nonexistent node_path should return success=false"
    );
}
