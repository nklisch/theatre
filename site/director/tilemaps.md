<script setup>
const messages0 = [
  { role: 'human', text: `Build a simple platformer level in the TileMap. A floor at row 0 from columns 0-30, and three floating platforms.` },
  { role: 'agent', text: `Floor placed (30 tiles). Adding the three platforms.` },
  { role: 'agent', text: `Done. Floor (columns 0-30, row 0) and three platforms at rows -3, -6, and -9 placed. Total: 38 tiles.` },
]
</script>

# TileMap & GridMap

Modify tile-based layouts for 2D and 3D worlds.

## TileMap (2D)

`TileMap` is Godot's 2D tile system. Director can set, get, and fill tiles using tile coordinates (column, row).

### `tilemap_set`

Set one or more specific tiles.

```json
{
  "op": "tilemap_set",
  "project_path": "/home/user/my-game",
  "scene": "scenes/level_01.tscn",
  "node": "World/TileMap",
  "tiles": [
    { "position": [0, 0], "source_id": 0, "atlas_coords": [0, 0], "layer": 0 },
    { "position": [1, 0], "source_id": 0, "atlas_coords": [0, 0], "layer": 0 },
    { "position": [2, 0], "source_id": 0, "atlas_coords": [0, 0], "layer": 0 },
    { "position": [0, -1], "source_id": 0, "atlas_coords": [1, 0], "layer": 0 }
  ]
}
```

| Parameter | Type | Description |
|---|---|---|
| `node` | `string` | Path to the TileMap node |
| `tiles` | `array` | List of tile placements |
| `tiles[].position` | `[col, row]` | Tile grid coordinates |
| `tiles[].source_id` | `integer` | TileSet source ID (which tileset to use) |
| `tiles[].atlas_coords` | `[x, y]` | Which tile in the atlas (0-based) |
| `tiles[].layer` | `integer` | TileMap layer index (default: 0) |

To erase a tile, set `atlas_coords` to `[-1, -1]`:
```json
{ "position": [5, 3], "source_id": 0, "atlas_coords": [-1, -1], "layer": 0 }
```

**Response:**
```json
{
  "op": "tilemap_set",
  "tiles_set": 4,
  "result": "ok"
}
```

### `tilemap_get`

Read tile data from a region.

```json
{
  "op": "tilemap_get",
  "project_path": "/home/user/my-game",
  "scene": "scenes/level_01.tscn",
  "node": "World/TileMap",
  "region": { "min": [0, -5], "max": [20, 5] },
  "layer": 0
}
```

**Response:**
```json
{
  "op": "tilemap_get",
  "region": { "min": [0, -5], "max": [20, 5] },
  "tiles": [
    { "position": [0, 0], "source_id": 0, "atlas_coords": [0, 0] },
    { "position": [1, 0], "source_id": 0, "atlas_coords": [0, 0] },
    { "position": [3, -1], "source_id": 0, "atlas_coords": [2, 1] }
  ]
}
```

Only non-empty tiles are returned.

### `tilemap_fill`

Fill a rectangular region with one tile type. Efficient for placing large floor/wall sections.

```json
{
  "op": "tilemap_fill",
  "project_path": "/home/user/my-game",
  "scene": "scenes/level_01.tscn",
  "node": "World/TileMap",
  "region": { "min": [0, 0], "max": [20, 1] },
  "source_id": 0,
  "atlas_coords": [0, 0],
  "layer": 0
}
```

This fills 20 tiles at row 0 (a floor spanning columns 0-19).

**Response:**
```json
{
  "op": "tilemap_fill",
  "tiles_set": 20,
  "result": "ok"
}
```

To erase a region:
```json
{
  "op": "tilemap_fill",
  "region": { "min": [5, -3], "max": [10, -1] },
  "source_id": 0,
  "atlas_coords": [-1, -1]
}
```

### `tilemap_clear`

Remove all tiles from a layer.

```json
{
  "op": "tilemap_clear",
  "project_path": "/home/user/my-game",
  "scene": "scenes/level_01.tscn",
  "node": "World/TileMap",
  "layer": 0
}
```

## GridMap (3D)

`GridMap` is Godot's 3D tile system, using a 3D integer grid. Director can set individual cells or regions.

### `gridmap_set`

Set one or more cells in a GridMap.

```json
{
  "op": "gridmap_set",
  "project_path": "/home/user/my-game",
  "scene": "scenes/dungeon.tscn",
  "node": "World/GridMap",
  "cells": [
    { "position": [0, 0, 0], "item": 0, "orientation": 0 },
    { "position": [1, 0, 0], "item": 0, "orientation": 0 },
    { "position": [2, 0, 0], "item": 0, "orientation": 0 },
    { "position": [0, 1, 0], "item": 2, "orientation": 0 },
    { "position": [2, 1, 0], "item": 2, "orientation": 0 }
  ]
}
```

| Parameter | Type | Description |
|---|---|---|
| `cells[].position` | `[x, y, z]` | 3D grid coordinates (integer) |
| `cells[].item` | `integer` | MeshLibrary item index (-1 to erase) |
| `cells[].orientation` | `integer` | Rotation (0-23, mapping to 24 orientations) |

**Orientation values**: Godot's GridMap uses integer orientations 0-23 for each of the 24 possible orthogonal rotations. Common values: 0=default, 10=rotated 90° around Y, 16=rotated 180° around Y, 22=rotated 270° around Y.

**Response:**
```json
{
  "op": "gridmap_set",
  "cells_set": 5,
  "result": "ok"
}
```

### `gridmap_get`

Read cells in a region.

```json
{
  "op": "gridmap_get",
  "project_path": "/home/user/my-game",
  "scene": "scenes/dungeon.tscn",
  "node": "World/GridMap",
  "region": { "min": [-5, 0, -5], "max": [5, 2, 5] }
}
```

**Response:**
```json
{
  "op": "gridmap_get",
  "cells": [
    { "position": [0, 0, 0], "item": 0, "orientation": 0 },
    { "position": [1, 0, 0], "item": 0, "orientation": 0 }
  ]
}
```

### `gridmap_fill`

Fill a 3D region with one item.

```json
{
  "op": "gridmap_fill",
  "project_path": "/home/user/my-game",
  "scene": "scenes/dungeon.tscn",
  "node": "World/GridMap",
  "region": { "min": [0, 0, 0], "max": [10, 1, 10] },
  "item": 0,
  "orientation": 0
}
```

This fills a 10×1×10 floor at y=0.

## Example conversation: Building a platformer level

<AgentConversation :messages="messages0" />

## Tips

**Know your atlas coordinates first.** Open the TileSet resource in the Godot editor to find the `source_id` and `atlas_coords` for each tile type before calling Director.

**Use `tilemap_fill` for large regions.** It is a single operation regardless of how many tiles are filled. 100 tiles with `tilemap_fill` is one round-trip; 100 tiles with `tilemap_set` is also one round-trip (batched), but `tilemap_fill` is simpler for uniform regions.

**Use `tilemap_get` to audit existing levels.** Before modifying a level, read the existing tile layout to understand what is there.

**GridMap orientations**: When building 3D levels with GridMap, use `spatial_snapshot` after applying changes to verify that walls and floors are facing the right direction.
