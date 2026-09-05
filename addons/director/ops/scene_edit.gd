@tool
extends RefCounted

## One synchronous scene operation. Own detached roots, borrow editor roots.
## Native history owns the inverse arguments after this context is released.
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")
const READS := ["scene_read", "node_find", "signal_list", "tilemap_get_cells", "gridmap_get_cells", "scene_save"]
const OPERATIONS := ["scene_read", "scene_save", "node_find", "node_add", "node_remove", "node_reparent", "node_set_properties", "node_set_groups", "node_set_script", "node_set_meta", "scene_add_instance", "signal_connect", "signal_disconnect", "signal_list", "tilemap_set_cells", "tilemap_clear", "tilemap_get_cells", "gridmap_set_cells", "gridmap_clear", "gridmap_get_cells", "physics_set_layers", "shape_create"]

var root: Node
var owned := false
var path: String
var operation: String
var params: Dictionary
var target: Node
var before: Dictionary
var undo_manager: EditorUndoRedoManager
var previous_scene := ""

func open(op: String, arguments: Dictionary, borrowed: Node = null, manager: EditorUndoRedoManager = null) -> Dictionary:
	operation = op
	params = arguments
	path = str(params.get("scene_path", "")).trim_prefix("res://").simplify_path()
	if path.is_empty():
		return OpsUtil._error("scene_path is required", op, params)
	root = borrowed
	undo_manager = manager
	if root == null:
		var packed = ResourceLoader.load("res://" + path, "PackedScene", ResourceLoader.CACHE_MODE_IGNORE)
		if not packed is PackedScene:
			return OpsUtil._error("Scene not found or not a PackedScene: " + path, op, params)
		root = packed.instantiate(PackedScene.GEN_EDIT_STATE_INSTANCE)
		owned = true
	if root == null:
		return OpsUtil._error("Failed to instantiate scene: " + path, op, params)
	if not owned and not op in READS:
		var active := EditorInterface.get_edited_scene_root()
		if active != root:
			# Native history selection uses the active tab, not custom_context alone.
			if active != null:
				previous_scene = active.scene_file_path
			EditorInterface.open_scene_from_path("res://" + path)
			if EditorInterface.get_edited_scene_root() != root:
				if not previous_scene.is_empty():
					EditorInterface.open_scene_from_path(previous_scene)
				return OpsUtil._error("Could not activate the existing scene tab", op, params)
	if not op in READS:
		var target_path: String = params.get("source_path", params.get("node_path", "."))
		if op in ["node_add", "scene_add_instance"]:
			target_path = params.get("parent_path", ".")
		target = resolve(target_path)
		before = capture(op, target, params)
	return {"success": true}

func resolve(node_path: String) -> Node:
	var node := root if node_path in ["", "."] else root.get_node_or_null(NodePath(node_path))
	# Relative paths may include '..', but cannot target a different scene/editor node.
	return node if node == root or (node != null and root.is_ancestor_of(node)) else null

func finish(result: Dictionary) -> Dictionary:
	var persistence: Dictionary = result.get("persistence", {"saved_paths": [], "unsaved_scene_paths": []})
	if not operation in READS:
		var after := capture(operation, target, params)
		if operation in ["tilemap_clear", "gridmap_clear"]:
			# Cleared coordinates disappear from get_used_cells; retain their empty after-state.
			for coords in before:
				after[coords] = cell_state(target, coords)
		if before != after:
			if owned:
				var saved := save_root(root, path)
				if saved.success:
					persistence.saved_paths.append(path)
				else:
					# Preserve the mutation result alongside a checked serialization failure.
					saved["data"] = result.get("data", {})
					if not result.get("success", false):
						saved["error"] = result.get("error", "Mutation failed") + "; " + saved.error
					result = saved
			else:
				undo_manager.create_action("Director: " + operation, UndoRedo.MERGE_DISABLE, root)
				if operation in ["node_add", "scene_add_instance"]:
					for node in after.get("children", []):
						if not node in before.get("children", []):
							undo_manager.add_do_method(get_script(), "restore_placement", node, placement(node))
							undo_manager.add_undo_method(target, "remove_child", node)
							undo_manager.add_do_reference(node)
				else:
					undo_manager.add_do_method(get_script(), "restore", operation, target, after)
					undo_manager.add_undo_method(get_script(), "restore", operation, target, before)
					if operation == "node_remove":
						undo_manager.add_undo_reference(target)
				# Execute fallible operations once; retain their actual effects, even on error.
				undo_manager.commit_action(false)
				persistence.unsaved_scene_paths.append(path)
		if owned and operation == "node_remove" and is_instance_valid(target) and target != root and target.get_parent() == null:
			target.free()
	if not previous_scene.is_empty():
		EditorInterface.open_scene_from_path(previous_scene)
	if owned:
		root.free()
	result["persistence"] = persistence
	if result.get("data") is Dictionary:
		result.data["persistence"] = persistence
	return result

static func save_root(scene_root: Node, scene_path: String) -> Dictionary:
	# Never rewrite owners on a borrowed root, or flush unrelated external resources.
	var packed := PackedScene.new()
	var error := packed.pack(scene_root)
	if error == OK:
		error = ResourceSaver.save(packed, "res://" + scene_path)
	if error != OK:
		return OpsUtil._error("Failed to pack/save selected scene (error %d)" % error, "scene_save", {"scene_path": scene_path})
	return {"success": true}

static func placement(node: Node) -> Dictionary:
	var owners := {}
	capture_owners(node, owners)
	var state := {"parent": node.get_parent(), "name": node.name, "index": node.get_index(), "owners": owners}
	if node is Node2D or node is Node3D:
		state["transform"] = node.transform
	elif node is Control:
		state["position"] = node.position
	return state

