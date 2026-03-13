# Design: Director Phase 5 — TileMap & GridMap

## Overview

Phase 5 adds six tile/grid manipulation tools to Director:

- **TileMapLayer** (2D): `tilemap_set_cells`, `tilemap_get_cells`, `tilemap_clear`
- **GridMap** (3D): `gridmap_set_cells`, `gridmap_get_cells`, `gridmap_clear`

These tools operate on existing TileMapLayer/GridMap nodes in scenes. They
assume a TileSet (2D) or MeshLibrary (3D) is already assigned — TileSet/
MeshLibrary authoring is visual work done in the Godot editor, not Director's
domain.

**Scope boundary:** Director places tiles and reads tile state. It does not
create TileSet resources, define atlas sources, or configure MeshLibraries.
Agents can assign a TileSet/MeshLibrary to a node using `node_set_properties`.

**TileMapLayer only:** TileMap was deprecated in Godot 4.3 in favour of
TileMapLayer. These tools require TileMapLayer and will error on TileMap nodes.

**Region format:** Follows Godot's `Rect2i` pattern — `{position: [x, y],
size: [w, h]}` in cell coordinates. Omitting the region means "all cells".
For GridMap, uses axis-aligned box `{min: [x, y, z], max: [x, y, z]}` since
Godot has no Rect3i equivalent.

---

## Implementation Units

### Unit 1: GDScript TileMap Operations

**File**: `addons/director/ops/tilemap_ops.gd` (new file)

```gdscript
class_name TileMapOps


static func op_tilemap_set_cells(params: Dictionary) -> Dictionary:
    ## Set cells on a TileMapLayer node in a scene.
    ##
    ## Params:
    ##   scene_path: String — path to the .tscn file
    ##   node_path: String — path to the TileMapLayer node within the scene
    ##   cells: Array[Dictionary] — each cell: { coords: [x, y], source_id: int,
    ##       atlas_coords: [x, y], alternative_tile?: int (default 0) }
    ##
    ## Returns: { success, data: { cells_set: int, node_path: String } }

static func op_tilemap_get_cells(params: Dictionary) -> Dictionary:
    ## Get used cells from a TileMapLayer node in a scene.
    ##
    ## Params:
    ##   scene_path: String — path to the .tscn file
    ##   node_path: String — path to the TileMapLayer node within the scene
    ##   region?: Dictionary — { position: [x, y], size: [w, h] } in cell coords.
    ##       Omit for all used cells.
    ##   source_id?: int — filter to cells from this tile source only
    ##
    ## Returns: { success, data: { cells: Array[CellData], cell_count: int,
    ##     used_rect: { position: [x, y], size: [w, h] } } }
    ## CellData: { coords: [x, y], source_id: int, atlas_coords: [x, y],
    ##     alternative_tile: int }

static func op_tilemap_clear(params: Dictionary) -> Dictionary:
    ## Clear cells from a TileMapLayer node in a scene.
    ##
    ## Params:
    ##   scene_path: String — path to the .tscn file
    ##   node_path: String — path to the TileMapLayer node within the scene
    ##   region?: Dictionary — { position: [x, y], size: [w, h] } in cell coords.
    ##       Omit to clear all cells.
    ##
    ## Returns: { success, data: { cells_cleared: int, node_path: String } }


# Shared helpers

static func _load_scene_and_find_node(scene_path: String, node_path: String,
        operation: String) -> Dictionary:
    ## Load a scene, find a node, validate it's the expected type.
    ## Returns { success: true, root: Node, target: Node } or error dict.

static func _validate_tilemap_layer(node: Node, operation: String,
        context: Dictionary) -> Dictionary:
    ## Validate that a node is a TileMapLayer (not deprecated TileMap).
    ## Returns { success: true } or error dict.

static func _repack_scene(root: Node, full_path: String) -> Dictionary:
    ## Delegates to NodeOps._repack_and_save.

static func _error(message: String, operation: String,
        context: Dictionary) -> Dictionary:
    return {"success": false, "error": message, "operation": operation,
        "context": context}
```

**Full implementation for `op_tilemap_set_cells`:**

```gdscript
static func op_tilemap_set_cells(params: Dictionary) -> Dictionary:
    var scene_path: String = params.get("scene_path", "")
    var node_path: String = params.get("node_path", "")
    var cells = params.get("cells", [])

    if scene_path == "":
        return _error("scene_path is required", "tilemap_set_cells", params)
    if node_path == "":
        return _error("node_path is required", "tilemap_set_cells", params)
    if not cells is Array or cells.is_empty():
        return _error("cells must be a non-empty array", "tilemap_set_cells", params)

    var loaded = _load_scene_and_find_node(scene_path, node_path, "tilemap_set_cells")
    if not loaded.success:
        return loaded

    var root: Node = loaded.root
    var target: Node = loaded.target

    var valid = _validate_tilemap_layer(target, "tilemap_set_cells",
        {"scene_path": scene_path, "node_path": node_path})
    if not valid.success:
        root.free()
        return valid

    if target.tile_set == null:
        root.free()
        return _error("TileMapLayer has no TileSet assigned. Assign one via " +
            "node_set_properties before setting cells.",
            "tilemap_set_cells", {"node_path": node_path})

    var cells_set := 0
    for cell in cells:
        if not cell is Dictionary:
            root.free()
            return _error("Each cell must be a dictionary with coords, source_id, atlas_coords",
                "tilemap_set_cells", {"cell": cell})

        var coords_arr = cell.get("coords", null)
        if coords_arr == null or not coords_arr is Array or coords_arr.size() != 2:
            root.free()
            return _error("Cell coords must be [x, y] array",
                "tilemap_set_cells", {"cell": cell})

        var source_id: int = int(cell.get("source_id", 0))
        var atlas_arr = cell.get("atlas_coords", null)
        if atlas_arr == null or not atlas_arr is Array or atlas_arr.size() != 2:
            root.free()
            return _error("Cell atlas_coords must be [x, y] array",
                "tilemap_set_cells", {"cell": cell})

        var alt_tile: int = int(cell.get("alternative_tile", 0))

        var coords = Vector2i(int(coords_arr[0]), int(coords_arr[1]))
        var atlas_coords = Vector2i(int(atlas_arr[0]), int(atlas_arr[1]))

        target.set_cell(coords, source_id, atlas_coords, alt_tile)
        cells_set += 1

    var save_result = NodeOps._repack_and_save(root, "res://" + scene_path)
    root.free()
    if not save_result.success:
        return save_result

    return {"success": true, "data": {"cells_set": cells_set, "node_path": node_path}}
