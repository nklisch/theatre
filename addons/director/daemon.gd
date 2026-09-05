extends SceneTree

## Director headless daemon — persistent TCP command server.
## Launched via: godot --headless --path <project> --script addons/director/daemon.gd

const MessageCodec = preload("res://addons/director/message_codec.gd")
const Dispatcher = preload("res://addons/director/ops/dispatcher.gd")

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
	var decode_result = MessageCodec.try_decode(_read_buf)
	var msg: Dictionary = decode_result[0]
	var bytes_consumed: int = decode_result[1]
	if bytes_consumed > 0:
		_read_buf = _read_buf.slice(bytes_consumed)
	if msg.is_empty():
		return

	var operation: String = msg.get("operation", "")

	if operation == "quit":
		_client.put_data(MessageCodec.encode({"success": true, "data": {"status": "shutdown"}, "operation": "quit"}))
		print(JSON.stringify({"source": "director", "status": "shutdown"}))
		quit(0)
		return

	# Reset idle timer on any non-ping operation.
	if operation != "ping":
		_idle_time = 0.0

	var params: Dictionary = msg.get("params", {})
	var result = _dispatch(operation, params)
	_client.put_data(MessageCodec.encode(result))


func _dispatch(operation: String, params: Dictionary) -> Dictionary:
	if operation == "ping":
		return {"success": true, "data": {"status": "ok", "backend": "daemon", "project_path": ProjectSettings.globalize_path("res://"), "process_id": OS.get_process_id()}, "operation": "ping"}
	return Dispatcher.dispatch(operation, params)


func _check_idle_timeout(delta: float) -> void:
	_idle_time += delta
	if _idle_time >= IDLE_TIMEOUT_SEC:
		print(JSON.stringify({"source": "director", "status": "idle_shutdown"}))
		quit(0)
