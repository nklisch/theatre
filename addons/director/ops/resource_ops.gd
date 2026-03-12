class_name ResourceOps

const NodeOps = preload("res://addons/director/ops/node_ops.gd")
const SceneOps = preload("res://addons/director/ops/scene_ops.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")


static func op_resource_read(params: Dictionary) -> Dictionary:
	## Read a resource file and serialize its properties.
	##
	## Params: resource_path (String), depth (int, optional — default 1)
	## Returns: { success, data: { type, path, properties: { ... } } }

	var resource_path: String = params.get("resource_path", "")
	var depth: int = params.get("depth", 1)

	if resource_path == "":
		return OpsUtil._error("resource_path is required", "resource_read", params)

	var full_path = "res://" + resource_path
	if not ResourceLoader.exists(full_path):
		return OpsUtil._error("Resource not found: " + resource_path, "resource_read", {"resource_path": resource_path})

	var resource = load(full_path)
	if resource == null:
		return OpsUtil._error("Failed to load resource: " + resource_path, "resource_read", {"resource_path": resource_path})

	var data: Dictionary = {
		"type": resource.get_class(),
		"path": resource_path,
		"properties": _get_resource_properties(resource, 0, depth),
	}

	if resource_path.ends_with(".tscn"):
		data["hint"] = "Use scene_read for scene tree structure"

	return {"success": true, "data": data}


static func _get_resource_properties(resource: Resource, current_depth: int, max_depth: int) -> Dictionary:
	## Extract non-default properties from a resource.
	## Nested Resource values are serialized recursively up to max_depth.
	var props: Dictionary = {}
	var defaults = ClassDB.instantiate(resource.get_class())

	for prop_info in resource.get_property_list():
		var name: String = prop_info["name"]
		# Skip internal/meta properties
		if name.begins_with("_") or name == "script" or name == "resource_path" or name == "resource_name":
			continue
		if prop_info["usage"] & PROPERTY_USAGE_CATEGORY:
			continue
		if prop_info["usage"] & PROPERTY_USAGE_EDITOR == 0:
			continue

		var value = resource.get(name)
		var default_value = defaults.get(name) if defaults else null

		# Only include non-default values
		if defaults and value == default_value:
			continue

		props[name] = _serialize_resource_value_depth(value, current_depth, max_depth)

	return props


static func _serialize_resource_value_depth(value, current_depth: int, max_depth: int) -> Variant:
	## Serialize a resource property value with depth control.
	## At max_depth: Resource → path string.
	## Below max_depth: Resource → recursive property dict.
	if value is Resource:
		if current_depth >= max_depth:
			if value.resource_path != "":
				return value.resource_path.replace("res://", "")
			return "<" + value.get_class() + ">"
		else:
			var nested: Dictionary = {
				"type": value.get_class(),
			}
			if value.resource_path != "":
				nested["path"] = value.resource_path.replace("res://", "")
			nested["properties"] = _get_resource_properties(value, current_depth + 1, max_depth)
			return nested
	return SceneOps._serialize_value(value)


static func _serialize_resource_value(value) -> Variant:
	## Legacy wrapper: serialize at depth 0 (path only for nested resources).
	return _serialize_resource_value_depth(value, 0, 1)


static func op_material_create(params: Dictionary) -> Dictionary:
	## Create a material resource and save to disk.
	##
	## Params: resource_path, material_type, properties?, shader_path?
	## Returns: { success, data: { path, type } }

	var resource_path: String = params.get("resource_path", "")
	var material_type: String = params.get("material_type", "")

	if resource_path == "":
		return OpsUtil._error("resource_path is required", "material_create", params)
	if material_type == "":
		return OpsUtil._error("material_type is required", "material_create", params)
	if not ClassDB.class_exists(material_type):
		return OpsUtil._error("Unknown class: " + material_type, "material_create",
			{"material_type": material_type})
	if not ClassDB.is_parent_class(material_type, "Material"):
		return OpsUtil._error(material_type + " is not a Material subclass",
			"material_create", {"material_type": material_type})

	var material = ClassDB.instantiate(material_type)

	# Handle ShaderMaterial shader_path
	var shader_path: String = params.get("shader_path", "")
	if shader_path != "":
		if material_type != "ShaderMaterial":
			return OpsUtil._error("shader_path is only valid for ShaderMaterial",
				"material_create", {"material_type": material_type})
		var full_shader = "res://" + shader_path
		if not ResourceLoader.exists(full_shader):
			return OpsUtil._error("Shader not found: " + shader_path,
				"material_create", {"shader_path": shader_path})
		material.shader = load(full_shader)

	# Set properties
	var properties = params.get("properties", null)
	if properties is Dictionary and not properties.is_empty():
		var result = _set_properties_on_resource(material, properties)
		if not result.success:
			return result

	# Save
	var full_path = "res://" + resource_path
	_ensure_directory(full_path)
	var err = ResourceSaver.save(material, full_path)
	if err != OK:
		return OpsUtil._error("Failed to save material: " + str(err),
			"material_create", {"resource_path": resource_path})

	return {"success": true, "data": {"path": resource_path, "type": material_type}}