```

**Full implementation for `op_tilemap_get_cells`:**

```gdscript
static func op_tilemap_get_cells(params: Dictionary) -> Dictionary:
    var scene_path: String = params.get("scene_path", "")
    var node_path: String = params.get("node_path", "")
    var region = params.get("region", null)
    var filter_source_id = params.get("source_id", null)

    if scene_path == "":
        return _error("scene_path is required", "tilemap_get_cells", params)
    if node_path == "":
        return _error("node_path is required", "tilemap_get_cells", params)

    var loaded = _load_scene_and_find_node(scene_path, node_path, "tilemap_get_cells")
    if not loaded.success:
        return loaded

    var root: Node = loaded.root
    var target: Node = loaded.target

    var valid = _validate_tilemap_layer(target, "tilemap_get_cells",
        {"scene_path": scene_path, "node_path": node_path})
    if not valid.success:
        root.free()
        return valid

    # Get used cells — optionally filtered by source_id
    var used_cells: Array[Vector2i]
    if filter_source_id != null:
        used_cells = target.get_used_cells_by_id(int(filter_source_id))
    else:
        used_cells = target.get_used_cells()

    # Apply region filter if specified
    var region_rect: Rect2i
    var has_region := false
    if region is Dictionary:
        var pos = region.get("position", [0, 0])
        var sz = region.get("size", [0, 0])
        if pos is Array and pos.size() == 2 and sz is Array and sz.size() == 2:
            region_rect = Rect2i(
                int(pos[0]), int(pos[1]),
                int(sz[0]), int(sz[1])
            )
            has_region = true

    var cells: Array = []
    for coords in used_cells:
        if has_region and not region_rect.has_point(coords):
            continue
        var cell_data: Dictionary = {
            "coords": [coords.x, coords.y],
            "source_id": target.get_cell_source_id(coords),
            "atlas_coords": [
                target.get_cell_atlas_coords(coords).x,
                target.get_cell_atlas_coords(coords).y,
            ],
            "alternative_tile": target.get_cell_alternative_tile(coords),
        }
        cells.append(cell_data)

    # Get the used rect for context
    var used_rect = target.get_used_rect()
    var used_rect_data = {
        "position": [used_rect.position.x, used_rect.position.y],
        "size": [used_rect.size.x, used_rect.size.y],
    }

    root.free()

    return {"success": true, "data": {
        "cells": cells,
        "cell_count": cells.size(),
        "used_rect": used_rect_data,
    }}
```

**Full implementation for `op_tilemap_clear`:**

```gdscript
static func op_tilemap_clear(params: Dictionary) -> Dictionary:
    var scene_path: String = params.get("scene_path", "")
    var node_path: String = params.get("node_path", "")
    var region = params.get("region", null)

    if scene_path == "":
        return _error("scene_path is required", "tilemap_clear", params)
    if node_path == "":
        return _error("node_path is required", "tilemap_clear", params)

    var loaded = _load_scene_and_find_node(scene_path, node_path, "tilemap_clear")
    if not loaded.success:
        return loaded

    var root: Node = loaded.root
    var target: Node = loaded.target

    var valid = _validate_tilemap_layer(target, "tilemap_clear",
        {"scene_path": scene_path, "node_path": node_path})
    if not valid.success:
        root.free()
        return valid

    var cells_cleared := 0

    if region is Dictionary:
        # Clear only within region
        var pos = region.get("position", [0, 0])
        var sz = region.get("size", [0, 0])
        if pos is Array and pos.size() == 2 and sz is Array and sz.size() == 2:
            var region_rect = Rect2i(
                int(pos[0]), int(pos[1]),
                int(sz[0]), int(sz[1])
            )
            var used_cells = target.get_used_cells()
            for coords in used_cells:
                if region_rect.has_point(coords):
                    target.erase_cell(coords)
                    cells_cleared += 1
    else:
        # Clear all
        cells_cleared = target.get_used_cells().size()
        target.clear()

    var save_result = NodeOps._repack_and_save(root, "res://" + scene_path)
    root.free()
    if not save_result.success:
        return save_result

    return {"success": true, "data": {"cells_cleared": cells_cleared, "node_path": node_path}}
```

**Shared helper implementations:**

```gdscript
static func _load_scene_and_find_node(scene_path: String, node_path: String,
        operation: String) -> Dictionary:
    var full_path = "res://" + scene_path
    if not ResourceLoader.exists(full_path):
        return _error("Scene not found: " + scene_path, operation,
            {"scene_path": scene_path})

    var packed: PackedScene = load(full_path)
    var root = packed.instantiate()

    var target: Node
    if node_path == "." or node_path == "":
        target = root
    else:
        target = root.get_node_or_null(node_path)
    if target == null:
        root.free()
        return _error("Node not found: " + node_path, operation,
            {"scene_path": scene_path, "node_path": node_path})

    return {"success": true, "root": root, "target": target}


static func _validate_tilemap_layer(node: Node, operation: String,
        context: Dictionary) -> Dictionary:
    if node is TileMapLayer:
        return {"success": true}
    if node.get_class() == "TileMap":
        return _error("TileMap is deprecated in Godot 4.3+. Use TileMapLayer instead. " +
            "Convert your TileMap to TileMapLayer nodes in the Godot editor.",
            operation, context)
    return _error("Node is " + node.get_class() + ", expected TileMapLayer",
        operation, context)
