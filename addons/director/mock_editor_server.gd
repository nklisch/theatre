extends SceneTree

## Mock editor plugin server for E2E testing.
##
## Runs headlessly with the same TCP protocol as plugin.gd,
## but delegates to regular ops/ (no EditorInterface available in headless).
## Used by EditorFixture in tests to validate the Rust TCP client
## and backend selection logic without requiring the actual Godot editor.

const MessageCodec = preload("res://addons/director/message_codec.gd")
const Dispatcher = preload("res://addons/director/ops/dispatcher.gd")

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
	if operation == "ping":
		return {"success": true, "data": {"status": "ok", "backend": "editor", "project_path": ProjectSettings.globalize_path("res://"), "process_id": OS.get_process_id()}, "operation": "ping"}
	return Dispatcher.dispatch(operation, params)
