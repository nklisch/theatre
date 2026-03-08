class_name ResourceOps


static func op_resource_read(params: Dictionary) -> Dictionary:
	## Read a resource file and serialize its properties.
	##
	## Params: resource_path (String)
	## Returns: { success, data: { type, path, properties: { ... } } }

	var resource_path: String = params.get("resource_path", "")
	if resource_path == "":
		return _error("resource_path is required", "resource_read", params)

	var full_path = "res://" + resource_path
	if not ResourceLoader.exists(full_path):
		return _error("Resource not found: " + resource_path, "resource_read", {"resource_path": resource_path})

	var resource = load(full_path)
	if resource == null:
		return _error("Failed to load resource: " + resource_path, "resource_read", {"resource_path": resource_path})

	var data: Dictionary = {
		"type": resource.get_class(),
		"path": resource_path,
		"properties": _get_resource_properties(resource),
	}

	if resource_path.ends_with(".tscn"):
		data["hint"] = "Use scene_read for scene tree structure"

	return {"success": true, "data": data}


static func _get_resource_properties(resource: Resource) -> Dictionary:
	## Extract non-default properties from a resource, one level deep.
	## Nested Resource values serialize as their resource_path string.
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

		props[name] = _serialize_resource_value(value)

	if defaults:
		defaults.free()
	return props


static func _serialize_resource_value(value) -> Variant:
	## Like SceneOps._serialize_value but for resources.
	## Resource references → path string (one level deep).
	if value is Resource:
		if value.resource_path != "":
			return value.resource_path.replace("res://", "")
		return "<" + value.get_class() + ">"
	return SceneOps._serialize_value(value)


static func _error(message: String, operation: String, context: Dictionary) -> Dictionary:
	return {"success": false, "error": message, "operation": operation, "context": context}
