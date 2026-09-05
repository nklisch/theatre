class_name NodeOps

const SceneEdit = preload("res://addons/director/ops/scene_edit.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")


static func op_node_add(params: Dictionary, edit: SceneEdit) -> Dictionary:
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

	if not ClassDB.class_exists(node_type) or not ClassDB.is_parent_class(node_type, "Node") or not ClassDB.can_instantiate(node_type):
		return OpsUtil._error("Unknown or non-instantiable node type: " + node_type, "node_add", params)
	var root: Node = edit.root

	# Find the parent node
	var parent: Node = edit.resolve(parent_path)
	if parent == null:
		return OpsUtil._error("Parent node not found: " + parent_path, "node_add", {"scene_path": scene_path, "parent_path": parent_path})

	if not valid_node_name(node_name) or parent.has_node(NodePath(node_name)):
		return OpsUtil._error("Invalid or colliding node name: " + node_name, "node_add", params)
	# Create and add the new node
	var new_node = ClassDB.instantiate(node_type)
	new_node.name = node_name
	if properties is Dictionary:
		var checked = validate_properties(new_node, properties)
		if not checked.success:
			new_node.free()
			return checked
	parent.add_child(new_node)
	new_node.owner = root
	if properties is Dictionary:
		var applied = set_properties(new_node, properties)
		if not applied.success:
			return applied

	var result_path = str(root.get_path_to(new_node))

	return {"success": true, "data": {"node_path": result_path, "type": node_type}}


static func op_node_set_properties(params: Dictionary, edit: SceneEdit) -> Dictionary:
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

	# Find the target node
	var target: Node = edit.resolve(node_path)
	if target == null:
		return OpsUtil._error("Node not found: " + node_path, "node_set_properties", {"scene_path": scene_path, "node_path": node_path})

	# Set properties with type conversion
	var set_result = set_properties(target, properties)
	if not set_result.success:
		return set_result

	return {"success": true, "data": {"node_path": node_path, "properties_set": set_result.properties_set}}


static func op_node_remove(params: Dictionary, edit: SceneEdit) -> Dictionary:
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

	var root: Node = edit.root

	var target: Node = edit.resolve(node_path)
	if target == root:
		return OpsUtil._error("Cannot remove root node", "node_remove", params)
	if target == null:
		return OpsUtil._error("Node not found: " + node_path, "node_remove", {"scene_path": scene_path, "node_path": node_path})

	var children_count = _count_descendants(target)
	target.get_parent().remove_child(target)

	return {"success": true, "data": {"removed": node_path, "children_removed": children_count}}


static func op_node_reparent(params: Dictionary, edit: SceneEdit) -> Dictionary:
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

	var root: Node = edit.root

	# Find the target node
	var target: Node = edit.resolve(node_path)
	if target == root:
		return OpsUtil._error("Cannot reparent root node", "node_reparent", params)
	if target == null:
		return OpsUtil._error("Node not found: " + node_path, "node_reparent", {"scene_path": scene_path, "node_path": node_path})

	# Find the new parent
	var new_parent: Node = edit.resolve(new_parent_path)
	if new_parent == null:
		return OpsUtil._error("New parent not found: " + new_parent_path, "node_reparent", {"scene_path": scene_path, "new_parent_path": new_parent_path})

	# Check for circular reparent: target cannot be moved to itself or its own descendant
	if target == new_parent or target.is_ancestor_of(new_parent):
		return OpsUtil._error("Circular reparent: cannot move a node to itself or its own descendant", "node_reparent", {"node_path": node_path, "new_parent_path": new_parent_path})

	# Determine final name
	var final_name = new_name if new_name != null else str(target.name)

	# Name collision check
	if not valid_node_name(final_name):
		return OpsUtil._error("Invalid node name: " + str(final_name), "node_reparent", params)
	if new_parent.has_node(NodePath(final_name)) and new_parent.get_node(NodePath(final_name)) != target:
		return OpsUtil._error("Name collision: " + final_name + " already exists under " + new_parent_path + ". Use new_name to resolve.", "node_reparent", {"node_path": node_path, "new_parent_path": new_parent_path})

	# Record old path
	var old_path = str(root.get_path_to(target))

	# Reparent
	var owners := {}
	edit.capture_owners(target, owners)
	target.get_parent().remove_child(target)
	if new_name != null:
		target.name = new_name
	new_parent.add_child(target)
	for child in owners:
		var old_owner = owners[child]
		child.owner = old_owner if old_owner == null or old_owner.is_ancestor_of(child) else root

	# Record new path
	var new_path_str = str(root.get_path_to(target))

	return {"success": true, "data": {"old_path": old_path, "new_path": new_path_str}}


