## Tests that runtime.gd initializes correctly and wires its components.
##
## Validates the cross-component wiring that's required for the addon to
## function: collector → tcp_server, recorder → tcp_server, static instance var.
extends RefCounted

var _root: Window


func setup(root: Window) -> void:
	_root = root


func test_runtime_loads() -> String:
	var script: GDScript = load("res://addons/spectator/runtime.gd")
	return Assert.not_null(script, "runtime.gd loads")


func test_runtime_creates_children() -> String:
	var rt = load("res://addons/spectator/runtime.gd").new()
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
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var server = rt.get("tcp_server")
	var err := ""
	if server:
		err = Assert.eq(server.get_connection_status(), "waiting",
			"server listening after runtime._ready()")
	else:
		err = "tcp_server is null"

	rt.queue_free()
	return err


func test_runtime_static_instance_set() -> String:
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var script: GDScript = load("res://addons/spectator/runtime.gd")
	var inst = script.get("instance")
	var err := Assert.eq(inst, rt, "static instance points to runtime node")

	rt.queue_free()
	return err


func test_runtime_clears_instance_on_exit() -> String:
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	rt.queue_free()
	await _root.get_tree().process_frame

	var script: GDScript = load("res://addons/spectator/runtime.gd")
	var inst = script.get("instance")
	return Assert.is_null(inst, "static instance cleared after exit")


func test_runtime_stops_server_on_exit() -> String:
	var rt = load("res://addons/spectator/runtime.gd").new()
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