static func capture_owners(node: Node, owners: Dictionary) -> void:
	owners[node] = node.owner
	for child in node.get_children():
		capture_owners(child, owners)

static func capture(op: String, node: Node, arguments: Dictionary) -> Dictionary:
	if not is_instance_valid(node):
		return {}
	match op:
		"node_add", "scene_add_instance":
			return {"children": node.get_children()}
		"node_remove", "node_reparent":
			return placement(node)
		"node_set_groups":
			# Node exposes membership, while SceneState exposes its persistent flag.
			var packed := PackedScene.new()
			packed.pack(node)
			var persistent := packed.get_state().get_node_groups(0)
			var groups := {}
			for group in node.get_groups():
				groups[group] = group in persistent
			return groups
		"node_set_meta":
			var meta := {}
			for key in arguments.get("meta", {}):
				if node.has_meta(key):
					meta[key] = node.get_meta(key)
			return {"keys": arguments.get("meta", {}).keys(), "values": meta}
		"signal_connect", "signal_disconnect":
			var signal_name: String = arguments.get("signal_name", "")
			return {"signal": signal_name, "connections": node.get_signal_connection_list(signal_name) if node.has_signal(signal_name) else []}
		"tilemap_set_cells", "tilemap_clear", "gridmap_set_cells", "gridmap_clear":
			var cells := {}
			if not (node is TileMapLayer or node is GridMap):
				return cells
			if op.ends_with("set_cells"):
				var requested = arguments.get("cells", [])
				if not requested is Array:
					return cells
				for cell in requested:
					if not cell is Dictionary:
						continue
					var raw = cell.get("coords" if node is TileMapLayer else "position", [])
					if not valid_coordinates(raw, 2 if node is TileMapLayer else 3):
						continue
					var coords = Vector2i(raw[0], raw[1]) if node is TileMapLayer else Vector3i(raw[0], raw[1], raw[2])
					cells[coords] = cell_state(node, coords)
			else:
				for coords in node.get_used_cells():
					if cell_in_region(coords, arguments):
						cells[coords] = cell_state(node, coords)
			return cells
		_:
			var properties := {}
			# Stored values include script exports and observable custom-setter side effects.
			for info in node.get_property_list():
				if info.usage & PROPERTY_USAGE_STORAGE:
					var value = node.get(info.name)
					properties[info.name] = value.duplicate(true) if value is Array or value is Dictionary else value
			return properties

static func valid_coordinates(value: Variant, dimension: int) -> bool:
	if not value is Array or value.size() != dimension:
		return false
	for component in value:
		if not (component is int or component is float):
			return false
	return true

static func cell_state(node: Node, coords: Variant) -> Array:
	if node is TileMapLayer:
		return [node.get_cell_source_id(coords), node.get_cell_atlas_coords(coords), node.get_cell_alternative_tile(coords)]
	var item: int = node.get_cell_item(coords)
	return [item, node.get_cell_item_orientation(coords) if item >= 0 else 0]

static func cell_in_region(coords: Variant, arguments: Dictionary) -> bool:
	if coords is Vector2i and arguments.get("region") is Dictionary:
		var region: Dictionary = arguments.region
		var pos = region.get("position", [])
		var size = region.get("size", [])
		return valid_coordinates(pos, 2) and valid_coordinates(size, 2) and Rect2i(pos[0], pos[1], size[0], size[1]).has_point(coords)
	if coords is Vector3i and arguments.get("bounds") is Dictionary:
		var low = arguments.bounds.get("min", [])
		var high = arguments.bounds.get("max", [])
		return valid_coordinates(low, 3) and valid_coordinates(high, 3) and coords.x >= low[0] and coords.x <= high[0] and coords.y >= low[1] and coords.y <= high[1] and coords.z >= low[2] and coords.z <= high[2]
	return true

static func restore(op: String, node: Node, state: Dictionary) -> void:
	match op:
		"node_remove", "node_reparent":
			restore_placement(node, state)
		"node_set_groups":
			for group in node.get_groups():
				node.remove_from_group(group)
			for group in state:
				node.add_to_group(group, state[group])
		"node_set_meta":
			for key in state.keys:
				if state.values.has(key):
					node.set_meta(key, state.values[key])
				elif node.has_meta(key):
					node.remove_meta(key)
		"signal_connect", "signal_disconnect":
			for connection in node.get_signal_connection_list(state.signal):
				node.disconnect(state.signal, connection.callable)
			for connection in state.connections:
				node.connect(state.signal, connection.callable, connection.flags)
		"tilemap_set_cells", "tilemap_clear", "gridmap_set_cells", "gridmap_clear":
			for coords in state:
				var cell: Array = state[coords]
				if node is TileMapLayer:
					node.set_cell(coords, cell[0], cell[1], cell[2])
				else:
					node.set_cell_item(coords, cell[0], cell[1])
		_:
			if state.has("script") and node.get_script() != state.script:
				node.set_script(state.script)
			for property in state:
				if property != "script" and node.get(property) != state[property]:
					node.set(property, state[property])

static func restore_placement(node: Node, state: Dictionary) -> void:
	if node.get_parent() != state.parent:
		if node.get_parent() != null:
			node.get_parent().remove_child(node)
		node.name = state.name
		if state.parent != null:
			state.parent.add_child(node)
	else:
		node.name = state.name
	if state.parent != null:
		state.parent.move_child(node, state.index)
	for child in state.owners:
		child.owner = state.owners[child]
	if state.has("transform"):
		node.transform = state.transform
	elif state.has("position"):
		node.position = state.position
