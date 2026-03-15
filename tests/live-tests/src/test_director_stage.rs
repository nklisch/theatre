use crate::dual_test;
use crate::harness::*;
use crate::harness::assertions::*;
use serde_json::json;

const LIVE_3D: &str = "res://live_scene_3d.tscn";

/// Journey: Build a game level with Director, verify every layer with scene_read.
///
/// Tests the full Director scene authoring workflow: create scene, add multiple
/// node types with properties, set groups, verify the complete tree via scene_read,
/// then use scene_diff to confirm self-consistency.
///
/// Steps:
///   1. director(scene_create) → new scene with Node3D root
///   2. director(node_add, StaticBody3D "Floor")
///   3. director(node_add, CharacterBody3D "Player")
///   4. director(node_add, CharacterBody3D "Enemy")
///   5. director(node_add, MeshInstance3D "Obstacle")
///   6. director(node_set_properties, Player, {speed: 5.0})
///   7. director(node_set_groups, Enemy, add=["enemies"])
///   8. director(scene_read) → verify all 4 children present with correct types
///   9. director(node_add, CollisionShape3D under Player)
///  10. director(scene_read) → verify Player has child CollisionShape3D
///  11. director(scene_diff, path_a=scene, path_b=scene) → self-diff, no changes
///  12. Stage: scene_tree(roots) → verify live scene tree is accessible
///  13. Stage: spatial_snapshot(summary) → scene is alive and collecting data
async fn journey_director_builds_level(b: &impl LiveBackend) {
    let scene_path = "tmp/live_journey_level.tscn";
    let root_name = "LiveJourneyLevel"; // Godot derives from filename

    // Step 1: create scene
    b.director(
        "scene_create",
        json!({"scene_path": scene_path, "root_type": "Node3D"}),
    )
    .await
    .expect("scene_create")
    .unwrap_data();

    // Steps 2-5: add nodes
    for (name, node_type) in [
        ("Floor", "StaticBody3D"),
        ("Player", "CharacterBody3D"),
        ("Enemy", "CharacterBody3D"),
        ("Obstacle", "MeshInstance3D"),
    ] {
        b.director(
            "node_add",
            json!({
                "scene_path": scene_path,
                "parent": root_name,
                "node_name": name,
                "node_type": node_type
            }),
        )
        .await
        .unwrap_or_else(|e| panic!("node_add {name} failed: {e}"))
        .unwrap_data();
    }

    // Step 6: set properties on Player (use a real Godot property, not a script export)
    b.director(
        "node_set_properties",
        json!({
            "scene_path": scene_path,
            "node_path": "Player",
            "properties": {"collision_layer": 2}
        }),
    )
    .await
    .expect("set_properties")
    .unwrap_data();

    // Step 7: set groups on Enemy
    b.director(
        "node_set_groups",
        json!({
            "scene_path": scene_path,
            "node_path": "Enemy",
            "add": ["enemies"]
        }),
    )
    .await
    .expect("set_groups")
    .unwrap_data();

    // Step 8: read scene — verify all children
    let read1 = b
        .director("scene_read", json!({"scene_path": scene_path}))
        .await
        .expect("scene_read 1")
        .unwrap_data();
    let scene_str = serde_json::to_string(&read1).unwrap_or_default();
    for name in ["Floor", "Player", "Enemy", "Obstacle"] {
        assert!(
            scene_str.contains(name),
            "Scene should contain {name}: {read1}"
        );
    }
    assert!(
        scene_str.contains("StaticBody3D"),
        "Floor should be StaticBody3D"
    );
    assert!(
        scene_str.contains("CharacterBody3D"),
        "Player/Enemy should be CharacterBody3D"
    );
    assert!(
        scene_str.contains("MeshInstance3D"),
        "Obstacle should be MeshInstance3D"
    );

    // Step 9: add child under Player
    b.director(
        "node_add",
        json!({
            "scene_path": scene_path,
            "parent": "Player",
            "node_name": "Hitbox",
            "node_type": "CollisionShape3D"
        }),
    )
    .await
    .expect("add Hitbox under Player")
    .unwrap_data();

    // Step 10: verify hierarchy
    let read2 = b
        .director("scene_read", json!({"scene_path": scene_path}))
        .await
        .expect("scene_read 2")
        .unwrap_data();
    let scene_str2 = serde_json::to_string(&read2).unwrap_or_default();
    assert!(
        scene_str2.contains("Hitbox"),
        "Player should have Hitbox child: {read2}"
    );
    assert!(
        scene_str2.contains("CollisionShape3D"),
        "Hitbox should be CollisionShape3D"
    );

    // Step 11: self-diff should show no differences
    let diff = b
        .director(
            "scene_diff",
            json!({
                "scene_a": scene_path,
                "scene_b": scene_path
            }),
        )
        .await
        .expect("scene_diff self")
        .unwrap_data();
    // Self-diff should either return empty changes or indicate identical
    let diff_str = serde_json::to_string(&diff).unwrap_or_default();
    // Accept either no differences or an explicit "identical" flag
    let has_changes = diff["added"]
        .as_array()
        .map(|a| !a.is_empty())
        .unwrap_or(false)
        || diff["removed"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false);
    assert!(
        !has_changes,
        "Self-diff should show no changes: {diff_str}"
    );

    // Step 12: Stage verifies the running scene is accessible
    let tree = b
        .stage("scene_tree", json!({"action": "roots"}))
        .await
        .expect("scene_tree roots")
        .unwrap_data();
    assert!(
        tree["roots"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false),
        "Live scene tree should have roots"
    );

    // Step 13: Stage snapshot confirms the live scene is collecting spatial data
    let snap = b
        .stage("spatial_snapshot", json!({"detail": "summary"}))
        .await
        .expect("spatial_snapshot summary")
        .unwrap_data();
    assert!(
        !snap.is_null(),
        "Summary snapshot should return data from the live scene"
    );
}