```

**Implementation Notes:**
- `set_cell(coords, source_id, atlas_coords, alternative_tile)` is the
  TileMapLayer API for placing tiles. All four args are needed.
- `erase_cell(coords)` removes a single cell. `clear()` removes all.
- `get_used_cells()` returns `Array[Vector2i]`. Each cell's data is queried
  individually via `get_cell_source_id()`, `get_cell_atlas_coords()`,
  `get_cell_alternative_tile()`.
- Coordinates are returned as `[x, y]` arrays (not `{x, y}` objects) to match
  the input format and because these are integer grid indices, not spatial
  vectors. This follows Godot's convention where Vector2i cell coords are
  index pairs rather than continuous positions.
- `get_used_rect()` returns the bounding `Rect2i` of all used cells — included
  in `tilemap_get_cells` response for agent context.
- The `source_id` filter on `get_cells` uses Godot's built-in
  `get_used_cells_by_id(source_id)` which is more efficient than post-filtering.

**Acceptance Criteria:**
- [ ] `op_tilemap_set_cells` places tiles and saves the scene
- [ ] `op_tilemap_set_cells` validates TileSet is assigned before placing
- [ ] `op_tilemap_set_cells` validates each cell has required fields
- [ ] `op_tilemap_get_cells` returns all cells with source_id, atlas_coords, alternative_tile
- [ ] `op_tilemap_get_cells` filters by region when provided
- [ ] `op_tilemap_get_cells` filters by source_id when provided
- [ ] `op_tilemap_get_cells` includes used_rect in response
- [ ] `op_tilemap_clear` clears all cells when no region given
- [ ] `op_tilemap_clear` clears only cells within region when given
- [ ] All three ops validate the node is TileMapLayer (not TileMap)
- [ ] All three ops return structured errors on every failure path

---

### Unit 2: GDScript GridMap Operations

**File**: `addons/director/ops/gridmap_ops.gd` (new file)

```gdscript
class_name GridMapOps


static func op_gridmap_set_cells(params: Dictionary) -> Dictionary:
    ## Set cells in a GridMap node in a scene.
    ##
    ## Params:
    ##   scene_path: String — path to the .tscn file
    ##   node_path: String — path to the GridMap node within the scene
    ##   cells: Array[Dictionary] — each cell: { position: [x, y, z], item: int,
    ##       orientation?: int (default 0) }
    ##
    ## Returns: { success, data: { cells_set: int, node_path: String } }

static func op_gridmap_get_cells(params: Dictionary) -> Dictionary:
    ## Get used cells from a GridMap node in a scene.
    ##
    ## Params:
    ##   scene_path: String — path to the .tscn file
    ##   node_path: String — path to the GridMap node within the scene
    ##   bounds?: Dictionary — { min: [x, y, z], max: [x, y, z] }.
    ##       Omit for all used cells.
    ##   item?: int — filter to cells with this mesh library item only
    ##
    ## Returns: { success, data: { cells: Array[CellData], cell_count: int } }
    ## CellData: { position: [x, y, z], item: int, orientation: int }

static func op_gridmap_clear(params: Dictionary) -> Dictionary:
    ## Clear cells from a GridMap node in a scene.
    ##
    ## Params:
    ##   scene_path: String — path to the .tscn file
    ##   node_path: String — path to the GridMap node within the scene
    ##   bounds?: Dictionary — { min: [x, y, z], max: [x, y, z] }.
    ##       Omit to clear all cells.
    ##
    ## Returns: { success, data: { cells_cleared: int, node_path: String } }


static func _validate_gridmap(node: Node, operation: String,
        context: Dictionary) -> Dictionary:
    ## Validate that a node is a GridMap.

static func _error(message: String, operation: String,
        context: Dictionary) -> Dictionary:
    return {"success": false, "error": message, "operation": operation,
        "context": context}
```

**Full implementation for `op_gridmap_set_cells`:**

```gdscript
static func op_gridmap_set_cells(params: Dictionary) -> Dictionary:
    var scene_path: String = params.get("scene_path", "")
    var node_path: String = params.get("node_path", "")
    var cells = params.get("cells", [])

    if scene_path == "":
        return _error("scene_path is required", "gridmap_set_cells", params)
    if node_path == "":
        return _error("node_path is required", "gridmap_set_cells", params)
    if not cells is Array or cells.is_empty():
        return _error("cells must be a non-empty array", "gridmap_set_cells", params)

    var loaded = TileMapOps._load_scene_and_find_node(
        scene_path, node_path, "gridmap_set_cells")
    if not loaded.success:
        return loaded

    var root: Node = loaded.root
    var target: Node = loaded.target

    var valid = _validate_gridmap(target, "gridmap_set_cells",
        {"scene_path": scene_path, "node_path": node_path})
    if not valid.success:
        root.free()
        return valid

    if target.mesh_library == null:
        root.free()
        return _error("GridMap has no MeshLibrary assigned. Assign one via " +
            "node_set_properties before setting cells.",
            "gridmap_set_cells", {"node_path": node_path})

    var cells_set := 0
    for cell in cells:
        if not cell is Dictionary:
            root.free()
            return _error("Each cell must be a dictionary with position and item",
                "gridmap_set_cells", {"cell": cell})

        var pos_arr = cell.get("position", null)
        if pos_arr == null or not pos_arr is Array or pos_arr.size() != 3:
            root.free()
            return _error("Cell position must be [x, y, z] array",
                "gridmap_set_cells", {"cell": cell})

        var item: int = int(cell.get("item", -1))
        if item < 0:
            root.free()
            return _error("Cell item must be a non-negative integer (mesh library index)",
                "gridmap_set_cells", {"cell": cell})

        var orientation: int = int(cell.get("orientation", 0))
        var pos = Vector3i(int(pos_arr[0]), int(pos_arr[1]), int(pos_arr[2]))

        target.set_cell_item(pos, item, orientation)
        cells_set += 1

    var save_result = NodeOps._repack_and_save(root, "res://" + scene_path)
    root.free()
    if not save_result.success:
        return save_result

    return {"success": true, "data": {"cells_set": cells_set, "node_path": node_path}}
