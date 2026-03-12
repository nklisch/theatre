class_name ShaderOps

const NodeOps = preload("res://addons/director/ops/node_ops.gd")
const ResourceOps = preload("res://addons/director/ops/resource_ops.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")

## Maps shader_mode strings to VisualShader.Mode enum values.
const SHADER_MODES := {
	"spatial": VisualShader.MODE_SPATIAL,
	"canvas_item": VisualShader.MODE_CANVAS_ITEM,
	"particles": VisualShader.MODE_PARTICLES,
	"sky": VisualShader.MODE_SKY,
	"fog": VisualShader.MODE_FOG,
}

## Maps shader_function strings to VisualShader.Type enum values.
const SHADER_FUNCTIONS := {
	"vertex": VisualShader.TYPE_VERTEX,
	"fragment": VisualShader.TYPE_FRAGMENT,
	"light": VisualShader.TYPE_LIGHT,
	"start": VisualShader.TYPE_START,
	"process": VisualShader.TYPE_PROCESS,
	"collide": VisualShader.TYPE_COLLIDE,
	"start_custom": VisualShader.TYPE_START_CUSTOM,
	"process_custom": VisualShader.TYPE_PROCESS_CUSTOM,
	"sky": VisualShader.TYPE_SKY,
	"fog": VisualShader.TYPE_FOG,
}


static func op_visual_shader_create(params: Dictionary) -> Dictionary:
	## Create a VisualShader resource with nodes and connections.
	##
	## Params: resource_path, shader_mode, nodes: [{node_id, type, shader_function, position?, properties?}],
	##         connections: [{from_node, from_port, to_node, to_port, shader_function}]
	## Returns: { success, data: { path, node_count, connection_count } }

	var resource_path: String = params.get("resource_path", "")
	var shader_mode: String = params.get("shader_mode", "")
	var nodes: Array = params.get("nodes", [])
	var connections: Array = params.get("connections", [])

	# Validate required params
	if resource_path == "":
		return OpsUtil._error("resource_path is required",
			"visual_shader_create", params)
	if shader_mode == "":
		return OpsUtil._error("shader_mode is required",
			"visual_shader_create", params)
	if not SHADER_MODES.has(shader_mode):
		return OpsUtil._error(
			"Invalid shader_mode: " + shader_mode +
			". Must be one of: " + ", ".join(SHADER_MODES.keys()),
			"visual_shader_create", {"shader_mode": shader_mode})

	# Create the VisualShader
	var shader := VisualShader.new()
	shader.set_mode(SHADER_MODES[shader_mode])

	# Add nodes
	var node_count := 0
	for node_def in nodes:
		var result := _add_shader_node(shader, node_def)
		if not result.success:
			return result
		node_count += 1

	# Add connections
	var connection_count := 0
	for conn in connections:
		var result := _add_connection(shader, conn)
		if not result.success:
			return result
		connection_count += 1

	# Save
	var full_path := "res://" + resource_path
	ResourceOps._ensure_directory(full_path)
	var err := ResourceSaver.save(shader, full_path)
	if err != OK:
		return OpsUtil._error("Failed to save visual shader: " + str(err),
			"visual_shader_create", {"resource_path": resource_path})

	return {"success": true, "data": {
		"path": resource_path,
		"node_count": node_count,
		"connection_count": connection_count,
	}}


static func _add_shader_node(shader: VisualShader, node_def: Dictionary) -> Dictionary:
	## Add a single node to the visual shader graph.
	var node_id: int = node_def.get("node_id", -1)
	var node_type: String = node_def.get("type", "")
	var shader_function: String = node_def.get("shader_function", "")

	if node_id < 2:
		return OpsUtil._error(
			"node_id must be >= 2 (0 and 1 are reserved), got: " + str(node_id),
			"visual_shader_create", {"node_id": node_id})
	if node_type == "":
		return OpsUtil._error("Node type is required",
			"visual_shader_create", {"node_id": node_id})
	if shader_function == "":
		return OpsUtil._error(
			"shader_function is required on each node",
			"visual_shader_create", {"node_id": node_id})
	if not SHADER_FUNCTIONS.has(shader_function):
		return OpsUtil._error(
			"Invalid shader_function: " + shader_function +
			". Must be one of: " + ", ".join(SHADER_FUNCTIONS.keys()),
			"visual_shader_create",
			{"node_id": node_id, "shader_function": shader_function})
	if not ClassDB.class_exists(node_type):
		return OpsUtil._error("Unknown class: " + node_type,
			"visual_shader_create", {"node_id": node_id, "type": node_type})
	if not ClassDB.is_parent_class(node_type, "VisualShaderNode"):
		return OpsUtil._error(
			node_type + " is not a VisualShaderNode subclass",
			"visual_shader_create", {"node_id": node_id, "type": node_type})

	var vs_type: int = SHADER_FUNCTIONS[shader_function]
	var node: VisualShaderNode = ClassDB.instantiate(node_type)

	# Set properties if provided
	var properties = node_def.get("properties", null)
	if properties is Dictionary and not properties.is_empty():
		var prop_list = node.get_property_list()
		var type_map: Dictionary = {}
		for prop_info in prop_list:
			type_map[prop_info["name"]] = prop_info["type"]

		for prop_name in properties:
			if not type_map.has(prop_name):
				return OpsUtil._error(
					"Unknown property: " + prop_name + " on " + node_type,
					"visual_shader_create",
					{"node_id": node_id, "property": prop_name})
			var converted = NodeOps.convert_value(
				properties[prop_name], type_map[prop_name])
			node.set(prop_name, converted)

	# Add to the specified shader function graph
	shader.add_node(vs_type, node, Vector2.ZERO, node_id)

	# Set position if provided
	var position = node_def.get("position", null)
	if position is Array and position.size() == 2:
		shader.set_node_position(vs_type, node_id,
			Vector2(position[0], position[1]))

	return {"success": true}


static func _add_connection(shader: VisualShader, conn: Dictionary) -> Dictionary:
	## Add a connection between two nodes in the shader graph.
	var from_node: int = conn.get("from_node", -1)
	var from_port: int = conn.get("from_port", -1)
	var to_node: int = conn.get("to_node", -1)
	var to_port: int = conn.get("to_port", -1)
	var shader_function: String = conn.get("shader_function", "")

	if from_node < 0 or to_node < 0:
		return OpsUtil._error(
			"Connection requires from_node and to_node",
			"visual_shader_create", conn)
	if shader_function == "":
		return OpsUtil._error(
			"shader_function is required on each connection",
			"visual_shader_create", conn)
	if not SHADER_FUNCTIONS.has(shader_function):
		return OpsUtil._error(
			"Invalid shader_function: " + shader_function,
			"visual_shader_create", conn)

	var vs_type: int = SHADER_FUNCTIONS[shader_function]
	var err := shader.connect_nodes(
		vs_type, from_node, from_port, to_node, to_port)
	if err != OK:
		return OpsUtil._error(
			"Failed to connect nodes: " + str(from_node) + ":" +
			str(from_port) + " → " + str(to_node) + ":" + str(to_port) +
			" (error " + str(err) + ")",
			"visual_shader_create", conn)

	return {"success": true}
