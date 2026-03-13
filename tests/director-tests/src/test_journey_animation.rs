use crate::harness::{DirectorFixture, OperationResultExt, assert_approx};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_create_character_animations() {
    let f = DirectorFixture::new();
    let idle = DirectorFixture::temp_resource_path("anim_idle");
    let walk = DirectorFixture::temp_resource_path("anim_walk");
    let attack = DirectorFixture::temp_resource_path("anim_attack");

    // 1. Create "idle" animation, length=2.0, loop_mode=linear
    let idle_data = f
        .run(
            "animation_create",
            json!({
                "resource_path": idle,
                "length": 2.0,
                "loop_mode": "linear"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(idle_data["loop_mode"], "linear");
    assert_approx(idle_data["length"].as_f64().unwrap(), 2.0);

    // 2. Add value track on "Sprite2D:modulate" (color pulse)
    f.run(
        "animation_add_track",
        json!({
            "resource_path": idle,
            "track_type": "value",
            "node_path": "Sprite2D:modulate",
            "keyframes": [
                {"time": 0.0, "value": "#ffffff"},
                {"time": 1.0, "value": "#ffcccc"},
                {"time": 2.0, "value": "#ffffff"}
            ]
        }),
    )
    .unwrap()
    .unwrap_data();

    // 3. Add bezier track on "Sprite2D:modulate:a" (alpha breathe)
    f.run(
        "animation_add_track",
        json!({
            "resource_path": idle,
            "track_type": "bezier",
            "node_path": "Sprite2D:modulate:a",
            "keyframes": [
                {
                    "time": 0.0,
                    "value": 1.0,
                    "in_handle": {"x": -0.2, "y": 0.0},
                    "out_handle": {"x": 0.2, "y": 0.0}
                },
                {
                    "time": 1.0,
                    "value": 0.6,
                    "in_handle": {"x": -0.2, "y": 0.0},
                    "out_handle": {"x": 0.2, "y": 0.0}
                },
                {
                    "time": 2.0,
                    "value": 1.0,
                    "in_handle": {"x": -0.2, "y": 0.0},
                    "out_handle": {"x": 0.2, "y": 0.0}
                }
            ]
        }),
    )
    .unwrap()
    .unwrap_data();

    // 4. animation_read — verify idle has 2 tracks, correct types
    let idle_read = f
        .run("animation_read", json!({"resource_path": idle}))
        .unwrap()
        .unwrap_data();
    assert_approx(idle_read["length"].as_f64().unwrap(), 2.0);
    assert_eq!(idle_read["loop_mode"], "linear");
    let idle_tracks = idle_read["tracks"].as_array().unwrap();
    assert_eq!(idle_tracks.len(), 2);
    assert!(idle_tracks.iter().any(|t| t["type"] == "value"));
    assert!(idle_tracks.iter().any(|t| t["type"] == "bezier"));

    // 5. Create "walk" animation, length=0.8, loop_mode=linear
    let walk_data = f
        .run(
            "animation_create",
            json!({
                "resource_path": walk,
                "length": 0.8,
                "loop_mode": "linear"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(walk_data["loop_mode"], "linear");
    assert_approx(walk_data["length"].as_f64().unwrap(), 0.8);

    // 6. Add position_3d track on "." (bobbing motion)
    f.run(
        "animation_add_track",
        json!({
            "resource_path": walk,
            "track_type": "position_3d",
            "node_path": ".",
            "keyframes": [
                {"time": 0.0, "value": {"x": 0, "y": 0, "z": 0}},
                {"time": 0.4, "value": {"x": 0, "y": 0.1, "z": 0}},
                {"time": 0.8, "value": {"x": 0, "y": 0, "z": 0}}
            ]
        }),
    )
    .unwrap()
    .unwrap_data();

    // 7. Add method track on "." (play_footstep at 0.0 and 0.4)
    f.run(
        "animation_add_track",
        json!({
            "resource_path": walk,
            "track_type": "method",
            "node_path": ".",
            "keyframes": [
                {"time": 0.0, "method": "play_footstep", "args": []},
                {"time": 0.4, "method": "play_footstep", "args": []}
            ]
        }),
    )
    .unwrap()
    .unwrap_data();

    // 8. Add value track on "Sprite2D:frame" (discrete sprite frames)
    f.run(
        "animation_add_track",
        json!({
            "resource_path": walk,
            "track_type": "value",
            "node_path": "Sprite2D:frame",
            "update_mode": "discrete",
            "keyframes": [
                {"time": 0.0, "value": 0},
                {"time": 0.2, "value": 1},
                {"time": 0.4, "value": 2},
                {"time": 0.6, "value": 3}
            ]
        }),
    )
    .unwrap()
    .unwrap_data();

    // 9. animation_read — verify walk has 3 tracks, correct types and keyframe counts
    let walk_read = f
        .run("animation_read", json!({"resource_path": walk}))
        .unwrap()
        .unwrap_data();
    let walk_tracks = walk_read["tracks"].as_array().unwrap();
    assert_eq!(walk_tracks.len(), 3);
    assert!(walk_tracks.iter().any(|t| t["type"] == "position_3d"));
    assert!(walk_tracks.iter().any(|t| t["type"] == "method"));
    assert!(walk_tracks.iter().any(|t| t["type"] == "value"));

    // Verify method keyframes have correct method names
    let method_track = walk_tracks.iter().find(|t| t["type"] == "method").unwrap();
    let method_kfs = method_track["keyframes"].as_array().unwrap();
    assert_eq!(method_kfs.len(), 2);
    assert_eq!(method_kfs[0]["method"], "play_footstep");
    assert_eq!(method_kfs[1]["method"], "play_footstep");
    assert_approx(method_kfs[0]["time"].as_f64().unwrap(), 0.0);
    assert_approx(method_kfs[1]["time"].as_f64().unwrap(), 0.4);

    // 10. Create "attack" animation, length=0.5, loop_mode=none
    f.run(
        "animation_create",
        json!({
            "resource_path": attack,
            "length": 0.5,
            "loop_mode": "none"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 11. Add rotation_3d track on "WeaponPivot"
    f.run(
        "animation_add_track",
        json!({
            "resource_path": attack,
            "track_type": "rotation_3d",
            "node_path": "WeaponPivot",
            "keyframes": [
                {"time": 0.0, "value": {"x": 0, "y": 0, "z": 0, "w": 1}},
                {"time": 0.25, "value": {"x": 0, "y": 0, "z": 0.707, "w": 0.707}},
                {"time": 0.5, "value": {"x": 0, "y": 0, "z": 0, "w": 1}}
            ]
        }),
    )
    .unwrap()
    .unwrap_data();

    // 12. Add scale_3d track on "WeaponPivot/Weapon"
    f.run(
        "animation_add_track",
        json!({
            "resource_path": attack,
            "track_type": "scale_3d",
            "node_path": "WeaponPivot/Weapon",
            "keyframes": [
                {"time": 0.0, "value": {"x": 1, "y": 1, "z": 1}},
                {"time": 0.25, "value": {"x": 1.2, "y": 1.2, "z": 1.2}},
                {"time": 0.5, "value": {"x": 1, "y": 1, "z": 1}}
            ]
        }),
    )
    .unwrap()
    .unwrap_data();

    // Verify 2 tracks before removal
    let attack_before = f
        .run("animation_read", json!({"resource_path": attack}))
        .unwrap()
        .unwrap_data();
    assert_eq!(attack_before["tracks"].as_array().unwrap().len(), 2);

    // 13. animation_remove_track — Remove scale_3d track by node_path
    let remove_data = f
        .run(
            "animation_remove_track",
            json!({
                "resource_path": attack,
                "node_path": "WeaponPivot/Weapon"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert!(remove_data["tracks_removed"].as_u64().unwrap() >= 1);

    // 14. animation_read — verify attack has 1 track remaining (rotation only)
    let attack_after = f
        .run("animation_read", json!({"resource_path": attack}))
        .unwrap()
        .unwrap_data();
    let attack_tracks = attack_after["tracks"].as_array().unwrap();
    assert_eq!(attack_tracks.len(), 1);
    assert_eq!(attack_tracks[0]["type"], "rotation_3d");
    assert_eq!(attack_tracks[0]["node_path"], "WeaponPivot");
}