```

**Full implementation for `op_gridmap_get_cells`:**

```gdscript
static func op_gridmap_get_cells(params: Dictionary) -> Dictionary:
    var scene_path: String = params.get("scene_path", "")
    var node_path: String = params.get("node_path", "")
    var bounds = params.get("bounds", null)
    var filter_item = params.get("item", null)

    if scene_path == "":
        return _error("scene_path is required", "gridmap_get_cells", params)
    if node_path == "":
        return _error("node_path is required", "gridmap_get_cells", params)

    var loaded = TileMapOps._load_scene_and_find_node(
        scene_path, node_path, "gridmap_get_cells")
    if not loaded.success:
        return loaded

    var root: Node = loaded.root
    var target: Node = loaded.target

    var valid = _validate_gridmap(target, "gridmap_get_cells",
        {"scene_path": scene_path, "node_path": node_path})
    if not valid.success:
        root.free()
        return valid

    # Get used cells — optionally filtered by item
    var used_cells: Array[Vector3i]
    if filter_item != null:
        used_cells = target.get_used_cells_by_item(int(filter_item))
    else:
        used_cells = target.get_used_cells()

    # Apply bounds filter if specified
    var has_bounds := false
    var bounds_min := Vector3i.ZERO
    var bounds_max := Vector3i.ZERO
    if bounds is Dictionary:
        var min_arr = bounds.get("min", null)
        var max_arr = bounds.get("max", null)
        if min_arr is Array and min_arr.size() == 3 and max_arr is Array and max_arr.size() == 3:
            bounds_min = Vector3i(int(min_arr[0]), int(min_arr[1]), int(min_arr[2]))
            bounds_max = Vector3i(int(max_arr[0]), int(max_arr[1]), int(max_arr[2]))
            has_bounds = true

    var cells: Array = []
    for pos in used_cells:
        if has_bounds:
            if pos.x < bounds_min.x or pos.x > bounds_max.x \
                    or pos.y < bounds_min.y or pos.y > bounds_max.y \
                    or pos.z < bounds_min.z or pos.z > bounds_max.z:
                continue
        var cell_data: Dictionary = {
            "position": [pos.x, pos.y, pos.z],
            "item": target.get_cell_item(pos),
            "orientation": target.get_cell_item_orientation(pos),
        }
        cells.append(cell_data)

    root.free()

    return {"success": true, "data": {
        "cells": cells,
        "cell_count": cells.size(),
    }}
```

**Full implementation for `op_gridmap_clear`:**

```gdscript
static func op_gridmap_clear(params: Dictionary) -> Dictionary:
    var scene_path: String = params.get("scene_path", "")
    var node_path: String = params.get("node_path", "")
    var bounds = params.get("bounds", null)

    if scene_path == "":
        return _error("scene_path is required", "gridmap_clear", params)
    if node_path == "":
        return _error("node_path is required", "gridmap_clear", params)

    var loaded = TileMapOps._load_scene_and_find_node(
        scene_path, node_path, "gridmap_clear")
    if not loaded.success:
        return loaded

    var root: Node = loaded.root
    var target: Node = loaded.target

    var valid = _validate_gridmap(target, "gridmap_clear",
        {"scene_path": scene_path, "node_path": node_path})
    if not valid.success:
        root.free()
        return valid

    var cells_cleared := 0

    if bounds is Dictionary:
        var min_arr = bounds.get("min", null)
        var max_arr = bounds.get("max", null)
        if min_arr is Array and min_arr.size() == 3 and max_arr is Array and max_arr.size() == 3:
            var bounds_min = Vector3i(int(min_arr[0]), int(min_arr[1]), int(min_arr[2]))
            var bounds_max = Vector3i(int(max_arr[0]), int(max_arr[1]), int(max_arr[2]))
            var used_cells = target.get_used_cells()
            for pos in used_cells:
                if pos.x >= bounds_min.x and pos.x <= bounds_max.x \
                        and pos.y >= bounds_min.y and pos.y <= bounds_max.y \
                        and pos.z >= bounds_min.z and pos.z <= bounds_max.z:
                    target.set_cell_item(pos, -1)  # -1 = INVALID_CELL_ITEM (clears)
                    cells_cleared += 1
    else:
        cells_cleared = target.get_used_cells().size()
        target.clear()

    var save_result = NodeOps._repack_and_save(root, "res://" + scene_path)
    root.free()
    if not save_result.success:
        return save_result

    return {"success": true, "data": {"cells_cleared": cells_cleared, "node_path": node_path}}


static func _validate_gridmap(node: Node, operation: String,
        context: Dictionary) -> Dictionary:
    if node is GridMap:
        return {"success": true}
    return _error("Node is " + node.get_class() + ", expected GridMap",
        operation, context)
