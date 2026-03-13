use crate::harness::{DirectorFixture, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_scene_diff_tracks_iterative_changes() {
    let f = DirectorFixture::new();
    let before = DirectorFixture::journey_scene_path("diff_before");
    let after = DirectorFixture::journey_scene_path("diff_after");

    // 1. Create "before.tscn" with Node2D + Sprite2D + Label
    f.run(
        "scene_create",
        json!({"scene_path": before, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({
            "scene_path": before,
            "node_type": "Sprite2D",
            "node_name": "Sprite"
        }),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_set_properties",
        json!({
            "scene_path": before,
            "node_path": "Sprite",
            "properties": {"position": {"x": 100, "y": 100}}
        }),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({
            "scene_path": before,
            "node_type": "Label",
            "node_name": "Label"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 2. Create "after.tscn" as a copy (same initial nodes)
    f.run(
        "scene_create",
        json!({"scene_path": after, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({
            "scene_path": after,
            "node_type": "Sprite2D",
            "node_name": "Sprite"
        }),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_set_properties",
        json!({
            "scene_path": after,
            "node_path": "Sprite",
            "properties": {"position": {"x": 100, "y": 100}}
        }),
    )
    .unwrap()
    .unwrap_data();
    f.run(
        "node_add",
        json!({
            "scene_path": after,
            "node_type": "Label",
            "node_name": "Label"
        }),
    )
    .unwrap()
    .unwrap_data();

    // Verify no diff at this point (scenes are identical)
    let no_diff = f
        .run("scene_diff", json!({"scene_a": before, "scene_b": after}))
        .unwrap()
        .unwrap_data();
    assert!(
        no_diff["added"].as_array().unwrap().is_empty(),
        "No changes yet"
    );
    assert!(
        no_diff["removed"].as_array().unwrap().is_empty(),
        "No changes yet"
    );
    assert!(
        no_diff["changed"].as_array().unwrap().is_empty(),
        "No changes yet"
    );

    // 3. Add new "Particles" node to after
    f.run(
        "node_add",
        json!({
            "scene_path": after,
            "node_type": "GPUParticles2D",
            "node_name": "Particles"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 4. Remove "Label" from after
    f.run(
        "node_remove",
        json!({
            "scene_path": after,
            "node_path": "Label"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 5. Change Sprite position in after
    f.run(
        "node_set_properties",
        json!({
            "scene_path": after,
            "node_path": "Sprite",
            "properties": {"position": {"x": 250, "y": 150}}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 6. scene_diff — Compare before vs after
    let diff = f
        .run("scene_diff", json!({"scene_a": before, "scene_b": after}))
        .unwrap()
        .unwrap_data();

    // 7. Verify: added=[Particles], removed=[Label], changed includes Sprite position
    // scene_diff returns node_path (not name) in added/removed/changed entries
    let added = diff["added"].as_array().unwrap();
    assert_eq!(added.len(), 1, "Exactly 1 node should be added");
    assert!(
        added[0]["node_path"]
            .as_str()
            .unwrap_or("")
            .contains("Particles"),
        "Added node should be Particles, got: {:?}",
        added[0]
    );

    let removed = diff["removed"].as_array().unwrap();
    assert_eq!(removed.len(), 1, "Exactly 1 node should be removed");
    assert!(
        removed[0]["node_path"]
            .as_str()
            .unwrap_or("")
            .contains("Label"),
        "Removed node should be Label, got: {:?}",
        removed[0]
    );

    let changed = diff["changed"].as_array().unwrap();
    assert!(
        !changed.is_empty(),
        "Sprite position change should appear in diff"
    );
    assert!(
        changed
            .iter()
            .any(|c| c["node_path"].as_str().unwrap_or("").contains("Sprite")),
        "Sprite should appear in changed list"
    );
}
