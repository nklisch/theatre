use serde_json::json;

use crate::harness::DirectorFixture;

#[test]
#[ignore = "requires Godot binary"]
fn tilemap_set_cells_and_read_back() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("tilemap_set");

    // Create scene with TileMapLayer
    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "TileMapLayer",
            "node_name": "Ground",
            "properties": {
                "tile_set": "res://fixtures/test_tileset.tres"
            }
        }),
    )
    .unwrap()
    .unwrap_data();

    // Set cells
    let data = f
        .run(
            "tilemap_set_cells",
            json!({
                "scene_path": scene,
                "node_path": "Ground",
                "cells": [
                    {"coords": [0, 0], "source_id": 0, "atlas_coords": [0, 0]},
                    {"coords": [1, 0], "source_id": 0, "atlas_coords": [1, 0]},
                    {"coords": [0, 1], "source_id": 0, "atlas_coords": [0, 0]}
                ]
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["cells_set"], 3);

    // Read back
    let cells = f
        .run(
            "tilemap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Ground"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(cells["cell_count"], 3);
    assert_eq!(cells["cells"].as_array().unwrap().len(), 3);
    // used_rect should be present
    assert!(cells["used_rect"]["position"].is_array());
    assert!(cells["used_rect"]["size"].is_array());
}

#[test]
#[ignore = "requires Godot binary"]
fn tilemap_get_cells_with_region_filter() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("tilemap_region");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

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

    // Set cells scattered across the map
    f.run(
        "tilemap_set_cells",
        json!({
            "scene_path": scene,
            "node_path": "Ground",
            "cells": [
                {"coords": [0, 0], "source_id": 0, "atlas_coords": [0, 0]},
                {"coords": [5, 5], "source_id": 0, "atlas_coords": [0, 0]},
                {"coords": [10, 10], "source_id": 0, "atlas_coords": [0, 0]}
            ]
        }),
    )
    .unwrap()
    .unwrap_data();

    // Get cells in a region that includes only (0,0) and (5,5)
    let cells = f
        .run(
            "tilemap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Ground",
                "region": {"position": [0, 0], "size": [6, 6]}
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(cells["cell_count"], 2);
}

#[test]
#[ignore = "requires Godot binary"]
fn tilemap_clear_all() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("tilemap_clear_all");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

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

    f.run(
        "tilemap_set_cells",
        json!({
            "scene_path": scene,
            "node_path": "Ground",
            "cells": [
                {"coords": [0, 0], "source_id": 0, "atlas_coords": [0, 0]},
                {"coords": [1, 1], "source_id": 0, "atlas_coords": [0, 0]}
            ]
        }),
    )
    .unwrap()
    .unwrap_data();

    // Clear all
    let data = f
        .run(
            "tilemap_clear",
            json!({
                "scene_path": scene,
                "node_path": "Ground"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["cells_cleared"], 2);

    // Verify empty
    let cells = f
        .run(
            "tilemap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Ground"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(cells["cell_count"], 0);
}

#[test]
#[ignore = "requires Godot binary"]
fn tilemap_clear_region() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("tilemap_clear_region");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

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

    f.run(
        "tilemap_set_cells",
        json!({
            "scene_path": scene,
            "node_path": "Ground",
            "cells": [
                {"coords": [0, 0], "source_id": 0, "atlas_coords": [0, 0]},
                {"coords": [5, 5], "source_id": 0, "atlas_coords": [0, 0]},
                {"coords": [10, 10], "source_id": 0, "atlas_coords": [0, 0]}
            ]
        }),
    )
    .unwrap()
    .unwrap_data();

    // Clear only the first cell's region
    let data = f
        .run(
            "tilemap_clear",
            json!({
                "scene_path": scene,
                "node_path": "Ground",
                "region": {"position": [0, 0], "size": [1, 1]}
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(data["cells_cleared"], 1);

    // Verify 2 cells remain
    let cells = f
        .run(
            "tilemap_get_cells",
            json!({
                "scene_path": scene,
                "node_path": "Ground"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(cells["cell_count"], 2);
}

#[test]
#[ignore = "requires Godot binary"]
fn tilemap_rejects_non_tilemap_layer() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("tilemap_wrong_type");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "Sprite2D",
            "node_name": "NotATileMap"
        }),
    )
    .unwrap()
    .unwrap_data();

    let err = f
        .run(
            "tilemap_set_cells",
            json!({
                "scene_path": scene,
                "node_path": "NotATileMap",
                "cells": [{"coords": [0, 0], "source_id": 0, "atlas_coords": [0, 0]}]
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("expected TileMapLayer"));
}

#[test]
#[ignore = "requires Godot binary"]
fn tilemap_rejects_no_tileset() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("tilemap_no_tileset");

    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Node2D"}),
    )
    .unwrap()
    .unwrap_data();

    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "TileMapLayer",
            "node_name": "NoTileSet"
        }),
    )
    .unwrap()
    .unwrap_data();

    let err = f
        .run(
            "tilemap_set_cells",
            json!({
                "scene_path": scene,
                "node_path": "NoTileSet",
                "cells": [{"coords": [0, 0], "source_id": 0, "atlas_coords": [0, 0]}]
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(err.contains("no TileSet assigned"));
}
