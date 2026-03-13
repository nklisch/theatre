use crate::harness::{DirectorFixture, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn node_reparent_basic() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("reparent_basic");

    // Create scene with structure: Root > A > Child, Root > B
    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "Node2D", "node_name": "A"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene, "parent_path": "A", "node_type": "Sprite2D", "node_name": "Child"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "Node2D", "node_name": "B"}),
    )
    .unwrap()
    .unwrap_data();

    // Reparent Child from A to B
    let data = f
        .run(
            "node_reparent",
            json!({"scene_path": scene, "node_path": "A/Child", "new_parent_path": "B"}),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["old_path"], "A/Child");
    assert_eq!(data["new_path"], "B/Child");

    // Verify via scene_read
    let tree = f
        .run("scene_read", json!({"scene_path": scene}))
        .unwrap()
        .unwrap_data();
    let root = &tree["root"];

    // A should have no children
    let a = root["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "A")
        .unwrap();
    assert!(a.get("children").is_none() || a["children"].as_array().unwrap().is_empty());

    // B should have Child
    let b = root["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "B")
        .unwrap();
    let b_children = b["children"].as_array().unwrap();
    assert_eq!(b_children.len(), 1);
    assert_eq!(b_children[0]["name"], "Child");
}

#[test]
#[ignore = "requires Godot binary"]
fn node_reparent_with_rename() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("reparent_rename");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "Node2D", "node_name": "Source"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene, "parent_path": "Source", "node_type": "Sprite2D", "node_name": "Sprite"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "Node2D", "node_name": "Target"}),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .run(
            "node_reparent",
            json!({
                "scene_path": scene,
                "node_path": "Source/Sprite",
                "new_parent_path": "Target",
                "new_name": "RenamedSprite"
            }),
        )
        .unwrap()
        .unwrap_data();

    assert_eq!(data["new_path"], "Target/RenamedSprite");
}

#[test]
#[ignore = "requires Godot binary"]
fn node_reparent_root_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("reparent_root");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "Node2D", "node_name": "Child"}),
    )
    .unwrap()
    .unwrap_data();

    let err = f
        .run(
            "node_reparent",
            json!({"scene_path": scene, "node_path": ".", "new_parent_path": "Child"}),
        )
        .unwrap()
        .unwrap_err();

    assert!(err.to_lowercase().contains("root"));
}

#[test]
#[ignore = "requires Godot binary"]
fn node_reparent_circular_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("reparent_circular");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "Node2D", "node_name": "Parent"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene, "parent_path": "Parent", "node_type": "Node2D", "node_name": "Child"}),
    )
    .unwrap()
    .unwrap_data();

    // Try to reparent Parent under its own Child
    let err = f
        .run(
            "node_reparent",
            json!({
                "scene_path": scene,
                "node_path": "Parent",
                "new_parent_path": "Parent/Child"
            }),
        )
        .unwrap()
        .unwrap_err();

    assert!(
        err.to_lowercase().contains("circular")
            || err.to_lowercase().contains("descendant")
            || err.to_lowercase().contains("ancestor")
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn node_reparent_name_collision_returns_error() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("reparent_collision");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "Node2D", "node_name": "A"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene, "parent_path": "A", "node_type": "Sprite2D", "node_name": "Dupe"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene, "node_type": "Node2D", "node_name": "B"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({"scene_path": scene, "parent_path": "B", "node_type": "Node2D", "node_name": "Dupe"}),
    )
    .unwrap()
    .unwrap_data();

    // Reparent A/Dupe to B — name collision with B/Dupe
    let err = f
        .run(
            "node_reparent",
            json!({"scene_path": scene, "node_path": "A/Dupe", "new_parent_path": "B"}),
        )
        .unwrap()
        .unwrap_err();

    assert!(err.to_lowercase().contains("name") || err.to_lowercase().contains("exists"));
}
