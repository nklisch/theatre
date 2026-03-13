use crate::harness::{DirectorFixture, assert_approx, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn animation_create_basic() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_basic.tres";
    let data = f
        .run(
            "animation_create",
            json!({
                "resource_path": path,
                "length": 2.0,
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["path"], path);
    assert_approx(data["length"].as_f64().unwrap(), 2.0);
    assert_eq!(data["loop_mode"], "none");
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_create_with_loop_and_step() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_loop.tres";
    let data = f
        .run(
            "animation_create",
            json!({
                "resource_path": path,
                "length": 1.5,
                "loop_mode": "linear",
                "step": 0.05,
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["loop_mode"], "linear");

    // Verify via animation_read
    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    assert_approx(read["length"].as_f64().unwrap(), 1.5);
    assert_eq!(read["loop_mode"], "linear");
    assert_approx(read["step"].as_f64().unwrap(), 0.05);
    assert!(read["tracks"].as_array().unwrap().is_empty());
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_create_rejects_invalid_loop_mode() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "animation_create",
            json!({
                "resource_path": "tmp/bad.tres",
                "length": 1.0,
                "loop_mode": "bounce",
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("Invalid loop_mode"));
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_create_rejects_zero_length() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "animation_create",
            json!({
                "resource_path": "tmp/bad.tres",
                "length": 0.0,
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("length must be positive"));
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_add_value_track() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_value.tres";
    f.run(
        "animation_create",
        json!({
            "resource_path": path,
            "length": 1.0,
        }),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .run(
            "animation_add_track",
            json!({
                "resource_path": path,
                "track_type": "value",
                "node_path": "Sprite2D:modulate",
                "keyframes": [
                    {"time": 0.0, "value": "#ffffff"},
                    {"time": 0.5, "value": "#ff0000", "transition": 0.5},
                    {"time": 1.0, "value": "#ffffff"},
                ]
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["track_index"], 0);
    assert_eq!(data["keyframe_count"], 3);

    // Read back and verify
    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    let tracks = read["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0]["type"], "value");
    assert_eq!(tracks[0]["node_path"], "Sprite2D:modulate");
    let kfs = tracks[0]["keyframes"].as_array().unwrap();
    assert_eq!(kfs.len(), 3);
    assert_approx(kfs[1]["time"].as_f64().unwrap(), 0.5);
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_add_position_3d_track() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_pos3d.tres";
    f.run(
        "animation_create",
        json!({
            "resource_path": path,
            "length": 2.0,
        }),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .run(
            "animation_add_track",
            json!({
                "resource_path": path,
                "track_type": "position_3d",
                "node_path": "MeshInstance3D",
                "keyframes": [
                    {"time": 0.0, "value": {"x": 0, "y": 0, "z": 0}},
                    {"time": 1.0, "value": {"x": 5, "y": 2, "z": -3}},
                    {"time": 2.0, "value": {"x": 0, "y": 0, "z": 0}},
                ],
                "interpolation": "cubic",
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["track_index"], 0);
    assert_eq!(data["keyframe_count"], 3);

    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    let track = &read["tracks"][0];
    assert_eq!(track["type"], "position_3d");
    assert_eq!(track["interpolation"], "cubic");
    let kf1 = &track["keyframes"][1];
    assert_approx(kf1["value"]["x"].as_f64().unwrap(), 5.0);
    assert_approx(kf1["value"]["y"].as_f64().unwrap(), 2.0);
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_add_method_track() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_method.tres";
    f.run(
        "animation_create",
        json!({
            "resource_path": path,
            "length": 1.0,
        }),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .run(
            "animation_add_track",
            json!({
                "resource_path": path,
                "track_type": "method",
                "node_path": "../Player",
                "keyframes": [
                    {"time": 0.0, "method": "play_sfx", "args": ["jump"]},
                    {"time": 0.5, "method": "set_speed", "args": [2.0]},
                ]
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["keyframe_count"], 2);

    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    let kfs = read["tracks"][0]["keyframes"].as_array().unwrap();
    assert_eq!(kfs[0]["method"], "play_sfx");
    assert_eq!(kfs[1]["method"], "set_speed");
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_add_bezier_track() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_bezier.tres";
    f.run(
        "animation_create",
        json!({
            "resource_path": path,
            "length": 1.0,
        }),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .run(
            "animation_add_track",
            json!({
                "resource_path": path,
                "track_type": "bezier",
                "node_path": "Sprite2D:modulate:a",
                "keyframes": [
                    {
                        "time": 0.0,
                        "value": 1.0,
                        "in_handle": {"x": -0.5, "y": 0.0},
                        "out_handle": {"x": 0.5, "y": -0.5},
                    },
                    {
                        "time": 1.0,
                        "value": 0.0,
                        "in_handle": {"x": -0.5, "y": 0.5},
                        "out_handle": {"x": 0.5, "y": 0.0},
                    },
                ]
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["keyframe_count"], 2);

    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    let kfs = read["tracks"][0]["keyframes"].as_array().unwrap();
    assert_approx(kfs[0]["value"].as_f64().unwrap(), 1.0);
    assert_approx(kfs[1]["value"].as_f64().unwrap(), 0.0);
    // Verify handles
    assert_approx(kfs[0]["out_handle"]["y"].as_f64().unwrap(), -0.5);
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_add_multiple_tracks() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_multi.tres";
    f.run(
        "animation_create",
        json!({
            "resource_path": path,
            "length": 2.0,
        }),
    )
    .unwrap()
    .unwrap_data();

    // Add position track
    let d1 = f
        .run(
            "animation_add_track",
            json!({
                "resource_path": path,
                "track_type": "position_3d",
                "node_path": "Mesh",
                "keyframes": [
                    {"time": 0.0, "value": {"x": 0, "y": 0, "z": 0}},
                    {"time": 2.0, "value": {"x": 10, "y": 0, "z": 0}},
                ]
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(d1["track_index"], 0);

    // Add rotation track
    let d2 = f
        .run(
            "animation_add_track",
            json!({
                "resource_path": path,
                "track_type": "rotation_3d",
                "node_path": "Mesh",
                "keyframes": [
                    {"time": 0.0, "value": {"x": 0, "y": 0, "z": 0, "w": 1}},
                    {"time": 2.0, "value": {"x": 0, "y": 0.707, "z": 0, "w": 0.707}},
                ]
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(d2["track_index"], 1);

    // Read and verify both tracks
    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    let tracks = read["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0]["type"], "position_3d");
    assert_eq!(tracks[1]["type"], "rotation_3d");
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_remove_track_by_index() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_rm_idx.tres";
    f.run(
        "animation_create",
        json!({
            "resource_path": path,
            "length": 1.0,
        }),
    )
    .unwrap()
    .unwrap_data();

    // Add two tracks
    f.run(
        "animation_add_track",
        json!({
            "resource_path": path,
            "track_type": "value",
            "node_path": "Sprite:position",
            "keyframes": [{"time": 0.0, "value": {"x": 0, "y": 0}}],
        }),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "animation_add_track",
        json!({
            "resource_path": path,
            "track_type": "value",
            "node_path": "Sprite:scale",
            "keyframes": [{"time": 0.0, "value": {"x": 1, "y": 1}}],
        }),
    )
    .unwrap()
    .unwrap_data();

    // Remove first track by index
    let data = f
        .run(
            "animation_remove_track",
            json!({
                "resource_path": path,
                "track_index": 0,
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["tracks_removed"], 1);

    // Verify only scale track remains
    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    let tracks = read["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0]["node_path"], "Sprite:scale");
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_remove_track_by_node_path() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_rm_path.tres";
    f.run(
        "animation_create",
        json!({
            "resource_path": path,
            "length": 1.0,
        }),
    )
    .unwrap()
    .unwrap_data();

    // Add two tracks on same node path, one on different
    f.run(
        "animation_add_track",
        json!({
            "resource_path": path,
            "track_type": "position_3d",
            "node_path": "Enemy",
            "keyframes": [{"time": 0.0, "value": {"x": 0, "y": 0, "z": 0}}],
        }),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "animation_add_track",
        json!({
            "resource_path": path,
            "track_type": "rotation_3d",
            "node_path": "Enemy",
            "keyframes": [{"time": 0.0, "value": {"x": 0, "y": 0, "z": 0, "w": 1}}],
        }),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "animation_add_track",
        json!({
            "resource_path": path,
            "track_type": "position_3d",
            "node_path": "Player",
            "keyframes": [{"time": 0.0, "value": {"x": 0, "y": 0, "z": 0}}],
        }),
    )
    .unwrap()
    .unwrap_data();

    // Remove all Enemy tracks by path
    let data = f
        .run(
            "animation_remove_track",
            json!({
                "resource_path": path,
                "node_path": "Enemy",
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["tracks_removed"], 2);

    // Verify only Player track remains
    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    let tracks = read["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0]["node_path"], "Player");
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_remove_track_out_of_range() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_rm_oor.tres";
    f.run(
        "animation_create",
        json!({
            "resource_path": path,
            "length": 1.0,
        }),
    )
    .unwrap()
    .unwrap_data();

    let err = f
        .run(
            "animation_remove_track",
            json!({
                "resource_path": path,
                "track_index": 5,
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("out of range"));
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_remove_track_no_match() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_rm_nomatch.tres";
    f.run(
        "animation_create",
        json!({
            "resource_path": path,
            "length": 1.0,
        }),
    )
    .unwrap()
    .unwrap_data();

    let err = f
        .run(
            "animation_remove_track",
            json!({
                "resource_path": path,
                "node_path": "Nonexistent",
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("No tracks found"));
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_add_track_rejects_invalid_type() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_bad_type.tres";
    f.run(
        "animation_create",
        json!({
            "resource_path": path,
            "length": 1.0,
        }),
    )
    .unwrap()
    .unwrap_data();

    let err = f
        .run(
            "animation_add_track",
            json!({
                "resource_path": path,
                "track_type": "audio",
                "node_path": "Player",
                "keyframes": [{"time": 0.0, "value": 1.0}],
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("Invalid track_type"));
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_add_value_track_discrete() {
    let f = DirectorFixture::new();
    let path = "tmp/test_anim_discrete.tres";
    f.run(
        "animation_create",
        json!({
            "resource_path": path,
            "length": 1.0,
        }),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "animation_add_track",
        json!({
            "resource_path": path,
            "track_type": "value",
            "node_path": "Sprite:frame",
            "update_mode": "discrete",
            "keyframes": [
                {"time": 0.0, "value": 0},
                {"time": 0.5, "value": 1},
                {"time": 1.0, "value": 2},
            ],
        }),
    )
    .unwrap()
    .unwrap_data();

    let read = f
        .run("animation_read", json!({"resource_path": path}))
        .unwrap()
        .unwrap_data();
    assert_eq!(read["tracks"][0]["update_mode"], "discrete");
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_read_not_found() {
    let f = DirectorFixture::new();
    let err = f
        .run(
            "animation_read",
            json!({
                "resource_path": "nonexistent.tres",
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("not found"));
}

#[test]
#[ignore = "requires Godot binary"]
fn animation_read_non_animation_resource() {
    let f = DirectorFixture::new();
    // Create a material (not an animation)
    f.run(
        "material_create",
        json!({
            "resource_path": "tmp/not_an_anim.tres",
            "material_type": "StandardMaterial3D",
        }),
    )
    .unwrap()
    .unwrap_data();

    let err = f
        .run(
            "animation_read",
            json!({
                "resource_path": "tmp/not_an_anim.tres",
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("not an Animation"));
}
