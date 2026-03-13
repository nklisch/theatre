use serde_json::json;

use crate::harness::{DirectorFixture, OperationResultExt};

#[test]
#[ignore = "requires Godot binary"]
fn gridmap_set_cells_and_read_back() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("gridmap_set");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node3D"}),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "GridMap",
            "node_name": "Floor",
            "properties": {
                "mesh_library": "res://fixtures/test_mesh_library.tres"
            }
        }),
    )
    .unwrap()
    .unwrap_data();

    // Set cells
    let data = f
        .run(
            "gridmap_set_cells",
            json!({
                "scene_path": scene,
                "node_path": "Floor",
                "cells": [
                    {"position": [0, 0, 0], "item": 0},
                    {"position": [1, 0, 0], "item": 0},
                    {"position": [0, 0, 1], "item": 1, "orientation": 10}
                ]
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["cells_set"], 3);

    // Read back
    let cells = f
        .run(
            "gridmap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Floor"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(cells["cell_count"], 3);

    // Verify orientation was preserved
    let cell_list = cells["cells"].as_array().unwrap();
    let oriented_cell = cell_list
        .iter()
        .find(|c| c["position"] == json!([0, 0, 1]))
        .expect("should find cell at [0,0,1]");
    assert_eq!(oriented_cell["item"], 1);
    assert_eq!(oriented_cell["orientation"], 10);
}

#[test]
#[ignore = "requires Godot binary"]
fn gridmap_get_cells_with_bounds_filter() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("gridmap_bounds");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node3D"}),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "GridMap",
            "node_name": "Floor",
            "properties": {"mesh_library": "res://fixtures/test_mesh_library.tres"}
        }),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "gridmap_set_cells",
        json!({
            "scene_path": scene,
            "node_path": "Floor",
            "cells": [
                {"position": [0, 0, 0], "item": 0},
                {"position": [5, 0, 5], "item": 0},
                {"position": [10, 0, 10], "item": 0}
            ]
        }),
    )
    .unwrap()
    .unwrap_data();

    // Get cells within bounds that only include first two
    let cells = f
        .run(
            "gridmap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Floor",
                "bounds": {"min": [0, 0, 0], "max": [5, 0, 5]}
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(cells["cell_count"], 2);
}

#[test]
#[ignore = "requires Godot binary"]
fn gridmap_clear_all() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("gridmap_clear_all");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node3D"}),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "GridMap",
            "node_name": "Floor",
            "properties": {"mesh_library": "res://fixtures/test_mesh_library.tres"}
        }),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "gridmap_set_cells",
        json!({
            "scene_path": scene,
            "node_path": "Floor",
            "cells": [
                {"position": [0, 0, 0], "item": 0},
                {"position": [1, 0, 0], "item": 0}
            ]
        }),
    )
    .unwrap()
    .unwrap_data();

    let data = f
        .run(
            "gridmap_clear",
            json!({
                "scene_path": scene,
                "node_path": "Floor"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["cells_cleared"], 2);

    // Verify empty
    let cells = f
        .run(
            "gridmap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Floor"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(cells["cell_count"], 0);
}

#[test]
#[ignore = "requires Godot binary"]
fn gridmap_rejects_non_gridmap() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("gridmap_wrong_type");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node3D"}),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "MeshInstance3D",
            "node_name": "NotAGridMap"
        }),
    )
    .unwrap()
    .unwrap_data();

    let err = f
        .run(
            "gridmap_set_cells",
            json!({
                "scene_path": scene,
                "node_path": "NotAGridMap",
                "cells": [{"position": [0, 0, 0], "item": 0}]
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("expected GridMap"));
}

#[test]
#[ignore = "requires Godot binary"]
fn gridmap_rejects_no_mesh_library() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("gridmap_no_lib");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node3D"}),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "GridMap",
            "node_name": "NoLib"
        }),
    )
    .unwrap()
    .unwrap_data();

    let err = f
        .run(
            "gridmap_set_cells",
            json!({
                "scene_path": scene,
                "node_path": "NoLib",
                "cells": [{"position": [0, 0, 0], "item": 0}]
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("no MeshLibrary assigned"));
}