```

**Implementation Notes:**
- GridMap uses `set_cell_item(position: Vector3i, item: int, orientation: int)`
  where `item` is the MeshLibrary item index and `orientation` is a basis
  index (0-23 orthogonal rotations).
- Clearing a GridMap cell: `set_cell_item(pos, -1)` where `-1` is
  `GridMap.INVALID_CELL_ITEM`.
- `get_used_cells_by_item(item)` is a built-in GridMap method for efficient
  filtering.
- GridMap has no equivalent to TileMapLayer's `get_used_rect()` — there's no
  bounding box API. We don't include one in the response.
- `_load_scene_and_find_node` is reused from `TileMapOps` via cross-class
  static call (same pattern as `ResourceOps` calling `NodeOps.convert_value`).

**Acceptance Criteria:**
- [ ] `op_gridmap_set_cells` places items and saves the scene
- [ ] `op_gridmap_set_cells` validates MeshLibrary is assigned
- [ ] `op_gridmap_set_cells` validates each cell has required fields
- [ ] `op_gridmap_get_cells` returns all cells with item and orientation
- [ ] `op_gridmap_get_cells` filters by bounds when provided
- [ ] `op_gridmap_get_cells` filters by item when provided
- [ ] `op_gridmap_clear` clears all cells when no bounds given
- [ ] `op_gridmap_clear` clears only cells within bounds when given
- [ ] All three ops validate the node is a GridMap
- [ ] All three ops return structured errors on every failure path

---

### Unit 3: Dispatcher Updates

**File**: `addons/director/operations.gd`

Add to the `_init()` match block, after the existing `resource_duplicate` arm,
and add `TileMapOps`/`GridMapOps` preloads:

```gdscript
const TileMapOps = preload("res://addons/director/ops/tilemap_ops.gd")
const GridMapOps = preload("res://addons/director/ops/gridmap_ops.gd")

# In match block:
"tilemap_set_cells":
    result = TileMapOps.op_tilemap_set_cells(args.params)
"tilemap_get_cells":
    result = TileMapOps.op_tilemap_get_cells(args.params)
"tilemap_clear":
    result = TileMapOps.op_tilemap_clear(args.params)
"gridmap_set_cells":
    result = GridMapOps.op_gridmap_set_cells(args.params)
"gridmap_get_cells":
    result = GridMapOps.op_gridmap_get_cells(args.params)
"gridmap_clear":
    result = GridMapOps.op_gridmap_clear(args.params)
```

**File**: `addons/director/daemon.gd`

Add the same six match arms to `_dispatch()`, and add preloads:

```gdscript
const TileMapOps = preload("res://addons/director/ops/tilemap_ops.gd")
const GridMapOps = preload("res://addons/director/ops/gridmap_ops.gd")

# In _dispatch() match block:
"tilemap_set_cells":
    return TileMapOps.op_tilemap_set_cells(params)
"tilemap_get_cells":
    return TileMapOps.op_tilemap_get_cells(params)
"tilemap_clear":
    return TileMapOps.op_tilemap_clear(params)
"gridmap_set_cells":
    return GridMapOps.op_gridmap_set_cells(params)
"gridmap_get_cells":
    return GridMapOps.op_gridmap_get_cells(params)
"gridmap_clear":
    return GridMapOps.op_gridmap_clear(params)
```

**Acceptance Criteria:**
- [ ] Both dispatchers route all six new operations
- [ ] Preload statements are at the top of each file
- [ ] Unknown operation still returns the existing error response

---

### Unit 4: Rust Parameter Structs

**File**: `crates/director/src/mcp/tilemap.rs` (new file)

```rust
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
```

**File**: `crates/director/src/mcp/gridmap.rs` (new file)

```rust
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A single GridMap cell to set.
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
```

**Acceptance Criteria:**
- [ ] All structs derive `Debug, Deserialize, Serialize, JsonSchema`
- [ ] `project_path` is required on all param structs
- [ ] Optional fields use `Option<T>` with `#[serde(default)]`
- [ ] Cell coordinate arrays match GDScript input format exactly
- [ ] Shared types (`CellRegion`, `GridBounds`) are defined alongside their consumers

---

### Unit 5: Rust MCP Tool Handlers

**File**: `crates/director/src/mcp/mod.rs`

Add module declarations:

```rust
pub mod gridmap;
pub mod tilemap;
```

Add imports:

```rust
use gridmap::{GridMapClearParams, GridMapGetCellsParams, GridMapSetCellsParams};
use tilemap::{TileMapClearParams, TileMapGetCellsParams, TileMapSetCellsParams};
```

Add six tool handlers in the `#[tool_router]` impl block:

```rust
#[tool(
    name = "tilemap_set_cells",
    description = "Set cells on a TileMapLayer node in a Godot scene. Each cell is placed by \
        grid coordinates, TileSet source ID, and atlas coordinates. The TileMapLayer must already \
        have a TileSet resource assigned. Always use this instead of editing .tscn files directly."
)]
pub async fn tilemap_set_cells(
    &self,
    Parameters(params): Parameters<TileMapSetCellsParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "tilemap_set_cells", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "tilemap_get_cells",
    description = "Read used cells from a TileMapLayer node in a Godot scene. Returns cell \
        coordinates, source IDs, atlas coordinates, and the used rect. Optionally filter by \
        region or source ID."
)]
pub async fn tilemap_get_cells(
    &self,
    Parameters(params): Parameters<TileMapGetCellsParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "tilemap_get_cells", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "tilemap_clear",
    description = "Clear cells from a TileMapLayer node in a Godot scene. Optionally specify \
        a region to clear only cells within that rectangle; omit to clear all cells."
)]
pub async fn tilemap_clear(
    &self,
    Parameters(params): Parameters<TileMapClearParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "tilemap_clear", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "gridmap_set_cells",
    description = "Set cells in a GridMap node in a Godot scene. Each cell is placed by 3D grid \
        position and MeshLibrary item index. The GridMap must already have a MeshLibrary resource \
        assigned. Always use this instead of editing .tscn files directly."
)]
pub async fn gridmap_set_cells(
    &self,
    Parameters(params): Parameters<GridMapSetCellsParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "gridmap_set_cells", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "gridmap_get_cells",
    description = "Read used cells from a GridMap node in a Godot scene. Returns cell positions, \
        MeshLibrary item indices, and orientations. Optionally filter by bounds or item."
)]
pub async fn gridmap_get_cells(
    &self,
    Parameters(params): Parameters<GridMapGetCellsParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "gridmap_get_cells", &op_params).await?;
    serialize_response(&data)
}

#[tool(
    name = "gridmap_clear",
    description = "Clear cells from a GridMap node in a Godot scene. Optionally specify bounds \
        to clear only cells within that box; omit to clear all cells."
)]
pub async fn gridmap_clear(
    &self,
    Parameters(params): Parameters<GridMapClearParams>,
) -> Result<String, McpError> {
    let op_params = serialize_params(&params)?;
    let data = run_operation(&self.backend, &params.project_path, "gridmap_clear", &op_params).await?;
    serialize_response(&data)
}
```

