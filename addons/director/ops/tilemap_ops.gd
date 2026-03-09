class_name TileMapOps

const NodeOps = preload("res://addons/director/ops/node_ops.gd")


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
	var scene_path: String = params.get("scene_path", "")
	var node_path: String = params.get("node_path", "")
	var cells = params.get("cells", [])

	if scene_path == "":
		return OpsUtil._error("scene_path is required", "tilemap_set_cells", params)
	if node_path == "":
		return OpsUtil._error("node_path is required", "tilemap_set_cells", params)
	if not cells is Array or cells.is_empty():
		return OpsUtil._error("cells must be a non-empty array", "tilemap_set_cells", params)

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
		return OpsUtil._error("TileMapLayer has no TileSet assigned. Assign one via " +
			"node_set_properties before setting cells.",
			"tilemap_set_cells", {"node_path": node_path})

	var cells_set := 0
	for cell in cells:
		if not cell is Dictionary:
			root.free()
			return OpsUtil._error("Each cell must be a dictionary with coords, source_id, atlas_coords",
				"tilemap_set_cells", {"cell": cell})

		var coords_arr = cell.get("coords", null)
		if coords_arr == null or not coords_arr is Array or coords_arr.size() != 2:
			root.free()
			return OpsUtil._error("Cell coords must be [x, y] array",
				"tilemap_set_cells", {"cell": cell})

		var source_id: int = int(cell.get("source_id", 0))
		var atlas_arr = cell.get("atlas_coords", null)
		if atlas_arr == null or not atlas_arr is Array or atlas_arr.size() != 2:
			root.free()
			return OpsUtil._error("Cell atlas_coords must be [x, y] array",
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
	var scene_path: String = params.get("scene_path", "")
	var node_path: String = params.get("node_path", "")
	var region = params.get("region", null)
	var filter_source_id = params.get("source_id", null)

	if scene_path == "":
		return OpsUtil._error("scene_path is required", "tilemap_get_cells", params)
	if node_path == "":
		return OpsUtil._error("node_path is required", "tilemap_get_cells", params)

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
	var scene_path: String = params.get("scene_path", "")
	var node_path: String = params.get("node_path", "")
	var region = params.get("region", null)

	if scene_path == "":
		return OpsUtil._error("scene_path is required", "tilemap_clear", params)
	if node_path == "":
		return OpsUtil._error("node_path is required", "tilemap_clear", params)

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


# ---------------------------------------------------------------------------
# Shared helpers
# ---------------------------------------------------------------------------

static func _load_scene_and_find_node(scene_path: String, node_path: String,
		operation: String) -> Dictionary:
	## Load a scene, find a node, validate it's the expected type.
	## Returns { success: true, root: Node, target: Node } or error dict.
	var full_path = "res://" + scene_path
	if not ResourceLoader.exists(full_path):
		return OpsUtil._error("Scene not found: " + scene_path, operation,
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
		return OpsUtil._error("Node not found: " + node_path, operation,
			{"scene_path": scene_path, "node_path": node_path})

	return {"success": true, "root": root, "target": target}


static func _validate_tilemap_layer(node: Node, operation: String,
		context: Dictionary) -> Dictionary:
	## Validate that a node is a TileMapLayer (not deprecated TileMap).
	## Returns { success: true } or error dict.
	if node is TileMapLayer:
		return {"success": true}
	if node.get_class() == "TileMap":
		return OpsUtil._error("TileMap is deprecated in Godot 4.3+. Use TileMapLayer instead. " +
			"Convert your TileMap to TileMapLayer nodes in the Godot editor.",
			operation, context)
	return OpsUtil._error("Node is " + node.get_class() + ", expected TileMapLayer",
		operation, context)


