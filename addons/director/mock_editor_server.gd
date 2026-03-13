extends SceneTree

## Mock editor plugin server for E2E testing.
##
## Runs headlessly with the same TCP protocol as plugin.gd,
## but delegates to regular ops/ (no EditorInterface available in headless).
## Used by EditorFixture in tests to validate the Rust TCP client
## and backend selection logic without requiring the actual Godot editor.

const MessageCodec = preload("res://addons/director/message_codec.gd")
const SceneOps = preload("res://addons/director/ops/scene_ops.gd")
const NodeOps = preload("res://addons/director/ops/node_ops.gd")
const ResourceOps = preload("res://addons/director/ops/resource_ops.gd")
const TileMapOps = preload("res://addons/director/ops/tilemap_ops.gd")
const GridMapOps = preload("res://addons/director/ops/gridmap_ops.gd")
const AnimationOps = preload("res://addons/director/ops/animation_ops.gd")

const DEFAULT_PORT := 6551

var _server: TCPServer
var _client: StreamPeerTCP
var _read_buf: PackedByteArray = PackedByteArray()
var _port: int


func _init():
	_port = int(OS.get_environment("DIRECTOR_EDITOR_PORT")) \
		if OS.has_environment("DIRECTOR_EDITOR_PORT") \
		else DEFAULT_PORT

	_server = TCPServer.new()
	var err = _server.listen(_port)
	if err != OK:
		printerr("Mock editor: failed to listen on port %d (error %d)" % [_port, err])
		quit(1)
		return

	print(JSON.stringify({"source": "director", "status": "ready", "port": _port, "backend": "mock_editor"}))


func _process(_delta: float) -> bool:
	_accept_client()
	_poll_client()
	return false


func _accept_client() -> void:
	if not _server.is_connection_available():
		return
	if _client != null and _client.get_status() == StreamPeerTCP.STATUS_CONNECTED:
		_client.disconnect_from_host()
	_client = _server.take_connection()
	_read_buf.clear()


func _poll_client() -> void:
	if _client == null:
		return
	_client.poll()

	var status = _client.get_status()
	if status == StreamPeerTCP.STATUS_NONE or status == StreamPeerTCP.STATUS_ERROR:
		_client = null
		_read_buf.clear()
		return
	if status != StreamPeerTCP.STATUS_CONNECTED:
		return

	var available = _client.get_available_bytes()
	if available > 0:
		var res = _client.get_data(available)
		if res[0] == OK:
			_read_buf.append_array(res[1] as PackedByteArray)

	var decode_result = MessageCodec.try_decode(_read_buf)
	var msg: Dictionary = decode_result[0]
	var bytes_consumed: int = decode_result[1]
	if bytes_consumed > 0:
		_read_buf = _read_buf.slice(bytes_consumed)
	if msg.is_empty():
		return

	var operation: String = msg.get("operation", "")
	var params: Dictionary = msg.get("params", {})

	var result = _dispatch(operation, params)
	_client.put_data(MessageCodec.encode(result))


func _dispatch(operation: String, params: Dictionary) -> Dictionary:
	match operation:
		"scene_create": return SceneOps.op_scene_create(params)
		"scene_read": return SceneOps.op_scene_read(params)
		"node_add": return NodeOps.op_node_add(params)
		"node_set_properties": return NodeOps.op_node_set_properties(params)
		"node_remove": return NodeOps.op_node_remove(params)
		"node_reparent": return NodeOps.op_node_reparent(params)
		"scene_list": return SceneOps.op_scene_list(params)
		"scene_add_instance": return SceneOps.op_scene_add_instance(params)
		"resource_read": return ResourceOps.op_resource_read(params)
		"material_create": return ResourceOps.op_material_create(params)
		"shape_create": return ResourceOps.op_shape_create(params)
		"style_box_create": return ResourceOps.op_style_box_create(params)
		"resource_duplicate": return ResourceOps.op_resource_duplicate(params)
		"tilemap_set_cells": return TileMapOps.op_tilemap_set_cells(params)
		"tilemap_get_cells": return TileMapOps.op_tilemap_get_cells(params)
		"tilemap_clear": return TileMapOps.op_tilemap_clear(params)
		"gridmap_set_cells": return GridMapOps.op_gridmap_set_cells(params)
		"gridmap_get_cells": return GridMapOps.op_gridmap_get_cells(params)
		"gridmap_clear": return GridMapOps.op_gridmap_clear(params)
		"animation_create": return AnimationOps.op_animation_create(params)
		"animation_add_track": return AnimationOps.op_animation_add_track(params)
		"animation_read": return AnimationOps.op_animation_read(params)
		"animation_remove_track": return AnimationOps.op_animation_remove_track(params)
		"ping":
			return {"success": true, "data": {"status": "ok", "backend": "editor"}, "operation": "ping"}
		_:
			return {
				"success": false,
				"error": "Unknown operation: " + operation,
				"operation": operation,
				"context": {},
			}
