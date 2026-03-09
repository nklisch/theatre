use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A single tilemap cell to set.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct TileMapCell {
    /// Cell coordinates as [x, y] in the tilemap grid.
    pub coords: [i32; 2],

    /// TileSet source ID (index of the atlas source in the TileSet).
    pub source_id: i32,

    /// Atlas coordinates within the source as [x, y].
    pub atlas_coords: [i32; 2],

    /// Alternative tile index. Default: 0.
    #[serde(default)]
    pub alternative_tile: Option<i32>,
}

/// Region specified as position + size in cell coordinates, matching Godot's Rect2i.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CellRegion {
    /// Top-left corner as [x, y] in cell coordinates.
    pub position: [i32; 2],

    /// Size as [width, height] in cells.
    pub size: [i32; 2],
}

/// Parameters for `tilemap_set_cells`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct TileMapSetCellsParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Scene file containing the TileMapLayer (relative to project).
    pub scene_path: String,

    /// Path to the TileMapLayer node within the scene tree.
    pub node_path: String,

    /// Cells to set. Each cell specifies coords, source_id, and atlas_coords.
    pub cells: Vec<TileMapCell>,
}

/// Parameters for `tilemap_get_cells`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct TileMapGetCellsParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Scene file containing the TileMapLayer (relative to project).
    pub scene_path: String,

    /// Path to the TileMapLayer node within the scene tree.
    pub node_path: String,

    /// Optional region to filter cells. Only cells within this rectangle
    /// are returned. Omit to get all used cells.
    #[serde(default)]
    pub region: Option<CellRegion>,

    /// Optional filter: only return cells from this TileSet source.
    #[serde(default)]
    pub source_id: Option<i32>,
}

/// Parameters for `tilemap_clear`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct TileMapClearParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Scene file containing the TileMapLayer (relative to project).
    pub scene_path: String,

    /// Path to the TileMapLayer node within the scene tree.
    pub node_path: String,

    /// Optional region to clear. Only cells within this rectangle are erased.
    /// Omit to clear all cells.
    #[serde(default)]
    pub region: Option<CellRegion>,
}
