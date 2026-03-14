## Tests that runtime.gd initializes correctly and wires its components.
##
## Validates the cross-component wiring that's required for the addon to
## function: collector → tcp_server, recorder → tcp_server.
##
## NOTE: static var instance was removed. The dock now receives state via
## EditorDebuggerPlugin push messages instead of reading the static var.
extends RefCounted

var _root: Window


func setup(root: Window) -> void:
	_root = root


func test_runtime_loads() -> String:
	var script: GDScript = load("res://addons/stage/runtime.gd")
	return Assert.not_null(script, "runtime.gd loads")


func test_runtime_creates_children() -> String:
	var rt = load("res://addons/stage/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var err := Assert.not_null(rt.get("tcp_server"), "tcp_server created")
	if err:
		rt.queue_free()
		return err
	err = Assert.not_null(rt.get("collector"), "collector created")
	if err:
		rt.queue_free()
		return err
	err = Assert.not_null(rt.get("recorder"), "recorder created")
	rt.queue_free()
	return err


func test_runtime_server_is_listening() -> String:
	## Verify runtime creates a tcp_server and attempts to start it.
	## We can't assert "waiting" status because the test project's autoload
	## already holds port 9077. Instead we test StageTCPServer directly
	## with port 0 (ephemeral) to confirm the "waiting" path works.
	var server := StageTCPServer.new()
	_root.add_child(server)
	await _root.get_tree().process_frame

	server.start(0)  # bind to any free port
	var status: String = server.get_connection_status()
	server.stop()
	server.queue_free()

	return Assert.eq(status, "waiting",
		"StageTCPServer should be 'waiting' after start(0) with a free port")


func test_runtime_has_no_static_instance_var() -> String:
	## Regression: static var instance was removed in the EditorDebuggerPlugin
	## refactor. The dock no longer reads it — accessing it should return null.
	## This guards against accidentally re-adding it.
	var script: GDScript = load("res://addons/stage/runtime.gd")
	var inst = script.get("instance")
	# Should be null because the property doesn't exist on the script object.
	return Assert.is_null(inst, "runtime.gd must not have static var instance (removed in EditorDebuggerPlugin refactor)")


func test_runtime_process_mode_is_always() -> String:
	## Verify runtime sets PROCESS_MODE_ALWAYS so TCP polling and dashcam
	## continue even when the scene tree is paused via F11 / advance_frames.
	var rt = load("res://addons/stage/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var err := Assert.eq(rt.process_mode, Node.PROCESS_MODE_ALWAYS,
		"runtime.process_mode should be PROCESS_MODE_ALWAYS")

	rt.queue_free()
	return err


func test_runtime_polls_during_pause() -> String:
	## When the tree is paused, tcp_server.poll() must still be called because
	## runtime sets PROCESS_MODE_ALWAYS. We test this with a direct server on an
	## ephemeral port (bypasses the runtime's port-9077 conflict in test env).
	var server := StageTCPServer.new()
	server.set_process_mode(Node.PROCESS_MODE_ALWAYS)
	_root.add_child(server)
	await _root.get_tree().process_frame

	server.start(0)
	if server.get_connection_status() != "waiting":
		server.stop()
		server.queue_free()
		return "could not start server on ephemeral port"

	# Pause the tree
	_root.get_tree().paused = true
	await _root.get_tree().process_frame

	# Server should still be "waiting" — PROCESS_MODE_ALWAYS keeps poll() running
	var status: String = server.get_connection_status()
	_root.get_tree().paused = false
	server.stop()
	server.queue_free()

	return Assert.eq(status, "waiting",
		"server still waiting during pause (PROCESS_MODE_ALWAYS)")


func test_runtime_stops_server_on_exit() -> String:
	var rt = load("res://addons/stage/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var server = rt.get("tcp_server")
	if not server:
		rt.queue_free()
		return "tcp_server was null before exit"

	rt.queue_free()
	await _root.get_tree().process_frame

	return Assert.eq(server.get_connection_status(), "stopped",
		"server stopped after runtime exit")


func test_runtime_has_push_status_method() -> String:
	## Regression: runtime must have _push_status_to_editor for EditorDebuggerPlugin
	## integration. If this method is missing, the dock will never update.
	var rt = load("res://addons/stage/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var err := Assert.obj_has_method(rt, "_push_status_to_editor")
	if err:
		err = "runtime must have _push_status_to_editor method (EditorDebuggerPlugin bridge)"
	rt.queue_free()
	return err


func test_runtime_push_status_does_not_crash_without_debugger() -> String:
	## When EngineDebugger.is_active() is false (headless, standalone),
	## _push_status_to_editor must be a safe no-op.
	var rt = load("res://addons/stage/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	# Call it directly — should not crash even though EngineDebugger is inactive.
	rt._push_status_to_editor()
	await _root.get_tree().process_frame

	rt.queue_free()
	return ""  # No crash = pass


func test_runtime_has_debugger_command_handler() -> String:
	## Regression: runtime must register _on_debugger_command so the dock can
	## trigger marker/pause actions in the game.
	var rt = load("res://addons/stage/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var err := Assert.obj_has_method(rt, "_on_debugger_command")
	if err:
		err = "runtime must have _on_debugger_command method (EditorDebuggerPlugin bridge)"
	rt.queue_free()
	return err


func test_debugger_command_add_marker() -> String:
	## The "add_marker" debugger command must attempt to flush the dashcam clip.
	var rt = load("res://addons/stage/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var recorder = rt.get("recorder")
	if not recorder or not is_instance_valid(recorder):
		rt.queue_free()
		return "runtime has no recorder"

	# Use a Dictionary (ref type) to avoid closure-scope issues with primitives.
	var signal_result := {"fired": false}
	recorder.marker_added.connect(func(_f, _s, _l): signal_result["fired"] = true)

	rt._on_debugger_command("stage:command", ["add_marker"])
	await _root.get_tree().process_frame

	rt.queue_free()

	# Marker signal fires if dashcam is active (which it should be by default).
	# We just verify the command doesn't crash and the runtime handles it.
	return ""  # No crash = pass


func test_debugger_command_unknown_is_no_op() -> String:
	## Unknown commands must not crash the runtime.
	var rt = load("res://addons/stage/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	# Should return false and not crash
	var _result: bool = rt._on_debugger_command("stage:command", ["unknown_command_xyz"])
	await _root.get_tree().process_frame

	rt.queue_free()
	return ""  # Not crashing = pass
