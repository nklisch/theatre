---
description: "TileMap and GridMap operations — set tiles, configure tile sets, and build levels programmatically with Director."
---

<script setup>
import { data } from '../.vitepress/data/tools.data'

const tilemap_set_cells = data.params['tilemap_set_cells'] ?? []
const tilemap_get_cells = data.params['tilemap_get_cells'] ?? []
const tilemap_clear = data.params['tilemap_clear'] ?? []
const gridmap_set_cells = data.params['gridmap_set_cells'] ?? []
const gridmap_get_cells = data.params['gridmap_get_cells'] ?? []
const gridmap_clear = data.params['gridmap_clear'] ?? []

const messages0 = [
  { role: 'human', text: `Build a simple platformer level in the TileMap. A floor at row 0 from columns 0-30, and three floating platforms.` },
  { role: 'agent', text: `Floor placed (30 tiles). Adding the three platforms.` },
  { role: 'agent', text: `Done. Floor (columns 0-30, row 0) and three platforms at rows -3, -6, and -9 placed. Total: 38 tiles.` },
]
</script>

# TileMap & GridMap

Modify tile-based layouts for 2D and 3D worlds.

## TileMap (2D)

`TileMap` is Godot's 2D tile system. Director can set, get, and clear tiles using tile coordinates (column, row).

### `tilemap_set_cells`

Set one or more specific tiles.

```json
{
  "op": "tilemap_set_cells",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "node_path": "World/TileMap",
  "cells": [
    { "coords": [0, 0], "source_id": 0, "atlas_coords": [0, 0] },
    { "coords": [1, 0], "source_id": 0, "atlas_coords": [0, 0] }
  ]
}
```

<ParamTable :params="tilemap_set_cells" />

To erase a tile, set `atlas_coords` to `[-1, -1]`.

To set an alternative tile variant, provide `alternative_tile`:

```json
{ "coords": [5, 0], "source_id": 0, "atlas_coords": [2, 0], "alternative_tile": 1 }
```

**Response:**
```json
{
  "op": "tilemap_set_cells",
  "cells_set": 4,
  "result": "ok"
}
```

### `tilemap_get_cells`

Read tile data from a region.

```json
{
  "op": "tilemap_get_cells",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "node_path": "World/TileMap",
  "region": { "position": [0, -5], "size": [20, 10] }
}
```

<ParamTable :params="tilemap_get_cells" />

**Response:**
```json
{
  "op": "tilemap_get_cells",
  "region": { "position": [0, -5], "size": [20, 10] },
  "cells": [
    { "coords": [0, 0], "source_id": 0, "atlas_coords": [0, 0], "alternative_tile": 0 }
  ]
}
```

Only non-empty tiles are returned.

### `tilemap_clear`

Remove tiles from the TileMap, optionally limited to a region.

```json
{
  "op": "tilemap_clear",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "node_path": "World/TileMap"
}
```

To clear only a region:

```json
{
  "op": "tilemap_clear",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/level_01.tscn",
  "node_path": "World/TileMap",
  "region": { "position": [0, -5], "size": [20, 10] }
}
```

<ParamTable :params="tilemap_clear" />

## GridMap (3D)

`GridMap` is Godot's 3D tile system, using a 3D integer grid. Director can set individual cells or regions.

### `gridmap_set_cells`

Set one or more cells in a GridMap.

```json
{
  "op": "gridmap_set_cells",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/dungeon.tscn",
  "node_path": "World/GridMap",
  "cells": [
    { "position": [0, 0, 0], "item": 0, "orientation": 0 },
    { "position": [1, 0, 0], "item": 0, "orientation": 0 }
  ]
}
```

<ParamTable :params="gridmap_set_cells" />

**Orientation values**: Godot's GridMap uses integer orientations 0-23 for each of the 24 possible orthogonal rotations. Common values: 0=default, 10=rotated 90° around Y, 16=rotated 180° around Y, 22=rotated 270° around Y.

**Response:**
```json
{
  "op": "gridmap_set_cells",
  "cells_set": 5,
  "result": "ok"
}
```

### `gridmap_get_cells`

Read cells in a bounding region. Optionally filter by item index.

```json
{
  "op": "gridmap_get_cells",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/dungeon.tscn",
  "node_path": "World/GridMap",
  "bounds": { "min": [-5, 0, -5], "max": [5, 2, 5] }
}
```

<ParamTable :params="gridmap_get_cells" />

### `gridmap_clear`

Remove all cells from the GridMap, optionally limited to a bounding region.

```json
{
  "op": "gridmap_clear",
  "project_path": "/home/user/my-game",
  "scene_path": "scenes/dungeon.tscn",
  "node_path": "World/GridMap"
}
```

<ParamTable :params="gridmap_clear" />

## Example conversation: Building a platformer level

<AgentConversation :messages="messages0" />

## Tips

**Know your atlas coordinates first.** Open the TileSet resource in the Godot editor to find the `source_id` and `atlas_coords` for each tile type before calling Director.

**Use `tilemap_set_cells` with all cells in one call.** Batching all tile placements into a single `tilemap_set_cells` call is one round-trip regardless of how many tiles are set.

**Use `tilemap_get_cells` to audit existing levels.** Before modifying a level, read the existing tile layout to understand what is there.

**GridMap orientations**: When building 3D levels with GridMap, use `spatial_snapshot` after applying changes to verify that walls and floors are facing the right direction.
