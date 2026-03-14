## Tests keyboard shortcut handlers in runtime.gd.
##
## Default keys: F9 = add marker / save dashcam clip, F11 = toggle pause.
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
	var rt = load("res://addons/stage/runtime.gd").new()
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
	var rt = load("res://addons/stage/runtime.gd").new()
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


func test_editor_shortcut_keys_do_not_trigger_stage() -> String:
	## Regression: F8 (Godot "Stop") and F7 (Godot "Step Into") must NOT
	## drop a marker. They're reserved for the editor and should be
	## no-ops in the runtime's _shortcut_input.
	var rt = load("res://addons/stage/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var recorder = rt.get("recorder")
	if not recorder or not is_instance_valid(recorder):
		rt.queue_free()
		return "runtime has no recorder"

	# F7 should not drop a marker
	var marker_fired := false
	recorder.marker_added.connect(func(_f, _s, _l): marker_fired = true)
	rt._shortcut_input(_make_key_event(KEY_F7))
	await _root.get_tree().process_frame
	var err := Assert.false_(marker_fired, "F7 must not drop a marker")

	rt.queue_free()
	return err


func test_default_keycode_fields_match_expected() -> String:
	## Regression: verify the runtime's hardcoded default key codes match our
	## chosen safe defaults (not F5-F10 which Godot editor owns).
	var rt = load("res://addons/stage/runtime.gd").new()
	_root.add_child(rt)
	await _root.get_tree().process_frame

	var err := Assert.eq(rt.get("_marker_keycode"), KEY_F9, "_marker_keycode should be F9")
	if err:
		rt.queue_free()
		return err
	err = Assert.eq(rt.get("_pause_keycode"), KEY_F11, "_pause_keycode should be F11")
	rt.queue_free()
	return err
