@tool
extends EditorPlugin

const EditorOps = preload("res://addons/director/editor_ops.gd")

const DEFAULT_PORT := 6551
const SETTING_PATH := "director/connection/editor_port"

var _server: TCPServer
var _client: StreamPeerTCP
var _read_buf: PackedByteArray = PackedByteArray()
var _port: int


func _enter_tree() -> void:
	_register_settings()
	_port = _resolve_port()

	_server = TCPServer.new()
	var err = _server.listen(_port)
	if err != OK:
		printerr("[Director] Failed to listen on port %d (error %d)" % [_port, err])
		return

	print("[Director] Editor plugin listening on port %d" % _port)


func _exit_tree() -> void:
	if _client != null and _client.get_status() == StreamPeerTCP.STATUS_CONNECTED:
		_client.disconnect_from_host()
	_client = null
	if _server != null:
		_server.stop()
	_server = null
	print("[Director] Editor plugin stopped")


func _process(_delta: float) -> void:
	if _server == null:
		return
	_accept_client()
	_poll_client()


func _accept_client() -> void:
	if not _server.is_connection_available():
		return
	# Disconnect existing client before accepting new one.
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

	# Drain available bytes.
	var available = _client.get_available_bytes()
	if available > 0:
		var res = _client.get_data(available)
		if res[0] == OK:
			_read_buf.append_array(res[1] as PackedByteArray)

	# Try to decode one message per frame.
	var msg = _try_decode_message()
	if msg.is_empty():
		return

	var operation: String = msg.get("operation", "")
	var params: Dictionary = msg.get("params", {})

	if operation == "ping":
		_send_message({"success": true, "data": {"status": "ok", "backend": "editor"}, "operation": "ping"})
		return

	var result = EditorOps.dispatch(operation, params)
	_send_message(result)


func _try_decode_message() -> Dictionary:
	# Identical to daemon.gd — length-prefixed JSON decoding.
	if _read_buf.size() < 4:
		return {}
	var msg_len: int = (_read_buf[0] << 24) | (_read_buf[1] << 16) | (_read_buf[2] << 8) | _read_buf[3]
	if msg_len == 0:
		_read_buf = _read_buf.slice(4)
		return {}
	if _read_buf.size() < 4 + msg_len:
		return {}
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
	# Identical to daemon.gd — length-prefixed JSON encoding.
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


func _resolve_port() -> int:
	# 1. Env var
	if OS.has_environment("DIRECTOR_EDITOR_PORT"):
		var val = int(OS.get_environment("DIRECTOR_EDITOR_PORT"))
		if val > 0:
			return val
	# 2. Project setting
	if ProjectSettings.has_setting(SETTING_PATH):
		var val = int(ProjectSettings.get_setting(SETTING_PATH))
		if val > 0:
			return val
	# 3. Default
	return DEFAULT_PORT


func _register_settings() -> void:
	_add_setting(SETTING_PATH, TYPE_INT, DEFAULT_PORT,
		PROPERTY_HINT_RANGE, "1024,65535")


func _add_setting(path: String, type: int, default_value: Variant,
		hint: int = PROPERTY_HINT_NONE, hint_string: String = "") -> void:
	if not ProjectSettings.has_setting(path):
		ProjectSettings.set_setting(path, default_value)
	ProjectSettings.set_initial_value(path, default_value)
	ProjectSettings.add_property_info({
		"name": path,
		"type": type,
		"hint": hint,
		"hint_string": hint_string,
	})
