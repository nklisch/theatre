use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A single GridMap cell to set.
#[serde_with::skip_serializing_none]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GridMapCell {
    /// Cell position as [x, y, z] in the grid.
    pub position: [i32; 3],

    /// MeshLibrary item index.
    pub item: i32,

    /// Orientation index (0-23 orthogonal rotations). Default: 0.
    #[serde(default)]
    pub orientation: Option<i32>,
}

/// Axis-aligned bounding box in grid coordinates.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GridBounds {
    /// Minimum corner as [x, y, z] (inclusive).
    pub min: [i32; 3],

    /// Maximum corner as [x, y, z] (inclusive).
    pub max: [i32; 3],
}

/// Parameters for `gridmap_set_cells`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GridMapSetCellsParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Scene file containing the GridMap (relative to project).
    pub scene_path: String,

    /// Path to the GridMap node within the scene tree.
    pub node_path: String,

    /// Cells to set. Each cell specifies position and mesh library item index.
    pub cells: Vec<GridMapCell>,
}

/// Parameters for `gridmap_get_cells`.
#[serde_with::skip_serializing_none]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GridMapGetCellsParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Scene file containing the GridMap (relative to project).
    pub scene_path: String,

    /// Path to the GridMap node within the scene tree.
    pub node_path: String,

    /// Optional bounding box to filter cells. Only cells within these bounds
    /// are returned. Omit to get all used cells.
    #[serde(default)]
    pub bounds: Option<GridBounds>,

    /// Optional filter: only return cells with this MeshLibrary item.
    #[serde(default)]
    pub item: Option<i32>,
}

/// Parameters for `gridmap_clear`.
#[serde_with::skip_serializing_none]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct GridMapClearParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Scene file containing the GridMap (relative to project).
    pub scene_path: String,

    /// Path to the GridMap node within the scene tree.
    pub node_path: String,

    /// Optional bounding box to clear. Only cells within these bounds are
    /// removed. Omit to clear all cells.
    #[serde(default)]
    pub bounds: Option<GridBounds>,
}