**Acceptance Criteria:**
- [ ] All six handlers follow `serialize_params` → `run_operation` → `serialize_response`
- [ ] Tool descriptions include anti-direct-edit guidance where applicable
- [ ] Tool descriptions mention the TileSet/MeshLibrary prerequisite
- [ ] Compiles with `cargo build -p director`

---

### Unit 6: Test Fixtures

**File**: `tests/godot-project/fixtures/test_tileset.tres` (new file)

A minimal TileSet resource is needed for tilemap tests. This can be created
manually in the Godot editor with a simple 1x1 colored texture as an atlas
source. The implementer should:

1. Open the test project in Godot editor
2. Create a new TileSet resource
3. Add a TileSetAtlasSource with a small texture (e.g. 32x32 solid color)
4. Define at least 2 tiles at atlas coords (0,0) and (1,0)
5. Save as `fixtures/test_tileset.tres`

Alternatively, if a programmatic approach is preferred, use Director itself:
create a simple texture and TileSet via GDScript before the tests run.

**Note:** GridMap tests require a MeshLibrary resource. Since MeshLibrary
creation requires the Godot editor (mesh import + export as library), the
implementer should create a minimal `fixtures/test_mesh_library.tres` in the
editor with at least 2 items (simple box meshes).

If creating test fixtures in the editor is not feasible, the gridmap tests
can be marked with an additional `#[ignore]` reason noting the fixture
requirement, and tested manually.

---

### Unit 7: E2E Tests

**File**: `tests/director-tests/src/test_tilemap.rs` (new file)

```rust
use serde_json::json;
use crate::harness::DirectorFixture;

#[test]
#[ignore = "requires Godot binary"]
fn tilemap_set_cells_and_read_back() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("tilemap_set");

    // Create scene with TileMapLayer
    f.run("scene_create", json!({
        "scene_path": scene, "root_type": "Node2D"
    })).unwrap().unwrap_data();

    f.run("node_add", json!({
        "scene_path": scene,
        "node_type": "TileMapLayer",
        "node_name": "Ground",
        "properties": {
            "tile_set": "res://fixtures/test_tileset.tres"
        }
    })).unwrap().unwrap_data();

    // Set cells
    let data = f.run("tilemap_set_cells", json!({
        "scene_path": scene,
        "node_path": "Ground",
        "cells": [
            {"coords": [0, 0], "source_id": 0, "atlas_coords": [0, 0]},
            {"coords": [1, 0], "source_id": 0, "atlas_coords": [1, 0]},
            {"coords": [0, 1], "source_id": 0, "atlas_coords": [0, 0]}
        ]
    })).unwrap().unwrap_data();
    assert_eq!(data["cells_set"], 3);

    // Read back
    let cells = f.run("tilemap_get_cells", json!({
        "scene_path": scene,
        "node_path": "Ground"
    })).unwrap().unwrap_data();
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

    f.run("scene_create", json!({
        "scene_path": scene, "root_type": "Node2D"
    })).unwrap().unwrap_data();

    f.run("node_add", json!({
        "scene_path": scene,
        "node_type": "TileMapLayer",
        "node_name": "Ground",
        "properties": {"tile_set": "res://fixtures/test_tileset.tres"}
    })).unwrap().unwrap_data();

    // Set cells scattered across the map
    f.run("tilemap_set_cells", json!({
        "scene_path": scene,
        "node_path": "Ground",
        "cells": [
            {"coords": [0, 0], "source_id": 0, "atlas_coords": [0, 0]},
            {"coords": [5, 5], "source_id": 0, "atlas_coords": [0, 0]},
            {"coords": [10, 10], "source_id": 0, "atlas_coords": [0, 0]}
        ]
    })).unwrap().unwrap_data();

    // Get cells in a region that includes only (0,0) and (5,5)
    let cells = f.run("tilemap_get_cells", json!({
        "scene_path": scene,
        "node_path": "Ground",
        "region": {"position": [0, 0], "size": [6, 6]}
    })).unwrap().unwrap_data();
    assert_eq!(cells["cell_count"], 2);
}

#[test]
#[ignore = "requires Godot binary"]
fn tilemap_clear_all() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("tilemap_clear_all");

    f.run("scene_create", json!({
        "scene_path": scene, "root_type": "Node2D"
    })).unwrap().unwrap_data();

    f.run("node_add", json!({
        "scene_path": scene,
        "node_type": "TileMapLayer",
        "node_name": "Ground",
        "properties": {"tile_set": "res://fixtures/test_tileset.tres"}
    })).unwrap().unwrap_data();

    f.run("tilemap_set_cells", json!({
        "scene_path": scene,
        "node_path": "Ground",
        "cells": [
            {"coords": [0, 0], "source_id": 0, "atlas_coords": [0, 0]},
            {"coords": [1, 1], "source_id": 0, "atlas_coords": [0, 0]}
        ]
    })).unwrap().unwrap_data();

    // Clear all
    let data = f.run("tilemap_clear", json!({
        "scene_path": scene,
        "node_path": "Ground"
    })).unwrap().unwrap_data();
    assert_eq!(data["cells_cleared"], 2);

    // Verify empty
    let cells = f.run("tilemap_get_cells", json!({
        "scene_path": scene,
        "node_path": "Ground"
    })).unwrap().unwrap_data();
    assert_eq!(cells["cell_count"], 0);
}

#[test]
#[ignore = "requires Godot binary"]
fn tilemap_clear_region() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("tilemap_clear_region");

    f.run("scene_create", json!({
        "scene_path": scene, "root_type": "Node2D"
    })).unwrap().unwrap_data();

    f.run("node_add", json!({
        "scene_path": scene,
        "node_type": "TileMapLayer",
        "node_name": "Ground",
        "properties": {"tile_set": "res://fixtures/test_tileset.tres"}
    })).unwrap().unwrap_data();

    f.run("tilemap_set_cells", json!({
        "scene_path": scene,
        "node_path": "Ground",
        "cells": [
            {"coords": [0, 0], "source_id": 0, "atlas_coords": [0, 0]},
            {"coords": [5, 5], "source_id": 0, "atlas_coords": [0, 0]},
            {"coords": [10, 10], "source_id": 0, "atlas_coords": [0, 0]}
        ]
    })).unwrap().unwrap_data();

    // Clear only the first cell's region
    let data = f.run("tilemap_clear", json!({
        "scene_path": scene,
        "node_path": "Ground",
        "region": {"position": [0, 0], "size": [1, 1]}
    })).unwrap().unwrap_data();
    assert_eq!(data["cells_cleared"], 1);

    // Verify 2 cells remain
    let cells = f.run("tilemap_get_cells", json!({
        "scene_path": scene,
        "node_path": "Ground"
    })).unwrap().unwrap_data();
    assert_eq!(cells["cell_count"], 2);
}

#[test]
#[ignore = "requires Godot binary"]
fn tilemap_rejects_non_tilemap_layer() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("tilemap_wrong_type");

    f.run("scene_create", json!({
        "scene_path": scene, "root_type": "Node2D"
    })).unwrap().unwrap_data();

    f.run("node_add", json!({
        "scene_path": scene,
        "node_type": "Sprite2D",
        "node_name": "NotATileMap"
    })).unwrap().unwrap_data();

    let err = f.run("tilemap_set_cells", json!({
        "scene_path": scene,
        "node_path": "NotATileMap",
        "cells": [{"coords": [0, 0], "source_id": 0, "atlas_coords": [0, 0]}]
    })).unwrap().unwrap_err();
    assert!(err.contains("expected TileMapLayer"));
}

#[test]
#[ignore = "requires Godot binary"]
fn tilemap_rejects_no_tileset() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("tilemap_no_tileset");

    f.run("scene_create", json!({
        "scene_path": scene, "root_type": "Node2D"
    })).unwrap().unwrap_data();

    f.run("node_add", json!({
        "scene_path": scene,
        "node_type": "TileMapLayer",
        "node_name": "NoTileSet"
    })).unwrap().unwrap_data();

    let err = f.run("tilemap_set_cells", json!({
        "scene_path": scene,
        "node_path": "NoTileSet",
        "cells": [{"coords": [0, 0], "source_id": 0, "atlas_coords": [0, 0]}]
    })).unwrap().unwrap_err();
    assert!(err.contains("no TileSet assigned"));
}
```

