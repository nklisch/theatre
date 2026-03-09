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


# ---------------------------------------------------------------------------
# Shared helpers
# ---------------------------------------------------------------------------

static func _validate_gridmap(node: Node, operation: String,
		context: Dictionary) -> Dictionary:
	## Validate that a node is a GridMap.
	## Returns { success: true } or error dict.
	if node is GridMap:
		return {"success": true}
	return _error("Node is " + node.get_class() + ", expected GridMap",
		operation, context)


static func _error(message: String, operation: String,
		context: Dictionary) -> Dictionary:
	return {"success": false, "error": message, "operation": operation,
		"context": context}