/// Journey: Batch-create a scene, then build an animation resource with tracks.
///
/// Tests Director batch operations and the animation resource workflow end-to-end:
/// create + add nodes in one batch call, then create an animation resource with
/// multiple tracks and keyframes, read it back, and verify the full structure.
///
/// Steps:
///   1. director(batch) → scene_create + 3x node_add in one call
///   2. director(batch result) → completed=4, failed=0
///   3. director(scene_read) → verify all 3 nodes present
///   4. director(animation_create) → walk.tres, length=2.0, loop_mode=linear
///   5. director(animation_add_track, position_3d) → 2 keyframes
///   6. director(animation_add_track, value) → modulate track, 3 keyframes
///   7. director(animation_read) → length=2.0, loop_mode=linear,
///      2 tracks, correct keyframe counts
///   8. director(animation_remove_track, index=1) → remove value track
///   9. director(animation_read) → 1 track remaining (position_3d)
async fn journey_batch_and_animation(b: &impl LiveBackend) {
    // Use timestamp-based names to avoid accumulation from prior runs
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let scene_path = format!("tmp/live_batch_{ts}.tscn");
    let anim_path = format!("tmp/live_walk_{ts}.tres");

    // Step 1-2: batch create scene + nodes
    let batch_result = b
        .director(
            "batch",
            json!({
                "operations": [
                    {"operation": "scene_create", "params": {
                        "scene_path": scene_path, "root_type": "Node3D"
                    }},
                    {"operation": "node_add", "params": {
                        "scene_path": scene_path,
                        "node_type": "CharacterBody3D",
                        "node_name": "Hero"
                    }},
                    {"operation": "node_add", "params": {
                        "scene_path": scene_path,
                        "node_type": "MeshInstance3D",
                        "node_name": "HeroMesh"
                    }},
                    {"operation": "node_add", "params": {
                        "scene_path": scene_path,
                        "node_type": "AnimationPlayer",
                        "node_name": "AnimPlayer"
                    }}
                ]
            }),
        )
        .await
        .expect("batch")
        .unwrap_data();
    assert_eq!(
        batch_result["completed"], 4,
        "All 4 operations should complete"
    );
    assert_eq!(
        batch_result["failed"], 0,
        "No operations should fail"
    );

    // Step 3: verify all nodes
    let read = b
        .director("scene_read", json!({"scene_path": scene_path}))
        .await
        .expect("scene_read")
        .unwrap_data();
    let scene_str = serde_json::to_string(&read).unwrap_or_default();
    for name in ["Hero", "HeroMesh", "AnimPlayer"] {
        assert!(
            scene_str.contains(name),
            "Scene should contain {name}: {read}"
        );
    }

    // Step 4: create animation resource
    let anim_create = b
        .director(
            "animation_create",
            json!({
                "resource_path": anim_path,
                "length": 2.0,
                "loop_mode": "linear"
            }),
        )
        .await
        .expect("animation_create")
        .unwrap_data();
    assert!(
        anim_create["path"]
            .as_str()
            .map(|p| p.contains("walk"))
            .unwrap_or(false),
        "Animation path should contain 'walk': {anim_create}"
    );

    // Step 5: add position_3d track
    let track1 = b
        .director(
            "animation_add_track",
            json!({
                "resource_path": anim_path,
                "track_type": "position_3d",
                "node_path": "Hero",
                "keyframes": [
                    {"time": 0.0, "value": {"x": 0.0, "y": 0.0, "z": 0.0}},
                    {"time": 2.0, "value": {"x": 10.0, "y": 0.0, "z": 0.0}}
                ]
            }),
        )
        .await
        .expect("add position track")
        .unwrap_data();
    assert!(track1["track_index"].as_u64().is_some(), "Should return track_index");
    let track1_idx = track1["track_index"].as_u64().unwrap();
    assert_eq!(track1["keyframe_count"], 2);

    // Step 6: add value track (modulate)
    let track2 = b
        .director(
            "animation_add_track",
            json!({
                "resource_path": anim_path,
                "track_type": "value",
                "node_path": "HeroMesh:modulate",
                "keyframes": [
                    {"time": 0.0, "value": "#ffffff"},
                    {"time": 1.0, "value": "#ff0000"},
                    {"time": 2.0, "value": "#ffffff"}
                ]
            }),
        )
        .await
        .expect("add value track")
        .unwrap_data();
    let track2_idx = track2["track_index"].as_u64().unwrap();
    assert!(track2_idx > track1_idx, "Second track index should be after first");
    assert_eq!(track2["keyframe_count"], 3);

    // Step 7: read animation back — full verification
    let anim_read = b
        .director("animation_read", json!({"resource_path": anim_path}))
        .await
        .expect("animation_read")
        .unwrap_data();
    assert!(
        (anim_read["length"].as_f64().unwrap_or(0.0) - 2.0).abs() < 0.01,
        "Length should be 2.0"
    );
    assert_eq!(
        anim_read["loop_mode"].as_str(),
        Some("linear"),
        "Loop mode should be linear"
    );
    let tracks = anim_read["tracks"]
        .as_array()
        .expect("tracks array");
    assert_eq!(tracks.len(), 2, "Should have 2 tracks");
    assert_eq!(
        tracks[0]["type"].as_str(),
        Some("position_3d"),
        "First track should be position_3d"
    );
    assert_eq!(
        tracks[1]["type"].as_str(),
        Some("value"),
        "Second track should be value"
    );

    // Step 8: remove the value track
    b.director(
        "animation_remove_track",
        json!({
            "resource_path": anim_path,
            "track_index": track2_idx
        }),
    )
    .await
    .expect("remove value track")
    .unwrap_data();

    // Step 9: verify only position track remains
    let anim_final = b
        .director("animation_read", json!({"resource_path": anim_path}))
        .await
        .expect("final animation_read")
        .unwrap_data();
    let final_tracks = anim_final["tracks"]
        .as_array()
        .expect("tracks array");
    assert_eq!(
        final_tracks.len(),
        1,
        "Should have 1 track after removal"
    );
    assert_eq!(
        final_tracks[0]["type"].as_str(),
        Some("position_3d"),
        "Remaining track should be position_3d"
    );
}

dual_test!(
    journey_director_builds_level,
    LIVE_3D,
    journey_director_builds_level
);
dual_test!(
    journey_batch_and_animation,
    LIVE_3D,
    journey_batch_and_animation
);
