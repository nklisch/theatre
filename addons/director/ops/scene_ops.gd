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


static func op_scene_list(params: Dictionary) -> Dictionary:
	## List all .tscn files in the project (or a subdirectory).
	##
	## Params: directory (String, optional — default "")
	## Returns: { success, data: { scenes: [{ path, root_type, node_count }] } }

	var directory: String = params.get("directory", "")

	var base_path: String
	if directory == "":
		base_path = "res://"
	else:
		base_path = "res://" + directory
		var check_dir = DirAccess.open(base_path)
		if check_dir == null:
			return _error("Directory not found: " + directory, "scene_list", {"directory": directory})

	var scene_paths: Array = []
	_collect_scenes(base_path, scene_paths)
	scene_paths.sort()

	var scenes: Array = []
	for full_path in scene_paths:
		var rel_path = full_path.replace("res://", "")
		var packed = load(full_path)
		if packed == null:
			continue
		var root = packed.instantiate()
		if root == null:
			continue
		var root_type = root.get_class()
		var node_count = _count_nodes(root)
		root.free()
		scenes.append({"path": rel_path, "root_type": root_type, "node_count": node_count})

	return {"success": true, "data": {"scenes": scenes}}


static func op_scene_add_instance(params: Dictionary) -> Dictionary:
	## Add a scene instance (reference) as a child in another scene.
	##
	## Params: scene_path, instance_scene, parent_path (default "."), node_name (optional)
	## Returns: { success, data: { node_path, instance_scene } }

	var scene_path: String = params.get("scene_path", "")
	var instance_scene: String = params.get("instance_scene", "")
	var parent_path: String = params.get("parent_path", ".")
	var node_name = params.get("node_name", null)

	if scene_path == "":
		return _error("scene_path is required", "scene_add_instance", params)
	if instance_scene == "":
		return _error("instance_scene is required", "scene_add_instance", params)

	var full_path = "res://" + scene_path
	if not ResourceLoader.exists(full_path):
		return _error("Scene not found: " + scene_path, "scene_add_instance", {"scene_path": scene_path})

	var instance_full_path = "res://" + instance_scene
	if not ResourceLoader.exists(instance_full_path):
		return _error("Instance scene not found: " + instance_scene, "scene_add_instance", {"instance_scene": instance_scene})

	# Load the instance scene as PackedScene
	var instance_packed = load(instance_full_path)
	if not instance_packed is PackedScene:
		return _error("Not a valid scene file: " + instance_scene, "scene_add_instance", {"instance_scene": instance_scene})

	# Load and instantiate the target scene
	var packed: PackedScene = load(full_path)
	var root = packed.instantiate()

	# Create the instance node
	var instance_node = instance_packed.instantiate()
	if node_name != null:
		instance_node.name = node_name

	# Find the parent node
	var parent: Node
	if parent_path == "." or parent_path == "":
		parent = root
	else:
		parent = root.get_node_or_null(parent_path)
	if parent == null:
		instance_node.free()
		root.free()
		return _error("Parent node not found: " + parent_path, "scene_add_instance", {"scene_path": scene_path, "parent_path": parent_path})

	# Name collision check
	if parent.has_node(NodePath(str(instance_node.name))):
		instance_node.free()
		root.free()
		return _error("Name collision: " + str(instance_node.name) + " already exists under " + parent_path + ". Use node_name to resolve.", "scene_add_instance", {"parent_path": parent_path, "instance_scene": instance_scene})

	# Add instance and set owner — do NOT recurse into instance's children
	parent.add_child(instance_node)
	instance_node.owner = root
	# Do not call _set_owner_recursive on instance's children; they belong to the instanced scene

	var node_path_str = str(root.get_path_to(instance_node))

	# Re-pack and save
	var save_result = _repack_and_save(root, full_path)
	root.free()
	if not save_result.success:
		return save_result

	return {"success": true, "data": {"node_path": node_path_str, "instance_scene": instance_scene}}


static func _collect_scenes(dir_path: String, result: Array):
	## Recursively collect all .tscn file paths under dir_path.
	var dir = DirAccess.open(dir_path)
	if dir == null:
		return
	dir.list_dir_begin()
	var file_name = dir.get_next()
	while file_name != "":
		if file_name != "." and file_name != "..":
			var full = dir_path.trim_suffix("/") + "/" + file_name
			if dir.current_is_dir():
				_collect_scenes(full, result)
			elif file_name.ends_with(".tscn"):
				result.append(full)
		file_name = dir.get_next()
	dir.list_dir_end()


static func _count_nodes(node: Node) -> int:
	## Count node plus all its descendants.
	var count = 1
	for child in node.get_children():
		count += _count_nodes(child)
	return count


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