**File**: `tests/director-tests/src/test_gridmap.rs` (new file)

```rust
use serde_json::json;
use crate::harness::DirectorFixture;

#[test]
#[ignore = "requires Godot binary"]
fn gridmap_set_cells_and_read_back() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("gridmap_set");

    f.run("scene_create", json!({
        "scene_path": scene, "root_type": "Node3D"
    })).unwrap().unwrap_data();

    f.run("node_add", json!({
        "scene_path": scene,
        "node_type": "GridMap",
        "node_name": "Floor",
        "properties": {
            "mesh_library": "res://fixtures/test_mesh_library.tres"
        }
    })).unwrap().unwrap_data();

    // Set cells
    let data = f.run("gridmap_set_cells", json!({
        "scene_path": scene,
        "node_path": "Floor",
        "cells": [
            {"position": [0, 0, 0], "item": 0},
            {"position": [1, 0, 0], "item": 0},
            {"position": [0, 0, 1], "item": 1, "orientation": 10}
        ]
    })).unwrap().unwrap_data();
    assert_eq!(data["cells_set"], 3);

    // Read back
    let cells = f.run("gridmap_get_cells", json!({
        "scene_path": scene,
        "node_path": "Floor"
    })).unwrap().unwrap_data();
    assert_eq!(cells["cell_count"], 3);

    // Verify orientation was preserved
    let cell_list = cells["cells"].as_array().unwrap();
    let oriented_cell = cell_list.iter()
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

    f.run("scene_create", json!({
        "scene_path": scene, "root_type": "Node3D"
    })).unwrap().unwrap_data();

    f.run("node_add", json!({
        "scene_path": scene,
        "node_type": "GridMap",
        "node_name": "Floor",
        "properties": {"mesh_library": "res://fixtures/test_mesh_library.tres"}
    })).unwrap().unwrap_data();

    f.run("gridmap_set_cells", json!({
        "scene_path": scene,
        "node_path": "Floor",
        "cells": [
            {"position": [0, 0, 0], "item": 0},
            {"position": [5, 0, 5], "item": 0},
            {"position": [10, 0, 10], "item": 0}
        ]
    })).unwrap().unwrap_data();

    // Get cells within bounds that only include first two
    let cells = f.run("gridmap_get_cells", json!({
        "scene_path": scene,
        "node_path": "Floor",
        "bounds": {"min": [0, 0, 0], "max": [5, 0, 5]}
    })).unwrap().unwrap_data();
    assert_eq!(cells["cell_count"], 2);
}

#[test]
#[ignore = "requires Godot binary"]
fn gridmap_clear_all() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("gridmap_clear_all");

    f.run("scene_create", json!({
        "scene_path": scene, "root_type": "Node3D"
    })).unwrap().unwrap_data();

    f.run("node_add", json!({
        "scene_path": scene,
        "node_type": "GridMap",
        "node_name": "Floor",
        "properties": {"mesh_library": "res://fixtures/test_mesh_library.tres"}
    })).unwrap().unwrap_data();

    f.run("gridmap_set_cells", json!({
        "scene_path": scene,
        "node_path": "Floor",
        "cells": [
            {"position": [0, 0, 0], "item": 0},
            {"position": [1, 0, 0], "item": 0}
        ]
    })).unwrap().unwrap_data();

    let data = f.run("gridmap_clear", json!({
        "scene_path": scene,
        "node_path": "Floor"
    })).unwrap().unwrap_data();
    assert_eq!(data["cells_cleared"], 2);

    // Verify empty
    let cells = f.run("gridmap_get_cells", json!({
        "scene_path": scene,
        "node_path": "Floor"
    })).unwrap().unwrap_data();
    assert_eq!(cells["cell_count"], 0);
}

#[test]
#[ignore = "requires Godot binary"]
fn gridmap_rejects_non_gridmap() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("gridmap_wrong_type");

    f.run("scene_create", json!({
        "scene_path": scene, "root_type": "Node3D"
    })).unwrap().unwrap_data();

    f.run("node_add", json!({
        "scene_path": scene,
        "node_type": "MeshInstance3D",
        "node_name": "NotAGridMap"
    })).unwrap().unwrap_data();

    let err = f.run("gridmap_set_cells", json!({
        "scene_path": scene,
        "node_path": "NotAGridMap",
        "cells": [{"position": [0, 0, 0], "item": 0}]
    })).unwrap().unwrap_err();
    assert!(err.contains("expected GridMap"));
}

#[test]
#[ignore = "requires Godot binary"]
fn gridmap_rejects_no_mesh_library() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::temp_scene_path("gridmap_no_lib");

    f.run("scene_create", json!({
        "scene_path": scene, "root_type": "Node3D"
    })).unwrap().unwrap_data();

    f.run("node_add", json!({
        "scene_path": scene,
        "node_type": "GridMap",
        "node_name": "NoLib"
    })).unwrap().unwrap_data();

    let err = f.run("gridmap_set_cells", json!({
        "scene_path": scene,
        "node_path": "NoLib",
        "cells": [{"position": [0, 0, 0], "item": 0}]
    })).unwrap().unwrap_err();
    assert!(err.contains("no MeshLibrary assigned"));
}
```

