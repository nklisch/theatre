## Tests keyboard shortcut handlers in runtime.gd.
##
## F8 = toggle recording, F9 = add marker, F10 = toggle pause.
extends RefCounted

var _root: Window


func setup(root: Window) -> void:
	_root = root


func _make_key_event(keycode: Key) -> InputEventKey:
	var event := InputEventKey.new()
	event.keycode = keycode
	event.pressed = true
	return event


func test_f10_toggles_pause() -> String:
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var tree := _root.get_tree()
	var was_paused := tree.paused

	# Simulate F10 keypress
	rt._shortcut_input(_make_key_event(KEY_F10))

	var err := Assert.eq(tree.paused, not was_paused, "pause toggled by F10")

	# Restore
	tree.paused = was_paused
	rt.queue_free()
	return err


func test_f10_toggles_pause_twice() -> String:
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var tree := _root.get_tree()
	var original := tree.paused

	rt._shortcut_input(_make_key_event(KEY_F10))
	rt._shortcut_input(_make_key_event(KEY_F10))

	var err := Assert.eq(tree.paused, original, "double F10 restores pause state")

	tree.paused = original
	rt.queue_free()
	return err


func test_f8_toggles_recording() -> String:
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var recorder = rt.get("recorder")
	if not recorder or not is_instance_valid(recorder):
		rt.queue_free()
		return "runtime has no recorder — cannot test F8"

	var was_recording := recorder.is_recording()

	# F8 should start recording if not recording
	rt._shortcut_input(_make_key_event(KEY_F8))
	await _root.get_tree().process_frame

	var is_recording := recorder.is_recording()

	# Clean up
	if is_recording:
		recorder.stop_recording()
	rt.queue_free()

	return Assert.eq(is_recording, not was_recording, "F8 toggled recording")


func test_f9_adds_marker_when_recording() -> String:
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var recorder = rt.get("recorder")
	if not recorder or not is_instance_valid(recorder):
		rt.queue_free()
		return "runtime has no recorder — cannot test F9"

	# Start recording first
	rt._shortcut_input(_make_key_event(KEY_F8))
	await _root.get_tree().process_frame

	if not recorder.is_recording():
		rt.queue_free()
		return "F8 did not start recording — cannot test F9"

	var marker_fired := false
	recorder.marker_added.connect(func(_f, _s, _l): marker_fired = true)

	# F9 drops a marker
	rt._shortcut_input(_make_key_event(KEY_F9))
	await _root.get_tree().process_frame

	recorder.stop_recording()
	rt.queue_free()

	return Assert.true_(marker_fired, "F9 added a marker")
