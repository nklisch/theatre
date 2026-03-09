class_name NodeOps

const OpsUtil = preload("res://addons/director/ops/ops_util.gd")


static func op_node_add(params: Dictionary) -> Dictionary:
	## Add a node to an existing scene.
	##
	## Params: scene_path, parent_path (default "."), node_type, node_name, properties (optional)
	## Returns: { success, data: { node_path, type } }

	var scene_path: String = params.get("scene_path", "")
	var parent_path: String = params.get("parent_path", ".")
	var node_type: String = params.get("node_type", "")
	var node_name: String = params.get("node_name", "")
	var properties = params.get("properties", null)

	if scene_path == "":
		return OpsUtil._error("scene_path is required", "node_add", params)
	if node_type == "":
		return OpsUtil._error("node_type is required", "node_add", params)
	if node_name == "":
		return OpsUtil._error("node_name is required", "node_add", params)

	var full_path = "res://" + scene_path
	if not ResourceLoader.exists(full_path):
		return OpsUtil._error("Scene not found: " + scene_path, "node_add", {"scene_path": scene_path})

	if not ClassDB.class_exists(node_type):
		return OpsUtil._error("Unknown node type: " + node_type, "node_add", {"node_type": node_type})
	if not ClassDB.is_parent_class(node_type, "Node"):
		return OpsUtil._error(node_type + " is not a Node subclass", "node_add", {"node_type": node_type})

	# Load and instantiate the scene
	var packed: PackedScene = load(full_path)
	var root = packed.instantiate()

	# Find the parent node
	var parent: Node
	if parent_path == "." or parent_path == "":
		parent = root
	else:
		parent = root.get_node_or_null(parent_path)
	if parent == null:
		root.free()
		return OpsUtil._error("Parent node not found: " + parent_path, "node_add", {"scene_path": scene_path, "parent_path": parent_path})

	# Create and add the new node
	var new_node = ClassDB.instantiate(node_type)
	new_node.name = node_name
	parent.add_child(new_node)
	new_node.owner = root  # Required for PackedScene serialization

	# Set properties if provided
	if properties is Dictionary:
		var prop_result = _set_properties_on_node(new_node, properties)
		if not prop_result.success:
			root.free()
			return prop_result

	# Re-pack and save
	var save_result = _repack_and_save(root, full_path)
	if not save_result.success:
		root.free()
		return save_result

	var result_path = str(root.get_path_to(new_node))
	root.free()

	return {"success": true, "data": {"node_path": result_path, "type": node_type}}


static func op_node_set_properties(params: Dictionary) -> Dictionary:
	## Set properties on an existing node in a scene.
	##
	## Params: scene_path, node_path, properties (Dictionary)
	## Returns: { success, data: { node_path, properties_set: [] } }

	var scene_path: String = params.get("scene_path", "")
	var node_path: String = params.get("node_path", "")
	var properties = params.get("properties", {})

	if scene_path == "":
		return OpsUtil._error("scene_path is required", "node_set_properties", params)
	if node_path == "":
		return OpsUtil._error("node_path is required", "node_set_properties", params)
	if not properties is Dictionary or properties.is_empty():
		return OpsUtil._error("properties must be a non-empty dictionary", "node_set_properties", params)

	var full_path = "res://" + scene_path
	if not ResourceLoader.exists(full_path):
		return OpsUtil._error("Scene not found: " + scene_path, "node_set_properties", {"scene_path": scene_path})

	var packed: PackedScene = load(full_path)
	var root = packed.instantiate()

	# Find the target node
	var target: Node
	if node_path == "." or node_path == "":
		target = root
	else:
		target = root.get_node_or_null(node_path)
	if target == null:
		root.free()
		return OpsUtil._error("Node not found: " + node_path, "node_set_properties", {"scene_path": scene_path, "node_path": node_path})

	# Set properties with type conversion
	var set_result = _set_properties_on_node(target, properties)
	if not set_result.success:
		root.free()
		return set_result

	# Re-pack and save
	var save_result = _repack_and_save(root, full_path)
	root.free()
	if not save_result.success:
		return save_result

	return {"success": true, "data": {"node_path": node_path, "properties_set": set_result.properties_set}}


