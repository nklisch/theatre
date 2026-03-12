extends SceneTree

## Director headless daemon — persistent TCP command server.
## Launched via: godot --headless --path <project> --script addons/director/daemon.gd

const SceneOps = preload("res://addons/director/ops/scene_ops.gd")
const NodeOps = preload("res://addons/director/ops/node_ops.gd")
const ResourceOps = preload("res://addons/director/ops/resource_ops.gd")
const TileMapOps = preload("res://addons/director/ops/tilemap_ops.gd")
const GridMapOps = preload("res://addons/director/ops/gridmap_ops.gd")
const AnimationOps = preload("res://addons/director/ops/animation_ops.gd")
const PhysicsOps = preload("res://addons/director/ops/physics_ops.gd")
const ShaderOps = preload("res://addons/director/ops/shader_ops.gd")
const MetaOps = preload("res://addons/director/ops/meta_ops.gd")
const ProjectOps = preload("res://addons/director/ops/project_ops.gd")

const DEFAULT_PORT := 6550
const IDLE_TIMEOUT_SEC := 300  # 5 minutes

var _server: TCPServer
var _client: StreamPeerTCP
var _read_buf: PackedByteArray = PackedByteArray()
var _idle_time: float = 0.0
var _port: int


func _init():
	_port = int(OS.get_environment("DIRECTOR_DAEMON_PORT")) \
		if OS.has_environment("DIRECTOR_DAEMON_PORT") \
		else DEFAULT_PORT

	_server = TCPServer.new()
	var err = _server.listen(_port)
	if err != OK:
		printerr(JSON.stringify({
			"source": "director",
			"status": "error",
			"error": "Failed to listen on port %d (error %d)" % [_port, err],
		}))
		quit(1)
		return

	print(JSON.stringify({"source": "director", "status": "ready", "port": _port}))


func _process(delta: float) -> bool:
	_accept_client()
	_poll_client()
	_check_idle_timeout(delta)
	return false


func _accept_client() -> void:
	if not _server.is_connection_available():
		return

	# Disconnect any existing client before accepting the new one.
	if _client != null and _client.get_status() == StreamPeerTCP.STATUS_CONNECTED:
		_client.disconnect_from_host()

	_client = _server.take_connection()
	_read_buf.clear()
	_idle_time = 0.0  # Reset on connect


func _poll_client() -> void:
	if _client == null:
		return

	_client.poll()

	var status = _client.get_status()
	if status == StreamPeerTCP.STATUS_NONE or status == StreamPeerTCP.STATUS_ERROR:
		_client = null
		_read_buf.clear()
		_idle_time = 0.0  # Reset on disconnect
		return

	if status != StreamPeerTCP.STATUS_CONNECTED:
		return

	# Drain all available bytes into the read buffer.
	var available = _client.get_available_bytes()
	if available > 0:
		var res = _client.get_data(available)
		if res[0] == OK:
			_read_buf.append_array(res[1] as PackedByteArray)

	# Try to decode and handle one message per frame.
	var msg = _try_decode_message()
	if msg.is_empty():
		return

	var operation: String = msg.get("operation", "")

	if operation == "quit":
		_send_message({"success": true, "data": {"status": "shutdown"}, "operation": "quit"})
		print(JSON.stringify({"source": "director", "status": "shutdown"}))
		quit(0)
		return

	# Reset idle timer on any non-ping operation.
	if operation != "ping":
		_idle_time = 0.0

	var params: Dictionary = msg.get("params", {})
	var result = _dispatch(operation, params)
	_send_message(result)


