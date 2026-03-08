## Tests the dock's push-driven API introduced in the EditorDebuggerPlugin refactor.
##
## The dock no longer polls a shared static var — it receives data via three
## public methods called by debugger_plugin.gd when the game pushes messages.
## Dock buttons send commands back via _debugger_plugin.send_command().
##
## All the old _try_acquire_runtime / _tcp_server / _recorder tests are gone
## because those fields were removed. These tests cover the new contract.
extends RefCounted

var _root: Window


## Minimal mock for the debugger plugin — records commands sent by the dock.
class MockDebuggerPlugin extends RefCounted:
	var last_command: String = ""
	var last_args: Array = []
	var commands: Array[String] = []

	func send_command(command: String, args: Array = []) -> void:
		last_command = command
		last_args = args
		commands.append(command)


func setup(root: Window) -> void:
	_root = root


func test_dock_instantiates() -> String:
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	return Assert.not_null(dock, "dock instantiates")


func test_dock_has_no_try_acquire_runtime() -> String:
	## Regression: _try_acquire_runtime was the broken approach (reads static var
	## across process boundary). It must not exist in the new implementation.
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	var has_old_method: bool = dock.has_method("_try_acquire_runtime")
	dock.queue_free()

	if has_old_method:
		return "_try_acquire_runtime still exists — must be removed (broken cross-process approach)"
	return ""


func test_dock_has_no_tcp_server_field() -> String:
	## Regression: the dock must not have a _tcp_server field (was never valid
	## in real editor use because of process isolation).
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	var server = dock.get("_tcp_server")
	dock.queue_free()

	if server != null:
		return "_tcp_server field exists on dock — must be removed"
	return ""


func test_dock_has_no_recorder_field() -> String:
	## Regression: the dock must not have a _recorder field.
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	var recorder = dock.get("_recorder")
	dock.queue_free()

	if recorder != null:
		return "_recorder field exists on dock — must be removed"
	return ""


func test_dock_receive_status_connected() -> String:
	## receive_status("connected", ...) must update the status label to "Connected"
	## and fill in tracking / frame info.
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	dock.receive_status("connected", 9077, 42, 7, 1200, 60)
	await _root.get_tree().process_frame

	var status_label: Label = dock.get_node("%StatusLabel")
	var tracking_label: Label = dock.get_node("%TrackingLabel")
	var frame_label: Label = dock.get_node("%FrameLabel")

	var err := Assert.eq(status_label.text, "Connected", "status label should be 'Connected'")
	if err:
		dock.queue_free()
		return err
	err = Assert.true_(tracking_label.text.contains("42"), "tracking label should show 42 nodes")
	if err:
		dock.queue_free()
		return err
	err = Assert.true_(frame_label.text.contains("1200"), "frame label should show frame 1200")
	dock.queue_free()
	return err


func test_dock_receive_status_stopped() -> String:
	## receive_status("stopped", ...) must show "Stopped" and clear info labels.
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	dock.receive_status("stopped", 9077, 0, 0, 0, 0)
	await _root.get_tree().process_frame

	var status_label: Label = dock.get_node("%StatusLabel")
	var tracking_label: Label = dock.get_node("%TrackingLabel")

	var err := Assert.eq(status_label.text, "Stopped", "status label should be 'Stopped'")
	if err:
		dock.queue_free()
		return err
	err = Assert.true_(tracking_label.text.contains("—"), "tracking should show dash when stopped")
	dock.queue_free()
	return err


func test_dock_receive_status_waiting() -> String:
	## receive_status("waiting", ...) must show "Waiting...".
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	dock.receive_status("waiting", 9077, 0, 0, 0, 0)
	await _root.get_tree().process_frame

	var status_label: Label = dock.get_node("%StatusLabel")
	var err := Assert.eq(status_label.text, "Waiting...", "status label should be 'Waiting...'")
	dock.queue_free()
	return err


func test_dock_receive_activity_adds_entry() -> String:
	## receive_activity(...) must add a new entry to the activity list.
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	var activity_list: VBoxContainer = dock.get_node("%ActivityList")
	var before_count: int = activity_list.get_child_count()

	dock.receive_activity("query", "Snapshot (standard)", "spatial_snapshot", 0)
	await _root.get_tree().process_frame

	var after_count: int = activity_list.get_child_count()
	var err := Assert.true_(after_count > before_count,
		"activity list should have more entries after receive_activity")
	dock.queue_free()
	return err


