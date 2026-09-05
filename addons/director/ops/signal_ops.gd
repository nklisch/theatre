class_name SignalOps

const SceneEdit = preload("res://addons/director/ops/scene_edit.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")


static func op_signal_connect(params: Dictionary, edit: SceneEdit) -> Dictionary:
	## Connect a signal between two nodes in a scene.
	##
	## Params:
	##   scene_path: String
	##   source_path: String        — node emitting the signal (relative to root)
	##   signal_name: String        — signal name (e.g., "pressed", "body_entered")
	##   target_path: String        — node receiving the signal
	##   method_name: String        — method to call on target
	##   binds: Array? (optional)   — extra arguments to pass to the method
	##   flags: int? (optional)     — ConnectFlags bitmask (default 0)
	##
	## Returns: { success, data: { source_path, signal_name, target_path, method_name } }

	var scene_path: String = params.get("scene_path", "")
	var source_path: String = params.get("source_path", "")
	var signal_name: String = params.get("signal_name", "")
	var target_path: String = params.get("target_path", "")
	var method_name: String = params.get("method_name", "")
	var flags: int = params.get("flags", 0)

	if scene_path == "":
		return OpsUtil._error("scene_path is required", "signal_connect", params)
	if source_path == "":
		return OpsUtil._error("source_path is required", "signal_connect", params)
	if signal_name == "":
		return OpsUtil._error("signal_name is required", "signal_connect", params)
	if target_path == "":
		return OpsUtil._error("target_path is required", "signal_connect", params)
	if method_name == "":
		return OpsUtil._error("method_name is required", "signal_connect", params)

	var source: Node = edit.resolve(source_path)
	if source == null:
		return OpsUtil._error("Source node not found: " + source_path, "signal_connect", {"scene_path": scene_path, "source_path": source_path})

	var target: Node = edit.resolve(target_path)
	if target == null:
		return OpsUtil._error("Target node not found: " + target_path, "signal_connect", {"scene_path": scene_path, "target_path": target_path})

	# Validate signal exists on source node
	var signal_exists := false
	for sig in source.get_signal_list():
		if sig["name"] == signal_name:
			signal_exists = true
			break
	if not signal_exists:
		var source_class: String = source.get_class()

		return OpsUtil._error(
			"Signal '" + signal_name + "' not found on " + source_class,
			"signal_connect",
			{"source_path": source_path, "signal_name": signal_name}
		)

	# Ensure CONNECT_PERSIST (flag=2) for scene serialization
	flags = flags | 2  # CONNECT_PERSIST = 2

	var callable := Callable(target, method_name)

	# Apply binds if provided
	var raw_binds = params.get("binds", null)
	if raw_binds != null and raw_binds is Array and not raw_binds.is_empty():
		callable = callable.bindv(raw_binds)

	var connect_err := source.connect(signal_name, callable, flags)
	if connect_err != OK:
		return OpsUtil._error(
			"Failed to connect '%s' (error %d). Godot 4.7+ validates bound callables at connect time — does '%s' exist on the target?" % [signal_name, connect_err, method_name],
			"signal_connect",
			{"source_path": source_path, "signal_name": signal_name, "method_name": method_name})

	return {"success": true, "data": {
		"source_path": source_path,
		"signal_name": signal_name,
		"target_path": target_path,
		"method_name": method_name,
	}}


static func op_signal_disconnect(params: Dictionary, edit: SceneEdit) -> Dictionary:
	## Remove a signal connection from a scene.
	##
	## Params:
	##   scene_path: String
	##   source_path: String
	##   signal_name: String
	##   target_path: String
	##   method_name: String
	##
	## Returns: { success, data: { source_path, signal_name, target_path, method_name } }

	var scene_path: String = params.get("scene_path", "")
	var source_path: String = params.get("source_path", "")
	var signal_name: String = params.get("signal_name", "")
	var target_path: String = params.get("target_path", "")
	var method_name: String = params.get("method_name", "")

	if scene_path == "":
		return OpsUtil._error("scene_path is required", "signal_disconnect", params)
	if source_path == "":
		return OpsUtil._error("source_path is required", "signal_disconnect", params)
	if signal_name == "":
		return OpsUtil._error("signal_name is required", "signal_disconnect", params)
	if target_path == "":
		return OpsUtil._error("target_path is required", "signal_disconnect", params)
	if method_name == "":
		return OpsUtil._error("method_name is required", "signal_disconnect", params)

	var source: Node = edit.resolve(source_path)
	if source == null:
		return OpsUtil._error("Source node not found: " + source_path, "signal_disconnect", {"scene_path": scene_path, "source_path": source_path})

	var target: Node = edit.resolve(target_path)
	if target == null:
		return OpsUtil._error("Target node not found: " + target_path, "signal_disconnect", {"scene_path": scene_path, "target_path": target_path})

	var matching: Array = []
	if source.has_signal(signal_name):
		for connection in source.get_signal_connection_list(signal_name):
			if connection.callable.get_object() == target and connection.callable.get_method() == method_name:
				matching.append(connection.callable)
	if matching.is_empty():
		return OpsUtil._error("Connection does not exist", "signal_disconnect", params)
	for callable in matching:
		source.disconnect(signal_name, callable)

	return {"success": true, "data": {
		"source_path": source_path,
		"signal_name": signal_name,
		"target_path": target_path,
		"method_name": method_name,
	}}


static func op_signal_list(params: Dictionary, edit: SceneEdit) -> Dictionary:
	var connections: Array = []
	_collect_connections(edit.root, edit.root, str(params.get("node_path", "")), connections)
	return {"success": true, "data": {"connections": connections}}

static func _collect_connections(root: Node, node: Node, filter: String, connections: Array) -> void:
	for signal_info in node.get_signal_list():
		for connection in node.get_signal_connection_list(signal_info.name):
			if not connection.flags & CONNECT_PERSIST:
				continue
			var target = connection.callable.get_object()
			if not target is Node or (target != root and not root.is_ancestor_of(target)):
				continue
			var source_path := str(root.get_path_to(node))
			var target_path := str(root.get_path_to(target))
			if filter != "" and filter != source_path and filter != target_path:
				continue
			connections.append({"source_path": source_path, "signal_name": signal_info.name, "target_path": target_path, "method_name": connection.callable.get_method(), "flags": connection.flags, "binds": connection.callable.get_bound_arguments()})
	for child in node.get_children():
		_collect_connections(root, child, filter, connections)
