class_name SceneOps


static func op_scene_create(params: Dictionary) -> Dictionary:
	## Create a new scene with the specified root node type.
	##
	## Params: scene_path (String), root_type (String)
	## Returns: { success, data: { path, root_type } }

	var scene_path: String = params.get("scene_path", "")
	var root_type: String = params.get("root_type", "")

	if scene_path == "":
		return _error("scene_path is required", "scene_create", params)
	if root_type == "":
		return _error("root_type is required", "scene_create", params)

	# Validate the class exists
	if not ClassDB.class_exists(root_type):
		return _error("Unknown node type: " + root_type, "scene_create", {"scene_path": scene_path, "root_type": root_type})

	# Ensure the class is a Node subclass
	if not ClassDB.is_parent_class(root_type, "Node"):
		return _error(root_type + " is not a Node subclass", "scene_create", {"scene_path": scene_path, "root_type": root_type})

	# Create the root node
	var root = ClassDB.instantiate(root_type)
	root.name = _name_from_path(scene_path)

	# Pack into a scene
	var packed = PackedScene.new()
	var err = packed.pack(root)
	root.free()
	if err != OK:
		return _error("Failed to pack scene: " + str(err), "scene_create", {"scene_path": scene_path})

	# Ensure parent directory exists
	var full_path = "res://" + scene_path
	var dir_path = full_path.get_base_dir()
	if not DirAccess.dir_exists_absolute(dir_path):
		DirAccess.make_dir_recursive_absolute(dir_path)

	# Save
	err = ResourceSaver.save(packed, full_path)
	if err != OK:
		return _error("Failed to save scene: " + str(err), "scene_create", {"scene_path": scene_path})

	return {"success": true, "data": {"path": scene_path, "root_type": root_type}}


static func op_scene_read(params: Dictionary) -> Dictionary:
	## Read the full node tree of a scene file.
	##
	## Params: scene_path (String), depth (int, optional), properties (bool, default true)
	## Returns: { success, data: { root: NodeData } }

	var scene_path: String = params.get("scene_path", "")
	if scene_path == "":
		return _error("scene_path is required", "scene_read", params)

	var full_path = "res://" + scene_path
	if not ResourceLoader.exists(full_path):
		return _error("Scene not found: " + scene_path, "scene_read", {"scene_path": scene_path})

	var packed: PackedScene = load(full_path)
	if packed == null:
		return _error("Failed to load scene: " + scene_path, "scene_read", {"scene_path": scene_path})

	var root = packed.instantiate()
	if root == null:
		return _error("Failed to instantiate scene: " + scene_path, "scene_read", {"scene_path": scene_path})

	var max_depth: int = params.get("depth", -1)
	var include_props: bool = params.get("properties", true)

	var node_data = _read_node(root, 0, max_depth, include_props)
	root.free()

	return {"success": true, "data": {"root": node_data}}


static func _read_node(node: Node, current_depth: int, max_depth: int, include_props: bool) -> Dictionary:
	var data: Dictionary = {
		"name": node.name,
		"type": node.get_class(),
	}

	if include_props:
		data["properties"] = _get_serializable_properties(node)

	if max_depth < 0 or current_depth < max_depth:
		var children: Array = []
		for child in node.get_children():
			children.append(_read_node(child, current_depth + 1, max_depth, include_props))
		if children.size() > 0:
			data["children"] = children

	return data


static func _get_serializable_properties(node: Node) -> Dictionary:
	## Extract non-default, user-relevant properties from a node.
	var props: Dictionary = {}
	var defaults = ClassDB.instantiate(node.get_class())

	for prop_info in node.get_property_list():
		var name: String = prop_info["name"]
		# Skip internal/meta properties
		if name.begins_with("_") or name == "script" or prop_info["usage"] & PROPERTY_USAGE_CATEGORY:
			continue
		if prop_info["usage"] & PROPERTY_USAGE_EDITOR == 0:
			continue

		var value = node.get(name)
		var default_value = defaults.get(name) if defaults else null

		# Only include non-default values
		if defaults and value == default_value:
			continue

		props[name] = _serialize_value(value)

	if defaults:
		defaults.free()
	return props


static func _serialize_value(value) -> Variant:
	## Convert a Godot value to a JSON-safe representation.
	if value is Vector2:
		return {"x": value.x, "y": value.y}
	elif value is Vector3:
		return {"x": value.x, "y": value.y, "z": value.z}
	elif value is Color:
		return {"r": value.r, "g": value.g, "b": value.b, "a": value.a}
	elif value is NodePath:
		return str(value)
	elif value is Resource:
		return value.resource_path if value.resource_path != "" else str(value)
	elif value is Rect2:
		return {"position": {"x": value.position.x, "y": value.position.y}, "size": {"x": value.size.x, "y": value.size.y}}
	elif value is Transform2D:
		return {"origin": {"x": value.origin.x, "y": value.origin.y}, "x": {"x": value.x.x, "y": value.x.y}, "y": {"x": value.y.x, "y": value.y.y}}
	elif value is Basis:
		return {"x": {"x": value.x.x, "y": value.x.y, "z": value.x.z}, "y": {"x": value.y.x, "y": value.y.y, "z": value.y.z}, "z": {"x": value.z.x, "y": value.z.y, "z": value.z.z}}
	elif value is Transform3D:
		return {"basis": _serialize_value(value.basis), "origin": {"x": value.origin.x, "y": value.origin.y, "z": value.origin.z}}
	elif value is Array:
		var arr = []
		for item in value:
			arr.append(_serialize_value(item))
		return arr
	elif value is Dictionary:
		var dict = {}
		for key in value:
			dict[str(key)] = _serialize_value(value[key])
		return dict
	else:
		return value


static func _name_from_path(scene_path: String) -> String:
	## Extract a node name from a scene path: "scenes/player.tscn" → "Player"
	var file_name = scene_path.get_file().get_basename()
	return file_name.capitalize().replace(" ", "")


static func _error(message: String, operation: String, context: Dictionary) -> Dictionary:
	return {"success": false, "error": message, "operation": operation, "context": context}