static func op_shape_create(params: Dictionary) -> Dictionary:
	## Create a collision shape and save/attach it.
	##
	## Params: shape_type, shape_params?, save_path?, scene_path?, node_path?
	## Returns: { success, data: { shape_type, saved_to?, attached_to? } }

	var shape_type: String = params.get("shape_type", "")
	var save_path: String = params.get("save_path", "")
	var scene_path: String = params.get("scene_path", "")
	var node_path: String = params.get("node_path", "")

	if shape_type == "":
		return OpsUtil._error("shape_type is required", "shape_create", params)
	if save_path == "" and scene_path == "":
		return OpsUtil._error("At least one of save_path or scene_path is required",
			"shape_create", params)
	if scene_path != "" and node_path == "":
		return OpsUtil._error("node_path is required when scene_path is set",
			"shape_create", {"scene_path": scene_path})

	if not ClassDB.class_exists(shape_type):
		return OpsUtil._error("Unknown class: " + shape_type, "shape_create",
			{"shape_type": shape_type})
	# Accept both Shape2D and Shape3D subclasses
	if not (ClassDB.is_parent_class(shape_type, "Shape3D") or
			ClassDB.is_parent_class(shape_type, "Shape2D")):
		return OpsUtil._error(shape_type + " is not a Shape2D or Shape3D subclass",
			"shape_create", {"shape_type": shape_type})

	var shape = ClassDB.instantiate(shape_type)

	# Set shape params
	var shape_params = params.get("shape_params", null)
	if shape_params is Dictionary and not shape_params.is_empty():
		var result = _set_properties_on_resource(shape, shape_params)
		if not result.success:
			return result

	var data: Dictionary = {"shape_type": shape_type}

	# Save to file if requested
	if save_path != "":
		var full_save = "res://" + save_path
		_ensure_directory(full_save)
		var err = ResourceSaver.save(shape, full_save)
		if err != OK:
			return OpsUtil._error("Failed to save shape: " + str(err),
				"shape_create", {"save_path": save_path})
		data["saved_to"] = save_path

	# Attach to scene node if requested
	if scene_path != "":
		var full_scene = "res://" + scene_path
		if not ResourceLoader.exists(full_scene):
			return OpsUtil._error("Scene not found: " + scene_path,
				"shape_create", {"scene_path": scene_path})
		var packed: PackedScene = load(full_scene)
		var root = packed.instantiate()
		var target = root.get_node_or_null(node_path)
		if target == null:
			root.free()
			return OpsUtil._error("Node not found: " + node_path,
				"shape_create", {"scene_path": scene_path, "node_path": node_path})

		# Verify the node has a "shape" property
		var has_shape_prop := false
		for prop_info in target.get_property_list():
			if prop_info["name"] == "shape":
				has_shape_prop = true
				break
		if not has_shape_prop:
			root.free()
			return OpsUtil._error("Node " + node_path + " (" + target.get_class() +
				") has no 'shape' property",
				"shape_create", {"node_path": node_path, "class": target.get_class()})

		target.shape = shape
		var save_result = NodeOps._repack_and_save(root, full_scene)
		root.free()
		if not save_result.success:
			return save_result
		data["attached_to"] = node_path

	return {"success": true, "data": data}


