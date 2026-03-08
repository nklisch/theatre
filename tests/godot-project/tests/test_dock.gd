## Tests the dock's push-driven API.
##
## The dock receives data via public methods called by debugger_plugin.gd
## when the game pushes messages. The dock is read-only — no buttons send
## commands to the game.
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
	## Regression: the dock must not have a _tcp_server field.
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


func test_dock_has_no_recording_buttons() -> String:
	## Regression: record/stop/marker buttons were removed in dashcam-only refactor.
	var dock_scene: PackedScene = load("res://addons/spectator/dock.tscn")
	if not dock_scene:
		return "dock.tscn failed to load"
	var dock := dock_scene.instantiate()
	_root.add_child(dock)
	await _root.get_tree().process_frame

	var record_btn = dock.get("record_btn")
	var stop_btn = dock.get("stop_btn")
	dock.queue_free()

	var err := Assert.is_null(record_btn, "record_btn must not exist on dock")
	if err:
		return err
	return Assert.is_null(stop_btn, "stop_btn must not exist on dock")


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