func test_dock_receive_recording_updates_controls() -> String:
	## receive_recording(true, ...) must enable the stop button and disable the record button.
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	dock.receive_recording(true, 5000, 300, 128)
	await _root.get_tree().process_frame

	var record_btn: Button = dock.get_node("%RecordBtn")
	var stop_btn: Button = dock.get_node("%StopBtn")
	var recording_stats: Label = dock.get_node("%RecordingStats")

	var err := Assert.true_(record_btn.disabled, "record button disabled when recording")
	if err:
		dock.queue_free()
		return err
	err = Assert.false_(stop_btn.disabled, "stop button enabled when recording")
	if err:
		dock.queue_free()
		return err
	err = Assert.true_(recording_stats.visible, "recording stats visible when recording")
	dock.queue_free()
	return err


func test_dock_receive_recording_stopped_updates_controls() -> String:
	## receive_recording(false, ...) must re-enable the record button.
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	# First set recording active, then stop it
	dock.receive_recording(true, 5000, 300, 128)
	await _root.get_tree().process_frame
	dock.receive_recording(false, 0, 300, 0)
	await _root.get_tree().process_frame

	var record_btn: Button = dock.get_node("%RecordBtn")
	var stop_btn: Button = dock.get_node("%StopBtn")

	var err := Assert.false_(record_btn.disabled, "record button re-enabled after recording stops")
	if err:
		dock.queue_free()
		return err
	err = Assert.true_(stop_btn.disabled, "stop button disabled after recording stops")
	dock.queue_free()
	return err


func test_dock_record_button_sends_command() -> String:
	## Regression: pressing Record must call _debugger_plugin.send_command("start_recording").
	## This is the new mechanism since the dock has no direct recorder reference.
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	var mock_plugin := MockDebuggerPlugin.new()
	dock.set("_debugger_plugin", mock_plugin)

	dock.call("_on_record_pressed")
	await _root.get_tree().process_frame

	var err := Assert.eq(mock_plugin.last_command, "start_recording",
		"record button should send 'start_recording' command")
	dock.queue_free()
	return err


func test_dock_stop_button_sends_command() -> String:
	## Regression: pressing Stop must call _debugger_plugin.send_command("stop_recording").
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	var mock_plugin := MockDebuggerPlugin.new()
	dock.set("_debugger_plugin", mock_plugin)

	dock.call("_on_stop_pressed")
	await _root.get_tree().process_frame

	var err := Assert.eq(mock_plugin.last_command, "stop_recording",
		"stop button should send 'stop_recording' command")
	dock.queue_free()
	return err


func test_dock_marker_button_sends_command() -> String:
	## Regression: pressing Marker must call _debugger_plugin.send_command("add_marker").
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	var mock_plugin := MockDebuggerPlugin.new()
	dock.set("_debugger_plugin", mock_plugin)

	dock.call("_on_marker_pressed")
	await _root.get_tree().process_frame

	var err := Assert.eq(mock_plugin.last_command, "add_marker",
		"marker button should send 'add_marker' command")
	dock.queue_free()
	return err


func test_dock_buttons_safe_without_plugin() -> String:
	## Dock buttons must not crash when _debugger_plugin is null.
	## This happens in editor before the game has run.
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	# No plugin set — all buttons must be safe no-ops
	dock.call("_on_record_pressed")
	dock.call("_on_stop_pressed")
	dock.call("_on_marker_pressed")
	await _root.get_tree().process_frame

	dock.queue_free()
	return ""  # Not crashing = pass


func test_dock_activity_list_respects_max() -> String:
	## Dock must trim activity entries once MAX_ACTIVITY_ENTRIES is reached.
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	var max_entries: int = dock.get("MAX_ACTIVITY_ENTRIES") if dock.get("MAX_ACTIVITY_ENTRIES") else 20

	# Push more entries than the maximum
	for i in range(max_entries + 5):
		dock.receive_activity("query", "Entry %d" % i, "test", -1)
	await _root.get_tree().process_frame

	var activity_list: VBoxContainer = dock.get_node("%ActivityList")
	var count: int = activity_list.get_child_count()

	dock.queue_free()
	return Assert.true_(count <= max_entries,
		"activity list capped at MAX_ACTIVITY_ENTRIES (got %d, max %d)" % [count, max_entries])