static func op_node_remove(params: Dictionary) -> Dictionary:
	## Remove a node (and all children) from a scene.
	##
	## Params: scene_path, node_path
	## Returns: { success, data: { removed: node_path, children_removed: int } }

	var scene_path: String = params.get("scene_path", "")
	var node_path: String = params.get("node_path", "")

	if scene_path == "":
		return OpsUtil._error("scene_path is required", "node_remove", params)
	if node_path == "" or node_path == ".":
		return OpsUtil._error("Cannot remove root node", "node_remove", {"scene_path": scene_path})

	var full_path = "res://" + scene_path
	if not ResourceLoader.exists(full_path):
		return OpsUtil._error("Scene not found: " + scene_path, "node_remove", {"scene_path": scene_path})

	var packed: PackedScene = load(full_path)
	var root = packed.instantiate()

	var target = root.get_node_or_null(node_path)
	if target == null:
		root.free()
		return OpsUtil._error("Node not found: " + node_path, "node_remove", {"scene_path": scene_path, "node_path": node_path})

	var children_count = _count_descendants(target)
	target.get_parent().remove_child(target)
	target.free()

	# Re-pack and save
	var save_result = _repack_and_save(root, full_path)
	root.free()
	if not save_result.success:
		return save_result

	return {"success": true, "data": {"removed": node_path, "children_removed": children_count}}


static func op_node_reparent(params: Dictionary) -> Dictionary:
	## Move a node to a new parent within the same scene.
	##
	## Params: scene_path, node_path, new_parent_path, new_name (optional)
	## Returns: { success, data: { old_path, new_path } }

	var scene_path: String = params.get("scene_path", "")
	var node_path: String = params.get("node_path", "")
	var new_parent_path: String = params.get("new_parent_path", "")
	var new_name = params.get("new_name", null)

	if scene_path == "":
		return OpsUtil._error("scene_path is required", "node_reparent", params)
	if node_path == "" or node_path == ".":
		return OpsUtil._error("Cannot reparent root node", "node_reparent", {"scene_path": scene_path})
	if new_parent_path == "":
		return OpsUtil._error("new_parent_path is required", "node_reparent", params)

	var full_path = "res://" + scene_path
	if not ResourceLoader.exists(full_path):
		return OpsUtil._error("Scene not found: " + scene_path, "node_reparent", {"scene_path": scene_path})

	var packed: PackedScene = load(full_path)
	var root = packed.instantiate()

	# Find the target node
	var target = root.get_node_or_null(node_path)
	if target == null:
		root.free()
		return OpsUtil._error("Node not found: " + node_path, "node_reparent", {"scene_path": scene_path, "node_path": node_path})

	# Find the new parent
	var new_parent: Node
	if new_parent_path == "." or new_parent_path == "":
		new_parent = root
	else:
		new_parent = root.get_node_or_null(new_parent_path)
	if new_parent == null:
		root.free()
		return OpsUtil._error("New parent not found: " + new_parent_path, "node_reparent", {"scene_path": scene_path, "new_parent_path": new_parent_path})

	# Check for circular reparent: target cannot be moved to itself or its own descendant
	if target == new_parent or target.is_ancestor_of(new_parent):
		root.free()
		return OpsUtil._error("Circular reparent: cannot move a node to itself or its own descendant", "node_reparent", {"node_path": node_path, "new_parent_path": new_parent_path})

	# Determine final name
	var final_name = new_name if new_name != null else str(target.name)

	# Name collision check
	if new_parent.has_node(NodePath(final_name)):
		root.free()
		return OpsUtil._error("Name collision: " + final_name + " already exists under " + new_parent_path + ". Use new_name to resolve.", "node_reparent", {"node_path": node_path, "new_parent_path": new_parent_path})

	# Record old path
	var old_path = str(root.get_path_to(target))

	# Reparent
	target.get_parent().remove_child(target)
	if new_name != null:
		target.name = new_name
	new_parent.add_child(target)
	target.owner = root
	_set_owner_recursive(target, root)

	# Record new path
	var new_path_str = str(root.get_path_to(target))

	# Re-pack and save
	var save_result = _repack_and_save(root, full_path)
	root.free()
	if not save_result.success:
		return save_result

	return {"success": true, "data": {"old_path": old_path, "new_path": new_path_str}}