static func op_style_box_create(params: Dictionary) -> Dictionary:
	## Create a StyleBox resource and save to disk.
	##
	## Params: resource_path, style_type, properties?
	## Returns: { success, data: { path, type } }

	var resource_path: String = params.get("resource_path", "")
	var style_type: String = params.get("style_type", "")

	if resource_path == "":
		return OpsUtil._error("resource_path is required", "style_box_create", params)
	if style_type == "":
		return OpsUtil._error("style_type is required", "style_box_create", params)

	var valid_types = ["StyleBoxFlat", "StyleBoxTexture", "StyleBoxLine", "StyleBoxEmpty"]
	if not style_type in valid_types:
		return OpsUtil._error("Invalid style_type: " + style_type +
			". Must be one of: " + ", ".join(valid_types),
			"style_box_create", {"style_type": style_type})

	var style_box = ClassDB.instantiate(style_type)

	var properties = params.get("properties", null)
	if properties is Dictionary and not properties.is_empty():
		var result = _set_properties_on_resource(style_box, properties)
		if not result.success:
			return result

	var full_path = "res://" + resource_path
	_ensure_directory(full_path)
	var err = ResourceSaver.save(style_box, full_path)
	if err != OK:
		return OpsUtil._error("Failed to save style box: " + str(err),
			"style_box_create", {"resource_path": resource_path})

	return {"success": true, "data": {"path": resource_path, "type": style_type}}


static func op_resource_duplicate(params: Dictionary) -> Dictionary:
	## Duplicate a resource file to a new path, optionally overriding properties.
	##
	## Params: source_path, dest_path, property_overrides?, deep_copy?
	## Returns: { success, data: { path, type, overrides_applied: [] } }

	var source_path: String = params.get("source_path", "")
	var dest_path: String = params.get("dest_path", "")
	var deep_copy: bool = params.get("deep_copy", false)

	if source_path == "":
		return OpsUtil._error("source_path is required", "resource_duplicate", params)
	if dest_path == "":
		return OpsUtil._error("dest_path is required", "resource_duplicate", params)

	var full_source = "res://" + source_path
	if not ResourceLoader.exists(full_source):
		return OpsUtil._error("Source resource not found: " + source_path,
			"resource_duplicate", {"source_path": source_path})

	var source = load(full_source)
	if source == null:
		return OpsUtil._error("Failed to load source resource: " + source_path,
			"resource_duplicate", {"source_path": source_path})

	var duplicate = source.duplicate(deep_copy)
	var overrides_applied: Array = []

	var property_overrides = params.get("property_overrides", null)
	if property_overrides is Dictionary and not property_overrides.is_empty():
		var result = _set_properties_on_resource(duplicate, property_overrides)
		if not result.success:
			return result
		overrides_applied = result.properties_set

	var full_dest = "res://" + dest_path
	_ensure_directory(full_dest)
	var err = ResourceSaver.save(duplicate, full_dest)
	if err != OK:
		return OpsUtil._error("Failed to save duplicate: " + str(err),
			"resource_duplicate", {"dest_path": dest_path})

	return {"success": true, "data": {
		"path": dest_path,
		"type": duplicate.get_class(),
		"overrides_applied": overrides_applied,
	}}


static func _set_properties_on_resource(resource: Resource, properties: Dictionary) -> Dictionary:
	## Set multiple properties on a resource with type conversion.
	## Mirrors NodeOps._set_properties_on_node but for Resource instead of Node.
	var properties_set: Array = []
	var prop_list = resource.get_property_list()
	var type_map: Dictionary = {}
	for prop_info in prop_list:
		type_map[prop_info["name"]] = prop_info["type"]

	for prop_name in properties:
		var value = properties[prop_name]
		if not type_map.has(prop_name):
			return {"success": false, "error": "Unknown property: " + prop_name +
				" on " + resource.get_class(), "operation": "set_properties",
				"context": {"resource": resource.get_class(), "property": prop_name}}
		var expected_type = type_map[prop_name]
		var converted = NodeOps.convert_value(value, expected_type)
		resource.set(prop_name, converted)
		properties_set.append(prop_name)

	return {"success": true, "properties_set": properties_set}


static func _ensure_directory(full_path: String) -> void:
	## Create parent directories for a resource path if they don't exist.
	var dir_path = full_path.get_base_dir()
	if not DirAccess.dir_exists_absolute(dir_path):
		DirAccess.make_dir_recursive_absolute(dir_path)


