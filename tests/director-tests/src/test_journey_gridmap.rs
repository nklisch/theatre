use crate::harness::DirectorFixture;
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_gridmap_3d_level_building() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::journey_scene_path("gridmap_dungeon");

    // 1. Create Node3D "DungeonRoom"
    f.run(
        "scene_create",
        json!({
            "scene_path": scene,
            "root_type": "Node3D",
            "root_name": "DungeonRoom"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 2. Add GridMap "Floor" with mesh_library
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

    // 3. Add GridMap "Walls" with mesh_library
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "GridMap",
            "node_name": "Walls",
            "properties": {"mesh_library": "res://fixtures/test_mesh_library.tres"}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 4. Fill Floor with item 0 in a 5x5 grid (y=0)
    let floor_cells: Vec<serde_json::Value> = (0..5)
        .flat_map(|x| (0..5).map(move |z| json!({"position": [x, 0, z], "item": 0})))
        .collect();
    let floor_set = f
        .run(
            "gridmap_set_cells",
            json!({
                "scene_path": scene,
                "node_path": "Floor",
                "cells": floor_cells
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(
        floor_set["cells_set"], 25,
        "Floor should have 25 cells (5x5)"
    );

    // 5. Place wall cells around perimeter (item 1, various orientations)
    // North wall (z=-1), South wall (z=5), West wall (x=-1), East wall (x=5)
    let mut wall_cells: Vec<serde_json::Value> = Vec::new();
    for i in 0..5 {
        // North wall
        wall_cells.push(json!({"position": [i, 0, -1], "item": 1, "orientation": 0}));
        // South wall
        wall_cells.push(json!({"position": [i, 0, 5], "item": 1, "orientation": 0}));
        // West wall
        wall_cells.push(json!({"position": [-1, 0, i], "item": 1, "orientation": 16}));
        // East wall
        wall_cells.push(json!({"position": [5, 0, i], "item": 1, "orientation": 16}));
    }
    let wall_set = f
        .run(
            "gridmap_set_cells",
            json!({
                "scene_path": scene,
                "node_path": "Walls",
                "cells": wall_cells
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(wall_set["cells_set"], 20, "Should set 20 wall cells");

    // 6. gridmap_get_cells — Read Floor, verify 25 cells
    let floor_cells_read = f
        .run(
            "gridmap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Floor"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(floor_cells_read["cell_count"], 25);

    // 7. gridmap_get_cells — Read Walls with bounds filter for north wall
    let north_wall = f
        .run(
            "gridmap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Walls",
                "bounds": {"min": [0, 0, -1], "max": [4, 0, -1]}
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(
        north_wall["cell_count"], 5,
        "North wall should have 5 cells"
    );

    // Verify wall orientations are preserved
    let all_walls = f
        .run(
            "gridmap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Walls"
            }),
        )
        .unwrap()
        .unwrap_data();
    let wall_list = all_walls["cells"].as_array().unwrap();
    // Check that west wall cells have orientation 16
    let west_wall_cell = wall_list
        .iter()
        .find(|c| c["position"] == json!([-1, 0, 0]))
        .expect("Should find west wall cell at [-1,0,0]");
    assert_eq!(west_wall_cell["orientation"], 16);

    // 8. gridmap_clear — Clear a doorway in the north wall (x=2)
    let clear_data = f
        .run(
            "gridmap_clear",
            json!({
                "scene_path": scene,
                "node_path": "Walls",
                "bounds": {"min": [2, 0, -1], "max": [2, 0, -1]}
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(
        clear_data["cells_cleared"], 1,
        "Should clear exactly 1 doorway cell"
    );

    // 9. gridmap_get_cells — Verify wall count decreased by doorway cells
    let walls_after = f
        .run(
            "gridmap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Walls"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(
        walls_after["cell_count"], 19,
        "Should have 19 wall cells after doorway"
    );

    // 10. scene_read — verify both GridMap nodes in tree
    let floor_node = f.read_node(&scene, "Floor");
    assert_eq!(floor_node["type"], "GridMap");

    let walls_node = f.read_node(&scene, "Walls");
    assert_eq!(walls_node["type"], "GridMap");
}
