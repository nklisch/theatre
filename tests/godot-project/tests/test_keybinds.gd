## Tests keyboard shortcut handlers in runtime.gd.
##
## Default keys: F12 = toggle recording, F9 = add marker, F11 = toggle pause.
## These must not conflict with Godot editor shortcuts (F5-F10 are editor-owned).
extends RefCounted

var _root: Window


func setup(root: Window) -> void:
	_root = root


func _make_key_event(keycode: Key) -> InputEventKey:
	var event := InputEventKey.new()
	event.keycode = keycode
	event.pressed = true
	return event


func test_f11_toggles_pause() -> String:
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var tree := _root.get_tree()
	var was_paused := tree.paused

	# Simulate F11 keypress (default pause key — not a Godot editor shortcut)
	rt._shortcut_input(_make_key_event(KEY_F11))

	var err := Assert.eq(tree.paused, not was_paused, "pause toggled by F11")

	# Restore
	tree.paused = was_paused
	rt.queue_free()
	return err


func test_f11_toggles_pause_twice() -> String:
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var tree := _root.get_tree()
	var original := tree.paused

	rt._shortcut_input(_make_key_event(KEY_F11))
	rt._shortcut_input(_make_key_event(KEY_F11))

	var err := Assert.eq(tree.paused, original, "double F11 restores pause state")

	tree.paused = original
	rt.queue_free()
	return err


func test_f12_toggles_recording() -> String:
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var recorder = rt.get("recorder")
	if not recorder or not is_instance_valid(recorder):
		rt.queue_free()
		return "runtime has no recorder — cannot test F12"

	var was_recording: bool = recorder.is_recording()

	# F12 should start recording if not recording (default record key, no editor conflict)
	rt._shortcut_input(_make_key_event(KEY_F12))
	await _root.get_tree().process_frame

	var is_recording: bool = recorder.is_recording()

	# Clean up
	if is_recording:
		recorder.stop_recording()
	rt.queue_free()

	return Assert.eq(is_recording, not was_recording, "F12 toggled recording")


func test_f9_adds_marker_when_recording() -> String:
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var recorder = rt.get("recorder")
	if not recorder or not is_instance_valid(recorder):
		rt.queue_free()
		return "runtime has no recorder — cannot test F9"

	# Start recording first via F12 (record key)
	rt._shortcut_input(_make_key_event(KEY_F12))
	await _root.get_tree().process_frame

	if not recorder.is_recording():
		rt.queue_free()
		return "F12 did not start recording — cannot test F9"

	# Use a Dictionary (ref type) to capture signal result across the closure boundary.
	var signal_result := {"fired": false}
	recorder.marker_added.connect(func(_f, _s, _l): signal_result["fired"] = true)

	# F9 drops a marker (default marker key)
	rt._shortcut_input(_make_key_event(KEY_F9))
	await _root.get_tree().process_frame

	recorder.stop_recording()
	rt.queue_free()

	return Assert.true_(signal_result["fired"], "F9 added a marker")


func test_editor_shortcut_keys_do_not_trigger_spectator() -> String:
	## Regression: F8 (Godot "Stop") and F7 (Godot "Step Into") must NOT start
	## recording or drop a marker. They're reserved for the editor and should be
	## no-ops in the runtime's _shortcut_input.
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var recorder = rt.get("recorder")
	if not recorder or not is_instance_valid(recorder):
		rt.queue_free()
		return "runtime has no recorder"

	# F8 should not start recording
	rt._shortcut_input(_make_key_event(KEY_F8))
	await _root.get_tree().process_frame
	var err := Assert.false_(recorder.is_recording(), "F8 must not start recording")
	if err:
		recorder.stop_recording()
		rt.queue_free()
		return err

	# F7 should not do anything either
	var marker_fired := false
	recorder.marker_added.connect(func(_f, _s, _l): marker_fired = true)
	rt._shortcut_input(_make_key_event(KEY_F7))
	await _root.get_tree().process_frame
	err = Assert.false_(marker_fired, "F7 must not drop a marker")

	rt.queue_free()
	return err


func test_default_keycode_fields_match_expected() -> String:
	## Regression: verify the runtime's hardcoded default key codes match our
	## chosen safe defaults (not F5-F10 which Godot editor owns).
	var rt = load("res://addons/spectator/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var err := Assert.eq(rt.get("_record_keycode"), KEY_F12, "_record_keycode should be F12")
	if err:
		rt.queue_free()
		return err
	err = Assert.eq(rt.get("_marker_keycode"), KEY_F9, "_marker_keycode should be F9")
	if err:
		rt.queue_free()
		return err
	err = Assert.eq(rt.get("_pause_keycode"), KEY_F11, "_pause_keycode should be F11")
	rt.queue_free()
	return err