# ---------------------------------------------------------------------------
# Type conversion system
# ---------------------------------------------------------------------------

static func convert_value(value, expected_type: int):
	## Convert a JSON value to the expected Godot type.
	## Called by set_properties after querying get_property_list().
	match expected_type:
		TYPE_BOOL:
			return bool(value)
		TYPE_INT:
			return int(value)
		TYPE_FLOAT:
			return float(value)
		TYPE_STRING:
			return str(value)
		TYPE_STRING_NAME:
			return StringName(str(value))
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


static func valid_node_name(value: String) -> bool:
	return not value.is_empty() and value not in [".", ".."] and value.validate_node_name() == value

static func validate_properties(object: Object, properties: Dictionary) -> Dictionary:
	var info_by_name := {}
	for info in object.get_property_list():
		info_by_name[info.name] = info
	var converted := {}
	for name in properties:
		if not info_by_name.has(name):
			return OpsUtil._error("Unknown property: " + name + " on " + object.get_class(), "set_properties", {"property": name})
		var info: Dictionary = info_by_name[name]
		var value = convert_value(properties[name], info.type)
		if info.type == TYPE_OBJECT and properties[name] is String and value == null:
			return OpsUtil._error("Resource not found for property: " + name, "set_properties", {"property": name})
		if info.type != TYPE_NIL and typeof(value) != info.type and not (value == null and info.type == TYPE_OBJECT):
			return OpsUtil._error("Invalid value type for property: " + name, "set_properties", {"property": name})
		if value is Object and info.class_name != &"":
			# Resource property hints may list alternatives, e.g. BaseMaterial3D,ShaderMaterial.
			var matches_class := false
			for allowed in str(info.class_name).split(","):
				if value.is_class(allowed):
					matches_class = true
				var script: Script = value.get_script()
				while script != null:
					if script.get_global_name() == allowed:
						matches_class = true
					script = script.get_base_script()
			if not matches_class:
				return OpsUtil._error("Invalid resource class for property: " + name, "set_properties", {"property": name})
		if object is Node and name == "name":
			if not valid_node_name(str(value)):
				return OpsUtil._error("Invalid node name", "set_properties", {"property": name})
			var parent: Node = object.get_parent()
			if parent != null and parent.has_node(NodePath(value)) and parent.get_node(NodePath(value)) != object:
				return OpsUtil._error("Name collision: " + str(value), "set_properties", {"property": name})
		converted[name] = value
	return {"success": true, "values": converted}

static func set_properties(object: Object, properties: Dictionary) -> Dictionary:
	var checked := validate_properties(object, properties)
	if not checked.success:
		return checked
	var applied: Array = []
	for name in checked.values:
		object.set(name, checked.values[name])
		if not values_match(object.get(name), checked.values[name]):
			var failure := OpsUtil._error("Property setter did not retain requested value: " + name, "set_properties", {"property": name})
			failure["data"] = {"properties_set": applied}
			return failure
		applied.append(name)
	return {"success": true, "properties_set": applied}

static func values_match(actual: Variant, requested: Variant) -> bool:
	if actual == requested:
		return true
	# Engine storage often narrows a JSON float to float32 (materials, shapes),
	# and rotation setters convert units. Native rounding is not a rejected edit.
	if actual is float and requested is float:
		return is_equal_approx(actual, requested)
	if typeof(actual) == typeof(requested) and (actual is Vector2 or actual is Vector3 or actual is Color):
		return actual.is_equal_approx(requested)
	return false


