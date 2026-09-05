class_name TileMapOps

const SceneEdit = preload("res://addons/director/ops/scene_edit.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")


static func op_tilemap_set_cells(params: Dictionary, edit: SceneEdit) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var node: Node = edit.resolve(node_path)
	if node_path.is_empty() or node == null:
		return OpsUtil._error("Node not found: " + node_path, "tilemap_set_cells", params)
	return _set_cells_on_node(node, params)


static func op_tilemap_get_cells(params: Dictionary, edit: SceneEdit) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var node: Node = edit.resolve(node_path)
	if node_path.is_empty() or node == null:
		return OpsUtil._error("Node not found: " + node_path, "tilemap_get_cells", params)
	return _get_cells_from_node(node, params)


static func op_tilemap_clear(params: Dictionary, edit: SceneEdit) -> Dictionary:
	var node_path: String = params.get("node_path", "")
	var node: Node = edit.resolve(node_path)
	if node_path.is_empty() or node == null:
		return OpsUtil._error("Node not found: " + node_path, "tilemap_clear", params)
	return _clear_node(node, params)


static func _set_cells_on_node(node: Node, params: Dictionary) -> Dictionary:
	## Set cells on an already-resolved TileMapLayer node.
	## Called by both op_tilemap_set_cells (headless) and EditorOps (live).
	var node_path: String = params.get("node_path", "")
	var cells = params.get("cells", [])

	var valid = _validate_tilemap_layer(node, "tilemap_set_cells", {"node_path": node_path})
	if not valid.success:
		return valid

	if node.tile_set == null:
		return OpsUtil._error("TileMapLayer has no TileSet assigned. Assign one via " +
			"node_set_properties before setting cells.",
			"tilemap_set_cells", {"node_path": node_path})

	if not cells is Array or cells.is_empty():
		return OpsUtil._error("cells must be a non-empty array", "tilemap_set_cells", params)
	var converted_cells: Array = []
	for cell in cells:
		if not cell is Dictionary:
			return OpsUtil._error("Each cell must be a dictionary with coords, source_id, atlas_coords",
				"tilemap_set_cells", {"cell": cell})

		var coords_arr = cell.get("coords", null)
		if not SceneEdit.valid_coordinates(coords_arr, 2):
			return OpsUtil._error("Cell coords must be [x, y] array",
				"tilemap_set_cells", {"cell": cell})

		var source_id: int = int(cell.get("source_id", 0))
		var atlas_arr = cell.get("atlas_coords", null)
		if not SceneEdit.valid_coordinates(atlas_arr, 2):
			return OpsUtil._error("Cell atlas_coords must be [x, y] array",
				"tilemap_set_cells", {"cell": cell})

		var alt_tile: int = int(cell.get("alternative_tile", 0))
		var coords = Vector2i(int(coords_arr[0]), int(coords_arr[1]))
		var atlas_coords = Vector2i(int(atlas_arr[0]), int(atlas_arr[1]))

		converted_cells.append([coords, source_id, atlas_coords, alt_tile])
	for cell in converted_cells:
		node.set_cell(cell[0], cell[1], cell[2], cell[3])
	var cells_set := converted_cells.size()

	return {"success": true, "data": {"cells_set": cells_set, "node_path": node_path}}


static func _get_cells_from_node(node: Node, params: Dictionary) -> Dictionary:
	## Read cells from an already-resolved TileMapLayer node.
	## Called by both op_tilemap_get_cells (headless) and EditorOps (live).
	var node_path: String = params.get("node_path", "")
	var region = params.get("region", null)
	var filter_source_id = params.get("source_id", null)

	var valid = _validate_tilemap_layer(node, "tilemap_get_cells", {"node_path": node_path})
	if not valid.success:
		return valid

	# Get used cells — optionally filtered by source_id
	var used_cells: Array[Vector2i]
	if filter_source_id != null:
		used_cells = node.get_used_cells_by_id(int(filter_source_id))
	else:
		used_cells = node.get_used_cells()

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
			"source_id": node.get_cell_source_id(coords),
			"atlas_coords": [
				node.get_cell_atlas_coords(coords).x,
				node.get_cell_atlas_coords(coords).y,
			],
			"alternative_tile": node.get_cell_alternative_tile(coords),
		}
		cells.append(cell_data)

	var used_rect = node.get_used_rect()
	var used_rect_data = {
		"position": [used_rect.position.x, used_rect.position.y],
		"size": [used_rect.size.x, used_rect.size.y],
	}

	return {"success": true, "data": {
		"cells": cells,
		"cell_count": cells.size(),
		"used_rect": used_rect_data,
	}}


static func _clear_node(node: Node, params: Dictionary) -> Dictionary:
	## Clear cells on an already-resolved TileMapLayer node.
	## Called by both op_tilemap_clear (headless) and EditorOps (live).
	var node_path: String = params.get("node_path", "")
	var region = params.get("region", null)

	var valid = _validate_tilemap_layer(node, "tilemap_clear", {"node_path": node_path})
	if not valid.success:
		return valid

	if region != null and (not region is Dictionary or not SceneEdit.valid_coordinates(region.get("position"), 2) or not SceneEdit.valid_coordinates(region.get("size"), 2)):
		return OpsUtil._error("region requires position and size [x, y] arrays", "tilemap_clear", params)
	var cells_cleared := 0

	if region is Dictionary:
		var pos = region.get("position", [0, 0])
		var sz = region.get("size", [0, 0])
		if pos is Array and pos.size() == 2 and sz is Array and sz.size() == 2:
			var region_rect = Rect2i(
				int(pos[0]), int(pos[1]),
				int(sz[0]), int(sz[1])
			)
			var used_cells = node.get_used_cells()
			for coords in used_cells:
				if region_rect.has_point(coords):
					node.erase_cell(coords)
					cells_cleared += 1
	else:
		cells_cleared = node.get_used_cells().size()
		node.clear()

	return {"success": true, "data": {"cells_cleared": cells_cleared, "node_path": node_path}}


# ---------------------------------------------------------------------------
# Shared helpers
# ---------------------------------------------------------------------------

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


