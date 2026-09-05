class_name PhysicsOps

const SceneEdit = preload("res://addons/director/ops/scene_edit.gd")
const NodeOps = preload("res://addons/director/ops/node_ops.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")


static func op_physics_set_layers(params: Dictionary, edit: SceneEdit) -> Dictionary:
	## Set collision_layer and/or collision_mask on a node in a scene.
	##
	## Params: scene_path, node_path, collision_layer?, collision_mask?
	## Returns: { success, data: { node_path, collision_layer, collision_mask } }

	var scene_path: String = params.get("scene_path", "")
	var node_path: String = params.get("node_path", "")
	var collision_layer = params.get("collision_layer", null)
	var collision_mask = params.get("collision_mask", null)

	# Validation
	if scene_path == "":
		return OpsUtil._error("scene_path is required", "physics_set_layers", params)
	if node_path == "":
		return OpsUtil._error("node_path is required", "physics_set_layers", params)
	if collision_layer == null and collision_mask == null:
		return OpsUtil._error(
			"At least one of collision_layer or collision_mask is required",
			"physics_set_layers", params)

	# Load scene + find node
	var target: Node = edit.resolve(node_path)
	if target == null:
		return OpsUtil._error("Node not found: " + node_path,
			"physics_set_layers", {"scene_path": scene_path, "node_path": node_path})

	# Verify the node has collision properties
	var has_layer := false
	var has_mask := false
	var target_class: String = target.get_class()
	for prop_info in target.get_property_list():
		if prop_info["name"] == "collision_layer":
			has_layer = true
		if prop_info["name"] == "collision_mask":
			has_mask = true
	if not has_layer and not has_mask:
		return OpsUtil._error(
			"Node " + node_path + " (" + target_class +
			") has no collision_layer/collision_mask properties",
			"physics_set_layers",
			{"node_path": node_path, "class": target_class})

	var properties := {}
	if collision_layer != null:
		properties["collision_layer"] = collision_layer
	if collision_mask != null:
		properties["collision_mask"] = collision_mask
	var applied = NodeOps.set_properties(target, properties)
	if not applied.success:
		return applied

	var final_layer: int = target.collision_layer
	var final_mask: int = target.collision_mask

	return {"success": true, "data": {
		"node_path": node_path,
		"collision_layer": final_layer,
		"collision_mask": final_mask,
	}}


static func op_physics_set_layer_names(params: Dictionary) -> Dictionary:
	## Write physics/render/navigation layer names to project.godot.
	##
	## Params: layer_type, layers (dict of layer_number → name)
	## Returns: { success, data: { layer_type, layers_set: int } }

	var layer_type: String = params.get("layer_type", "")
	var layers = params.get("layers", {})

	if layer_type == "":
		return OpsUtil._error("layer_type is required",
			"physics_set_layer_names", params)

	var valid_types := [
		"2d_physics", "3d_physics",
		"2d_render", "3d_render",
		"2d_navigation", "3d_navigation",
		"avoidance",
	]
	if not layer_type in valid_types:
		return OpsUtil._error(
			"Invalid layer_type: " + layer_type +
			". Must be one of: " + ", ".join(valid_types),
			"physics_set_layer_names", {"layer_type": layer_type})

	if not layers is Dictionary or layers.is_empty():
		return OpsUtil._error("layers must be a non-empty dictionary",
			"physics_set_layer_names", params)

	var layers_set := 0
	for key in layers:
		var layer_num: int = int(key)
		if layer_num < 1 or layer_num > 32:
			return OpsUtil._error(
				"Layer number must be 1-32, got: " + str(key),
				"physics_set_layer_names",
				{"layer_type": layer_type, "layer": key})

		var name: String = str(layers[key])
		var setting := "layer_names/" + layer_type + "/layer_" + str(layer_num)
		ProjectSettings.set_setting(setting, name)
		layers_set += 1

	var err := ProjectSettings.save()
	if err != OK:
		return OpsUtil._error("Failed to save project settings: " + str(err),
			"physics_set_layer_names", {"layer_type": layer_type})

	return {"success": true, "data": {
		"layer_type": layer_type,
		"layers_set": layers_set,
	}}