**File**: `tests/director-tests/src/lib.rs`

Add module declarations:

```rust
#[cfg(test)]
mod test_tilemap;
#[cfg(test)]
mod test_gridmap;
```

**Acceptance Criteria:**
- [ ] All tests use `#[ignore = "requires Godot binary"]`
- [ ] TileMap tests: set+read round-trip, region filter, clear all, clear region,
  wrong node type rejection, no TileSet rejection (7 tests)
- [ ] GridMap tests: set+read round-trip, bounds filter, clear all, wrong node
  type rejection, no MeshLibrary rejection (5 tests)
- [ ] Tests verify round-trip (set → read back)
- [ ] Tests verify region/bounds filtering returns correct subsets

---

## Implementation Order

1. **Unit 6: Test fixtures** — create `test_tileset.tres` and
   `test_mesh_library.tres` in the Godot editor first, since tests depend
   on them.
2. **Unit 1: GDScript tilemap ops** — `tilemap_ops.gd` with all three ops and
   shared helpers.
3. **Unit 2: GDScript gridmap ops** — `gridmap_ops.gd` with all three ops.
4. **Unit 3: Dispatcher updates** — wire ops into `operations.gd` and
   `daemon.gd`.
5. **Unit 4: Rust param structs** — `tilemap.rs` and `gridmap.rs`.
6. **Unit 5: Rust MCP handlers** — wire tools in `mod.rs`.
7. **Unit 7: E2E tests** — validates full stack.

Units 4-5 can be implemented together since they're small. Unit 7 should come
last to validate end-to-end.

---

## Testing

### E2E Tests: `tests/director-tests/src/test_tilemap.rs`

7 test cases:
- `tilemap_set_cells_and_read_back` — set 3 cells, read all, verify count and
  used_rect
- `tilemap_get_cells_with_region_filter` — set scattered cells, filter by
  region, verify subset
- `tilemap_clear_all` — set cells, clear all, verify empty
- `tilemap_clear_region` — set cells, clear region, verify partial clear
- `tilemap_rejects_non_tilemap_layer` — wrong node type → error
- `tilemap_rejects_no_tileset` — TileMapLayer without TileSet → error
- (source_id filter is tested implicitly; could add explicit test if needed)

### E2E Tests: `tests/director-tests/src/test_gridmap.rs`

5 test cases:
- `gridmap_set_cells_and_read_back` — set 3 cells with orientation, read all,
  verify count and orientation preservation
- `gridmap_get_cells_with_bounds_filter` — set scattered cells, filter by
  bounds, verify subset
- `gridmap_clear_all` — set cells, clear all, verify empty
- `gridmap_rejects_non_gridmap` — wrong node type → error
- `gridmap_rejects_no_mesh_library` — GridMap without MeshLibrary → error

### Test Fixtures

Required test project fixtures (created manually in Godot editor):
- `tests/godot-project/fixtures/test_tileset.tres` — TileSet with at least 1
  atlas source and 2 tiles at (0,0) and (1,0)
- `tests/godot-project/fixtures/test_mesh_library.tres` — MeshLibrary with at
  least 2 items (simple box meshes)

---

## Verification Checklist

```bash
# Build
cargo build -p director

# Lint
cargo clippy -p director

# Deploy and run E2E tests
theatre-deploy ~/dev/spectator/tests/godot-project
cargo test -p director-tests -- --include-ignored

# Verify all existing tests still pass
cargo test --workspace
```