static func _count_descendants(node: Node) -> int:
	var count = 0
	for child in node.get_children():
		count += 1 + _count_descendants(child)
	return count


static func op_node_set_groups(params: Dictionary, edit: SceneEdit) -> Dictionary:
	## Add or remove a node from groups.
	##
	## Params:
	##   scene_path: String
	##   node_path: String
	##   add: Array[String]?      — groups to add
	##   remove: Array[String]?   — groups to remove
	##
	## Returns: { success, data: { node_path, groups: [String] } }

	var scene_path: String = params.get("scene_path", "")
	var node_path: String = params.get("node_path", "")
	var add = params.get("add", null)
	var remove = params.get("remove", null)

	if scene_path == "":
		return OpsUtil._error("scene_path is required", "node_set_groups", params)
	if node_path == "":
		return OpsUtil._error("node_path is required", "node_set_groups", params)
	if (add == null or (add is Array and add.is_empty())) and \
		(remove == null or (remove is Array and remove.is_empty())):
		return OpsUtil._error("At least one of 'add' or 'remove' must be provided", "node_set_groups", params)

	var target: Node = edit.resolve(node_path)
	if target == null:
		return OpsUtil._error("Node not found: " + node_path, "node_set_groups", {"scene_path": scene_path, "node_path": node_path})

	if add is Array:
		for group in add:
			target.add_to_group(str(group), true)  # persistent=true required for PackedScene

	if remove is Array:
		for group in remove:
			target.remove_from_group(str(group))

	# Collect final groups, filtering internal ones
	var final_groups: Array = []
	for group in target.get_groups():
		if not str(group).begins_with("_"):
			final_groups.append(group)

	return {"success": true, "data": {"node_path": node_path, "groups": final_groups}}


static func op_node_set_script(params: Dictionary, edit: SceneEdit) -> Dictionary:
	## Attach or detach a script from a node in a scene.
	##
	## Params:
	##   scene_path: String
	##   node_path: String
	##   script_path: String?     — "res://" path or project-relative path to .gd file
	##                               omit or null to detach
	##
	## Returns: { success, data: { node_path, script_path: String|null } }

	var scene_path: String = params.get("scene_path", "")
	var node_path: String = params.get("node_path", "")
	var script_path = params.get("script_path", null)

	if scene_path == "":
		return OpsUtil._error("scene_path is required", "node_set_script", params)
	if node_path == "":
		return OpsUtil._error("node_path is required", "node_set_script", params)

	var target: Node = edit.resolve(node_path)
	if target == null:
		return OpsUtil._error("Node not found: " + node_path, "node_set_script", {"scene_path": scene_path, "node_path": node_path})

	var result_script_path = null

	if script_path != null and str(script_path) != "":
		var sp: String = str(script_path)
		# Normalize to res:// prefix
		if not sp.begins_with("res://"):
			sp = "res://" + sp

		if not ResourceLoader.exists(sp):
			return OpsUtil._error("Script not found: " + sp, "node_set_script", {"script_path": sp})

		var script = load(sp)
		if not script is Script:
			return OpsUtil._error("File is not a Script resource: " + sp, "node_set_script", {"script_path": sp})

		if not target.is_class(script.get_instance_base_type()):
			return OpsUtil._error("Script base type is incompatible with " + target.get_class(), "node_set_script", params)
		target.set_script(script)
		if target.get_script() != script:
			return OpsUtil._error("Godot rejected script attachment", "node_set_script", params)
		result_script_path = sp.replace("res://", "")
	else:
		target.set_script(null)

	return {"success": true, "data": {"node_path": node_path, "script_path": result_script_path}}


