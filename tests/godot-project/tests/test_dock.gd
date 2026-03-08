## Tests the dock's ability to acquire the runtime and interact with it.
##
## This validates the cross-component wiring that's broken when the dock
## runs in the editor process but the runtime runs in the game process.
## In a headless test, both run in the same process — so acquisition must succeed.
extends RefCounted

var _root: Window


func setup(root: Window) -> void:
	_root = root


func test_dock_instantiates() -> String:
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	return Assert.not_null(dock, "dock instantiates")


func test_dock_finds_runtime_in_same_process() -> String:
	# Start runtime first (simulates the game process)
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	# Create dock in the same tree (simulates same-process scenario)
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		rt.queue_free()
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	# Trigger the dock's runtime acquisition
	if dock.has_method("_try_acquire_runtime"):
		dock.call("_try_acquire_runtime")

	# Check if dock found the server
	var server = dock.get("_tcp_server")
	var err: String
	if server and is_instance_valid(server):
		err = ""  # pass — dock can find runtime when in same process
	else:
		err = "dock failed to acquire tcp_server from runtime"

	dock.queue_free()
	rt.queue_free()
	return err


func test_dock_without_runtime_does_not_crash() -> String:
	## Verifies that the dock's acquire-runtime path returns cleanly when no
	## runtime is present — simulating the editor process where runtime.instance
	## is null. The original bug: every button handler silently early-returned.
	## This test confirms the dock at least doesn't panic or crash.
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	# No runtime added — static instance should be null.
	var script: GDScript = load("res://addons/spectator/runtime.gd")
	var inst = script.get("instance")
	if inst != null:
		dock.queue_free()
		return "runtime.instance is not null — test precondition failed"

	# Calling acquire when no runtime is available must not crash.
	if dock.has_method("_try_acquire_runtime"):
		dock.call("_try_acquire_runtime")

	# Dock should report no server (not acquired).
	var server = dock.get("_tcp_server")
	dock.queue_free()
	# server being null/invalid is the correct result here.
	if server and is_instance_valid(server):
		return "dock acquired a server despite no runtime — unexpected"
	return ""


func test_dock_record_button_calls_recorder() -> String:
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		rt.queue_free()
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	if dock.has_method("_try_acquire_runtime"):
		dock.call("_try_acquire_runtime")

	var recorder = dock.get("_recorder")
	if not recorder or not is_instance_valid(recorder):
		dock.queue_free()
		rt.queue_free()
		return "dock never acquired recorder — button would do nothing"

	# Simulate pressing record
	if dock.has_method("_on_record_pressed"):
		dock.call("_on_record_pressed")
	await _root.get_tree().process_frame

	var is_recording: bool = recorder.is_recording()

	# Clean up
	if is_recording:
		recorder.stop_recording()
	dock.queue_free()
	rt.queue_free()

	return Assert.true_(is_recording, "recording started after button press")


func test_dock_stop_button_stops_recording() -> String:
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		rt.queue_free()
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	if dock.has_method("_try_acquire_runtime"):
		dock.call("_try_acquire_runtime")

	var recorder = dock.get("_recorder")
	if not recorder or not is_instance_valid(recorder):
		dock.queue_free()
		rt.queue_free()
		return "dock never acquired recorder"

	if dock.has_method("_on_record_pressed"):
		dock.call("_on_record_pressed")
	await _root.get_tree().process_frame

	if dock.has_method("_on_stop_pressed"):
		dock.call("_on_stop_pressed")
	await _root.get_tree().process_frame

	var still_recording: bool = recorder.is_recording()
	dock.queue_free()
	rt.queue_free()

	return Assert.false_(still_recording, "recording stopped after stop button")


func test_dock_marker_button_adds_marker() -> String:
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		rt.queue_free()
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	if dock.has_method("_try_acquire_runtime"):
		dock.call("_try_acquire_runtime")

	var recorder = dock.get("_recorder")
	if not recorder or not is_instance_valid(recorder):
		dock.queue_free()
		rt.queue_free()
		return "dock never acquired recorder"

	# Must be recording to add marker
	if dock.has_method("_on_record_pressed"):
		dock.call("_on_record_pressed")
	await _root.get_tree().process_frame

	var marker_fired := false
	recorder.marker_added.connect(func(_f, _s, _l): marker_fired = true)

	if dock.has_method("_on_marker_pressed"):
		dock.call("_on_marker_pressed")
	await _root.get_tree().process_frame

	if recorder.is_recording():
		recorder.stop_recording()
	dock.queue_free()
	rt.queue_free()

	return Assert.true_(marker_fired, "marker_added signal fired")