# ---------------------------------------------------------------------------
# Type conversion system
# ---------------------------------------------------------------------------

static func convert_value(value, expected_type: int):
	## Convert a JSON value to the expected Godot type.
	## Called by _set_properties_on_node after querying get_property_list().
	match expected_type:
		TYPE_BOOL:
			return bool(value)
		TYPE_INT:
			return int(value)
		TYPE_FLOAT:
			return float(value)
		TYPE_STRING:
			return str(value)
		TYPE_VECTOR2:
			if value is Dictionary:
				return Vector2(value.get("x", 0), value.get("y", 0))
			return value
		TYPE_VECTOR2I:
			if value is Dictionary:
				return Vector2i(int(value.get("x", 0)), int(value.get("y", 0)))
			return value
		TYPE_VECTOR3:
			if value is Dictionary:
				return Vector3(value.get("x", 0), value.get("y", 0), value.get("z", 0))
			return value
		TYPE_VECTOR3I:
			if value is Dictionary:
				return Vector3i(int(value.get("x", 0)), int(value.get("y", 0)), int(value.get("z", 0)))
			return value
		TYPE_COLOR:
			if value is String:
				return Color.html(value)
			if value is Dictionary:
				return Color(value.get("r", 0), value.get("g", 0), value.get("b", 0), value.get("a", 1.0))
			return value
		TYPE_NODE_PATH:
			return NodePath(str(value))
		TYPE_OBJECT:
			if value is String and str(value).begins_with("res://"):
				return load(str(value))
			return value
		TYPE_RECT2:
			if value is Dictionary:
				var pos = value.get("position", {"x": 0, "y": 0})
				var sz = value.get("size", {"x": 0, "y": 0})
				return Rect2(pos.get("x", 0), pos.get("y", 0), sz.get("x", 0), sz.get("y", 0))
			return value
		_:
			return value


static func _set_properties_on_node(node: Node, properties: Dictionary) -> Dictionary:
	## Set multiple properties on a node with automatic type conversion.
	## Returns { success: true, properties_set: [...] } or { success: false, error: ... }
	var properties_set: Array = []
	var prop_list = node.get_property_list()
	var type_map: Dictionary = {}
	for prop_info in prop_list:
		type_map[prop_info["name"]] = prop_info["type"]

	for prop_name in properties:
		var value = properties[prop_name]

		if not type_map.has(prop_name):
			return {"success": false, "error": "Unknown property: " + prop_name + " on " + node.get_class(), "operation": "node_set_properties", "context": {"node": str(node.name), "property": prop_name}}

		var expected_type = type_map[prop_name]
		var converted = convert_value(value, expected_type)
		node.set(prop_name, converted)
		properties_set.append(prop_name)

	return {"success": true, "properties_set": properties_set}


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

static func _repack_and_save(root: Node, full_path: String) -> Dictionary:
	## Re-pack a modified node tree and save it to disk.
	var packed = PackedScene.new()
	# Set ownership for all descendants so they get included in the packed scene
	_set_owner_recursive(root, root)
	var err = packed.pack(root)
	if err != OK:
		return {"success": false, "error": "Failed to pack scene: " + str(err), "operation": "save", "context": {"path": full_path}}
	err = ResourceSaver.save(packed, full_path)
	if err != OK:
		return {"success": false, "error": "Failed to save scene: " + str(err), "operation": "save", "context": {"path": full_path}}
	return {"success": true}


static func _set_owner_recursive(node: Node, owner: Node):
	## Set owner on all descendants, but skip children of scene instances.
	## A node with a non-empty scene_file_path is an instance root from
	## another scene — its children belong to that scene, not this one.
	for child in node.get_children():
		child.owner = owner
		if child.scene_file_path == "":
			# Only recurse into non-instance children
			_set_owner_recursive(child, owner)


static func _count_descendants(node: Node) -> int:
	var count = 0
	for child in node.get_children():
		count += 1 + _count_descendants(child)
	return count