static func op_node_set_meta(params: Dictionary, edit: SceneEdit) -> Dictionary:
	## Set or remove metadata entries on a node in a scene.
	##
	## Params:
	##   scene_path: String
	##   node_path: String
	##   meta: Dictionary          — keys to set; value of null removes the key
	##
	## Returns: { success, data: { node_path, meta_keys: [String] } }

	var scene_path: String = params.get("scene_path", "")
	var node_path: String = params.get("node_path", "")
	var meta = params.get("meta", null)

	if scene_path == "":
		return OpsUtil._error("scene_path is required", "node_set_meta", params)
	if node_path == "":
		return OpsUtil._error("node_path is required", "node_set_meta", params)
	if not meta is Dictionary:
		return OpsUtil._error("meta must be a Dictionary", "node_set_meta", params)

	var target: Node = edit.resolve(node_path)
	if target == null:
		return OpsUtil._error("Node not found: " + node_path, "node_set_meta", {"scene_path": scene_path, "node_path": node_path})

	for key in meta:
		if not str(key).is_valid_identifier():
			return OpsUtil._error("Metadata key must be a valid identifier: " + str(key), "node_set_meta", params)
	for key in meta:
		var value = meta[key]
		if value == null:
			if target.has_meta(key):
				target.remove_meta(key)
		else:
			target.set_meta(key, value)

	var meta_keys: Array = target.get_meta_list()

	return {"success": true, "data": {"node_path": node_path, "meta_keys": meta_keys}}


static func op_node_find(params: Dictionary, edit: SceneEdit) -> Dictionary:
	## Search for nodes in a scene tree by class, group, property, or name.
	##
	## Params:
	##   scene_path: String
	##   class_name: String?       — filter by Godot class (e.g., "Sprite2D")
	##   group: String?            — filter by group membership
	##   name_pattern: String?     — filter by node name (supports * wildcard)
	##   property: String?         — property name that must exist
	##   property_value: any?      — if set, property must equal this value
	##   limit: int?               — max results (default 100)
	##
	## Returns: { success, data: { results: [{ node_path, type, name }] } }

	var scene_path: String = params.get("scene_path", "")
	var filter_class = params.get("class_name", null)
	var filter_group = params.get("group", null)
	var filter_name_pattern = params.get("name_pattern", null)
	var filter_property = params.get("property", null)
	var filter_property_value = params.get("property_value", null)
	var limit: int = params.get("limit", 100)

	if scene_path == "":
		return OpsUtil._error("scene_path is required", "node_find", params)

	# At least one filter must be provided
	if filter_class == null and filter_group == null and filter_name_pattern == null and filter_property == null:
		return OpsUtil._error(
			"At least one filter must be provided (class_name, group, name_pattern, or property)",
			"node_find", params
		)

	var root: Node = edit.root

	var results: Array = []
	_find_nodes_recursive(root, root, filter_class, filter_group, filter_name_pattern,
		filter_property, filter_property_value, limit, results)

	return {"success": true, "data": {"results": results}}


static func _find_nodes_recursive(
		node: Node, root: Node,
		filter_class, filter_group, filter_name_pattern,
		filter_property, filter_property_value,
		limit: int, results: Array) -> void:
	if results.size() >= limit:
		return

	# Apply all filters as AND
	var matches := true

	if filter_class != null and filter_class != "":
		if not node.is_class(str(filter_class)):
			matches = false

	if matches and filter_group != null and filter_group != "":
		if not node.is_in_group(str(filter_group)):
			matches = false

	if matches and filter_name_pattern != null and filter_name_pattern != "":
		if not str(node.name).match(str(filter_name_pattern)):
			matches = false

	if matches and filter_property != null and filter_property != "":
		var prop_name: String = str(filter_property)
		if not prop_name in node:
			matches = false
		elif filter_property_value != null:
			if node.get(prop_name) != filter_property_value:
				matches = false

	if matches:
		var node_path: String = str(root.get_path_to(node))
		if node_path == ".":
			node_path = "."
		results.append({"node_path": node_path, "type": node.get_class(), "name": str(node.name)})

	for child in node.get_children():
		if results.size() >= limit:
			return
		_find_nodes_recursive(child, root, filter_class, filter_group, filter_name_pattern,
			filter_property, filter_property_value, limit, results)