func _try_decode_message() -> Dictionary:
	# Need at least 4 bytes for the length prefix.
	if _read_buf.size() < 4:
		return {}

	# Decode big-endian u32 from the first 4 bytes.
	var msg_len: int = (_read_buf[0] << 24) | (_read_buf[1] << 16) | (_read_buf[2] << 8) | _read_buf[3]

	if msg_len == 0:
		# Consume the 4 zero bytes and return empty (malformed message).
		_read_buf = _read_buf.slice(4)
		return {}

	# Wait until the full message body is buffered.
	if _read_buf.size() < 4 + msg_len:
		return {}

	# Extract the JSON body and advance the buffer.
	var msg_bytes: PackedByteArray = _read_buf.slice(4, 4 + msg_len)
	_read_buf = _read_buf.slice(4 + msg_len)

	var json_str = msg_bytes.get_string_from_utf8()
	var json = JSON.new()
	if json.parse(json_str) != OK:
		return {}

	var data = json.get_data()
	if typeof(data) != TYPE_DICTIONARY:
		return {}

	return data


func _send_message(data: Dictionary) -> void:
	var json_str = JSON.stringify(data)
	var json_bytes: PackedByteArray = json_str.to_utf8_buffer()
	var msg_len = json_bytes.size()

	var len_bytes = PackedByteArray([
		(msg_len >> 24) & 0xFF,
		(msg_len >> 16) & 0xFF,
		(msg_len >> 8) & 0xFF,
		msg_len & 0xFF,
	])

	_client.put_data(len_bytes)
	_client.put_data(json_bytes)


func _dispatch(operation: String, params: Dictionary) -> Dictionary:
	match operation:
		"scene_create":
			return SceneOps.op_scene_create(params)
		"scene_read":
			return SceneOps.op_scene_read(params)
		"node_add":
			return NodeOps.op_node_add(params)
		"node_set_properties":
			return NodeOps.op_node_set_properties(params)
		"node_remove":
			return NodeOps.op_node_remove(params)
		"node_reparent":
			return NodeOps.op_node_reparent(params)
		"scene_list":
			return SceneOps.op_scene_list(params)
		"scene_add_instance":
			return SceneOps.op_scene_add_instance(params)
		"resource_read":
			return ResourceOps.op_resource_read(params)
		"material_create":
			return ResourceOps.op_material_create(params)
		"shape_create":
			return ResourceOps.op_shape_create(params)
		"style_box_create":
			return ResourceOps.op_style_box_create(params)
		"resource_duplicate":
			return ResourceOps.op_resource_duplicate(params)
		"tilemap_set_cells":
			return TileMapOps.op_tilemap_set_cells(params)
		"tilemap_get_cells":
			return TileMapOps.op_tilemap_get_cells(params)
		"tilemap_clear":
			return TileMapOps.op_tilemap_clear(params)
		"gridmap_set_cells":
			return GridMapOps.op_gridmap_set_cells(params)
		"gridmap_get_cells":
			return GridMapOps.op_gridmap_get_cells(params)
		"gridmap_clear":
			return GridMapOps.op_gridmap_clear(params)
		"animation_create":
			return AnimationOps.op_animation_create(params)
		"animation_add_track":
			return AnimationOps.op_animation_add_track(params)
		"animation_read":
			return AnimationOps.op_animation_read(params)
		"animation_remove_track":
			return AnimationOps.op_animation_remove_track(params)
		"physics_set_layers":
			return PhysicsOps.op_physics_set_layers(params)
		"physics_set_layer_names":
			return PhysicsOps.op_physics_set_layer_names(params)
		"visual_shader_create":
			return ShaderOps.op_visual_shader_create(params)
		"batch":
			return MetaOps.op_batch(params)
		"scene_diff":
			return MetaOps.op_scene_diff(params)
		"uid_get":
			return ProjectOps.op_uid_get(params)
		"uid_update_project":
			return ProjectOps.op_uid_update_project(params)
		"export_mesh_library":
			return ProjectOps.op_export_mesh_library(params)
		"ping":
			return {"success": true, "data": {"status": "ok"}, "operation": "ping"}
		_:
			return {
				"success": false,
				"error": "Unknown operation: " + operation,
				"operation": operation,
				"context": {},
			}


func _check_idle_timeout(delta: float) -> void:
	_idle_time += delta
	if _idle_time >= IDLE_TIMEOUT_SEC:
		print(JSON.stringify({"source": "director", "status": "idle_shutdown"}))
		quit(0)
