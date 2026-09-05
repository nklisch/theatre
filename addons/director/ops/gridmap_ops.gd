class_name GridMapOps

const SceneEdit = preload("res://addons/director/ops/scene_edit.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")


static func op_gridmap_set_cells(params: Dictionary, edit: SceneEdit) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var node: Node = edit.resolve(node_path)
	if node_path.is_empty() or node == null:
		return OpsUtil._error("Node not found: " + node_path, "gridmap_set_cells", params)
	return _set_cells_on_node(node, params)


static func op_gridmap_get_cells(params: Dictionary, edit: SceneEdit) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var node: Node = edit.resolve(node_path)
	if node_path.is_empty() or node == null:
		return OpsUtil._error("Node not found: " + node_path, "gridmap_get_cells", params)
	return _get_cells_from_node(node, params)


static func op_gridmap_clear(params: Dictionary, edit: SceneEdit) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var node: Node = edit.resolve(node_path)
	if node_path.is_empty() or node == null:
		return OpsUtil._error("Node not found: " + node_path, "gridmap_clear", params)
	return _clear_node(node, params)


static func _set_cells_on_node(node: Node, params: Dictionary) -> Dictionary:
	## Set cells on an already-resolved GridMap node.
	## Called by both op_gridmap_set_cells (headless) and EditorOps (live).
	var node_path: String = params.get("node_path", "")
	var cells = params.get("cells", [])

	var valid = OpsUtil._validate_node_type(node, "GridMap", "gridmap_set_cells",
		{"node_path": node_path})
	if not valid.success:
		return valid

	if node.mesh_library == null:
		return OpsUtil._error("GridMap has no MeshLibrary assigned. Assign one via " +
			"node_set_properties before setting cells.",
			"gridmap_set_cells", {"node_path": node_path})

	if not cells is Array or cells.is_empty():
		return OpsUtil._error("cells must be a non-empty array", "gridmap_set_cells", params)
	var converted_cells: Array = []
	for cell in cells:
		if not cell is Dictionary:
			return OpsUtil._error("Each cell must be a dictionary with position and item",
				"gridmap_set_cells", {"cell": cell})

		var pos_arr = cell.get("position", null)
		if not SceneEdit.valid_coordinates(pos_arr, 3):
			return OpsUtil._error("Cell position must be [x, y, z] array",
				"gridmap_set_cells", {"cell": cell})

		var item: int = int(cell.get("item", -1))
		if item < 0:
			return OpsUtil._error("Cell item must be a non-negative integer (mesh library index)",
				"gridmap_set_cells", {"cell": cell})

		var orientation: int = int(cell.get("orientation", 0))
		var pos = Vector3i(int(pos_arr[0]), int(pos_arr[1]), int(pos_arr[2]))

		if orientation < 0 or orientation > 23:
			return OpsUtil._error("orientation must be 0-23", "gridmap_set_cells", params)
		converted_cells.append([pos, item, orientation])
	for cell in converted_cells:
		node.set_cell_item(cell[0], cell[1], cell[2])
	var cells_set := converted_cells.size()

	return {"success": true, "data": {"cells_set": cells_set, "node_path": node_path}}


static func _get_cells_from_node(node: Node, params: Dictionary) -> Dictionary:
	## Read cells from an already-resolved GridMap node.
	## Called by both op_gridmap_get_cells (headless) and EditorOps (live).
	var node_path: String = params.get("node_path", "")
	var bounds = params.get("bounds", null)
	var filter_item = params.get("item", null)

	var valid = OpsUtil._validate_node_type(node, "GridMap", "gridmap_get_cells",
		{"node_path": node_path})
	if not valid.success:
		return valid

	var used_cells: Array[Vector3i]
	if filter_item != null:
		used_cells = node.get_used_cells_by_item(int(filter_item))
	else:
		used_cells = node.get_used_cells()

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
			"item": node.get_cell_item(pos),
			"orientation": node.get_cell_item_orientation(pos),
		}
		cells.append(cell_data)

	return {"success": true, "data": {
		"cells": cells,
		"cell_count": cells.size(),
	}}


static func _clear_node(node: Node, params: Dictionary) -> Dictionary:
	## Clear cells on an already-resolved GridMap node.
	## Called by both op_gridmap_clear (headless) and EditorOps (live).
	var node_path: String = params.get("node_path", "")
	var bounds = params.get("bounds", null)

	var valid = OpsUtil._validate_node_type(node, "GridMap", "gridmap_clear",
		{"node_path": node_path})
	if not valid.success:
		return valid

	if bounds != null and (not bounds is Dictionary or not SceneEdit.valid_coordinates(bounds.get("min"), 3) or not SceneEdit.valid_coordinates(bounds.get("max"), 3)):
		return OpsUtil._error("bounds requires min and max [x, y, z] arrays", "gridmap_clear", params)
	var cells_cleared := 0

	if bounds is Dictionary:
		var min_arr = bounds.get("min", null)
		var max_arr = bounds.get("max", null)
		if min_arr is Array and min_arr.size() == 3 and max_arr is Array and max_arr.size() == 3:
			var bounds_min = Vector3i(int(min_arr[0]), int(min_arr[1]), int(min_arr[2]))
			var bounds_max = Vector3i(int(max_arr[0]), int(max_arr[1]), int(max_arr[2]))
			var used_cells = node.get_used_cells()
			for pos in used_cells:
				if pos.x >= bounds_min.x and pos.x <= bounds_max.x \
						and pos.y >= bounds_min.y and pos.y <= bounds_max.y \
						and pos.z >= bounds_min.z and pos.z <= bounds_max.z:
					node.set_cell_item(pos, -1)  # -1 = INVALID_CELL_ITEM (clears)
					cells_cleared += 1
	else:
		cells_cleared = node.get_used_cells().size()
		node.clear()

	return {"success": true, "data": {"cells_cleared": cells_cleared, "node_path": node_path}}


# ---------------------------------------------------------------------------
# Coordinate validation and undo cell state are shared through SceneEdit.
# ---------------------------------------------------------------------------

