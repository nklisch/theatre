use crate::harness::{DirectorFixture, OperationResultExt};
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_tilemap_level_design_workflow() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::journey_scene_path("tilemap_level");

    // 1. Create Node2D "Level"
    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D", "root_name": "Level"}),
    )
    .unwrap()
    .unwrap_data();

    // 2. Add TileMapLayer "Ground" with tileset
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "TileMapLayer",
            "node_name": "Ground",
            "properties": {"tile_set": "res://fixtures/test_tileset.tres"}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 3. Add TileMapLayer "Hazards" with tileset
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "TileMapLayer",
            "node_name": "Hazards",
            "properties": {"tile_set": "res://fixtures/test_tileset.tres"}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 4. Fill Ground with floor tiles (row of 20 cells at y=0)
    let floor_cells: Vec<serde_json::Value> = (0..20)
        .map(|x| json!({"coords": [x, 0], "source_id": 0, "atlas_coords": [0, 0]}))
        .collect();
    let floor_data = f
        .run(
            "tilemap_set_cells",
            json!({
                "scene_path": scene,
                "node_path": "Ground",
                "cells": floor_cells
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(floor_data["cells_set"], 20);

    // 5. Add platforms at scattered heights on Ground
    let platform_cells = vec![
        json!({"coords": [3, -3], "source_id": 0, "atlas_coords": [0, 0]}),
        json!({"coords": [4, -3], "source_id": 0, "atlas_coords": [0, 0]}),
        json!({"coords": [5, -3], "source_id": 0, "atlas_coords": [0, 0]}),
        json!({"coords": [12, -5], "source_id": 0, "atlas_coords": [0, 0]}),
        json!({"coords": [13, -5], "source_id": 0, "atlas_coords": [0, 0]}),
    ];
    f.run(
        "tilemap_set_cells",
        json!({
            "scene_path": scene,
            "node_path": "Ground",
            "cells": platform_cells
        }),
    )
    .unwrap()
    .unwrap_data();

    // 6. Add hazard cells on Hazards layer (spikes/lava)
    let hazard_cells = vec![
        json!({"coords": [7, 0], "source_id": 0, "atlas_coords": [1, 0]}),
        json!({"coords": [8, 0], "source_id": 0, "atlas_coords": [1, 0]}),
        json!({"coords": [9, 0], "source_id": 0, "atlas_coords": [1, 0]}),
    ];
    f.run(
        "tilemap_set_cells",
        json!({
            "scene_path": scene,
            "node_path": "Hazards",
            "cells": hazard_cells
        }),
    )
    .unwrap()
    .unwrap_data();

    // 7. tilemap_get_cells — Read Ground cells, verify count = 20 + 5 platforms = 25
    let ground_cells = f
        .run(
            "tilemap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Ground"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(
        ground_cells["cell_count"], 25,
        "Expected 20 floor + 5 platform cells"
    );

    // 8. tilemap_get_cells — Read Ground with region filter for just platforms area
    let platform_region = f
        .run(
            "tilemap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Ground",
                "region": {"position": [3, -5], "size": [11, 3]}
            }),
        )
        .unwrap()
        .unwrap_data();
    assert!(
        platform_region["cell_count"].as_u64().unwrap() >= 3,
        "Region filter should return platform cells"
    );

    // 9. tilemap_clear — Clear a region from Ground (create a gap in the floor at x=8..10, y=0)
    let clear_data = f
        .run(
            "tilemap_clear",
            json!({
                "scene_path": scene,
                "node_path": "Ground",
                "region": {"position": [8, 0], "size": [3, 1]}
            }),
        )
        .unwrap()
        .unwrap_data();
    assert!(
        clear_data["cells_cleared"].as_u64().unwrap() >= 1,
        "Should have cleared cells"
    );

    // 10. tilemap_get_cells — Verify gap exists (cell count reduced)
    let ground_after = f
        .run(
            "tilemap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Ground"
            }),
        )
        .unwrap()
        .unwrap_data();
    let after_count = ground_after["cell_count"].as_u64().unwrap();
    assert!(
        after_count < 25,
        "Cell count should be reduced after clearing"
    );

    // 11. tilemap_get_cells — Read Hazards layer, verify independent from Ground
    let hazard_cells_read = f
        .run(
            "tilemap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Hazards"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(
        hazard_cells_read["cell_count"], 3,
        "Hazards layer should still have 3 cells"
    );

    // 12. scene_read — verify both TileMapLayer nodes exist in tree
    let ground_node = f.read_node(&scene, "Ground");
    assert_eq!(ground_node["type"], "TileMapLayer");

    let hazards_node = f.read_node(&scene, "Hazards");
    assert_eq!(hazards_node["type"], "TileMapLayer");
}
