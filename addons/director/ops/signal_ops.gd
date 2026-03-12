class_name SignalOps

const NodeOps = preload("res://addons/director/ops/node_ops.gd")
const OpsUtil = preload("res://addons/director/ops/ops_util.gd")


static func op_signal_connect(params: Dictionary) -> Dictionary:
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

	var full_path = "res://" + scene_path
	if not ResourceLoader.exists(full_path):
		return OpsUtil._error("Scene not found: " + scene_path, "signal_connect", {"scene_path": scene_path})

	var packed: PackedScene = load(full_path)
	var root = packed.instantiate()

	var source: Node = _resolve_node(root, source_path)
	if source == null:
		root.free()
		return OpsUtil._error("Source node not found: " + source_path, "signal_connect", {"scene_path": scene_path, "source_path": source_path})

	var target: Node = _resolve_node(root, target_path)
	if target == null:
		root.free()
		return OpsUtil._error("Target node not found: " + target_path, "signal_connect", {"scene_path": scene_path, "target_path": target_path})

	# Validate signal exists on source node
	var signal_exists := false
	for sig in source.get_signal_list():
		if sig["name"] == signal_name:
			signal_exists = true
			break
	if not signal_exists:
		root.free()
		return OpsUtil._error(
			"Signal '" + signal_name + "' not found on " + source.get_class(),
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

	source.connect(signal_name, callable, flags)

	var save_result = NodeOps._repack_and_save(root, full_path)
	root.free()
	if not save_result.success:
		return save_result

	return {"success": true, "data": {
		"source_path": source_path,
		"signal_name": signal_name,
		"target_path": target_path,
		"method_name": method_name,
	}}


static func op_signal_disconnect(params: Dictionary) -> Dictionary:
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

	var full_path = "res://" + scene_path
	if not ResourceLoader.exists(full_path):
		return OpsUtil._error("Scene not found: " + scene_path, "signal_disconnect", {"scene_path": scene_path})

	var packed: PackedScene = load(full_path)
	var root = packed.instantiate()

	var source: Node = _resolve_node(root, source_path)
	if source == null:
		root.free()
		return OpsUtil._error("Source node not found: " + source_path, "signal_disconnect", {"scene_path": scene_path, "source_path": source_path})

	var target: Node = _resolve_node(root, target_path)
	if target == null:
		root.free()
		return OpsUtil._error("Target node not found: " + target_path, "signal_disconnect", {"scene_path": scene_path, "target_path": target_path})

	var callable := Callable(target, method_name)
	if not source.is_connected(signal_name, callable):
		root.free()
		return OpsUtil._error(
			"Connection does not exist: " + source_path + "." + signal_name + " → " + target_path + "." + method_name,
			"signal_disconnect",
			{"source_path": source_path, "signal_name": signal_name, "target_path": target_path, "method_name": method_name}
		)

	source.disconnect(signal_name, callable)

	var save_result = NodeOps._repack_and_save(root, full_path)
	root.free()
	if not save_result.success:
		return save_result

	return {"success": true, "data": {
		"source_path": source_path,
		"signal_name": signal_name,
		"target_path": target_path,
		"method_name": method_name,
	}}


static func op_signal_list(params: Dictionary) -> Dictionary:
	## List all signal connections in a scene.
	##
	## Params:
	##   scene_path: String
	##   node_path: String? (optional — filter to connections from/to this node)
	##
	## Returns: { success, data: { connections: [{ source_path, signal_name,
	##           target_path, method_name, flags }] } }

	var scene_path: String = params.get("scene_path", "")
	var node_path_filter = params.get("node_path", null)

	if scene_path == "":
		return OpsUtil._error("scene_path is required", "signal_list", params)

	var full_path = "res://" + scene_path
	if not ResourceLoader.exists(full_path):
		return OpsUtil._error("Scene not found: " + scene_path, "signal_list", {"scene_path": scene_path})

	var packed: PackedScene = load(full_path)
	var state: SceneState = packed.get_state()

	var connections: Array = []
	var count: int = state.get_connection_count()

	for i in range(count):
		var src_path: String = str(state.get_connection_source(i))
		var sig_name: String = state.get_connection_signal(i)
		var tgt_path: String = str(state.get_connection_target(i))
		var meth_name: String = state.get_connection_method(i)
		var conn_flags: int = state.get_connection_flags(i)

		# Normalize paths: SceneState returns NodePath, remove leading "./"
		src_path = src_path.trim_prefix("./")
		if src_path == "":
			src_path = "."
		tgt_path = tgt_path.trim_prefix("./")
		if tgt_path == "":
			tgt_path = "."

		# Apply node_path filter if provided
		if node_path_filter != null and node_path_filter != "":
			var filter: String = node_path_filter
			if src_path != filter and tgt_path != filter:
				continue

		connections.append({
			"source_path": src_path,
			"signal_name": sig_name,
			"target_path": tgt_path,
			"method_name": meth_name,
			"flags": conn_flags,
		})

	return {"success": true, "data": {"connections": connections}}


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

static func _resolve_node(root: Node, path: String) -> Node:
	if path == "" or path == ".":
		return root
	return root.get_node_or_null(NodePath(path))
