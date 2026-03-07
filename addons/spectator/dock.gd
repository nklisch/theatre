@tool
extends VBoxContainer

## Maximum number of activity entries displayed.
const MAX_ACTIVITY_ENTRIES := 20

var _tcp_server: Object  # SpectatorTCPServer (typed loosely for editor safety)
var _update_timer := 0.0
var _collapsed := false
var _active_watches := 0

var _recorder: Object  # SpectatorRecorder
var _recording_active := false
var _library_dirty := true  # refresh on next update

@onready var status_dot: ColorRect = %StatusDot
@onready var status_label: Label = %StatusLabel
@onready var port_label: Label = %PortLabel
@onready var tracking_label: Label = %TrackingLabel
@onready var watches_label: Label = %WatchesLabel
@onready var frame_label: Label = %FrameLabel
@onready var activity_list: VBoxContainer = %ActivityList
@onready var activity_scroll: ScrollContainer = %ActivityScroll
@onready var collapse_btn: Button = %CollapseBtn
@onready var record_btn: Button = %RecordBtn
@onready var stop_btn: Button = %StopBtn
@onready var marker_btn: Button = %MarkerBtn
@onready var recording_stats: Label = %RecordingStats
@onready var recording_library: VBoxContainer = %RecordingLibrary


func _ready() -> void:
	collapse_btn.pressed.connect(_toggle_collapse)
	record_btn.pressed.connect(_on_record_pressed)
	stop_btn.pressed.connect(_on_stop_pressed)
	marker_btn.pressed.connect(_on_marker_pressed)
	_update_status()


func _process(delta: float) -> void:
	_update_timer += delta
	if _update_timer < 1.0:
		return
	_update_timer = 0.0
	_try_acquire_runtime()
	_update_status()


func _try_acquire_runtime() -> void:
	if _tcp_server and is_instance_valid(_tcp_server):
		pass
	else:
		var rt_script := load("res://addons/spectator/runtime.gd")
		if rt_script == null:
			return
		var rt = rt_script.get("instance")
		if rt == null or not is_instance_valid(rt):
			return
		var server = rt.get("tcp_server")
		if server == null or not is_instance_valid(server):
			return
		_tcp_server = server
		_tcp_server.activity_received.connect(_on_activity_received)

	if not _recorder or not is_instance_valid(_recorder):
		var rt_script := load("res://addons/spectator/runtime.gd")
		if rt_script:
			var rt = rt_script.get("instance")
			if rt and is_instance_valid(rt):
				var rec = rt.get("recorder")
				if rec and is_instance_valid(rec):
					_recorder = rec
					_recorder.recording_stopped.connect(_on_recording_stopped)


func _update_status() -> void:
	if _tcp_server == null or not is_instance_valid(_tcp_server):
		_tcp_server = null
		_set_status("stopped", "Stopped")
		port_label.text = "Port 9077"
		tracking_label.text = "Tracking: —"
		frame_label.text = "Frame: —"
		return

	var status: String = _tcp_server.get_connection_status()
	match status:
		"connected":
			_set_status("connected", "Connected")
		"waiting":
			_set_status("waiting", "Waiting...")
		_:
			_set_status("stopped", "Stopped")

	port_label.text = "Port %d" % _tcp_server.get_port()

	# Session info from runtime's collector
	var rt_script := load("res://addons/spectator/runtime.gd")
	var collector: Object = null
	if rt_script:
		var rt = rt_script.get("instance")
		if rt and is_instance_valid(rt):
			collector = rt.get("collector")

	if collector and is_instance_valid(collector) and status == "connected":
		tracking_label.text = "Tracking: %d nodes (%d groups)" % [
			collector.get_tracked_count(),
			collector.get_group_count(),
		]
		frame_label.text = "Frame: %d | %d fps" % [
			Engine.get_physics_frames(),
			Engine.get_frames_per_second(),
		]
	else:
		tracking_label.text = "Tracking: —"
		frame_label.text = "Frame: —"

	watches_label.text = "Watches: %d active" % _active_watches

	# Recording stats (during recording)
	if _recording_active and _recorder and is_instance_valid(_recorder):
		var elapsed_ms: int = _recorder.get_elapsed_ms()
		var elapsed_sec := elapsed_ms / 1000.0
		var frames: int = _recorder.get_frames_captured()
		var buffer_kb: int = _recorder.get_buffer_size_kb()
		recording_stats.text = "  %s  |  Frame %d  |  %d KB" % [
			_format_elapsed(elapsed_sec), frames, buffer_kb,
		]

	# Refresh recording library
	if _library_dirty and _recorder and is_instance_valid(_recorder):
		_refresh_library()
		_library_dirty = false


func _set_status(state: String, text: String) -> void:
	status_label.text = text
	match state:
		"connected":
			status_dot.color = Color(0.2, 0.8, 0.2)
		"waiting":
			status_dot.color = Color(0.9, 0.8, 0.1)
		_:
			status_dot.color = Color(0.7, 0.2, 0.2)


func _on_record_pressed() -> void:
	if not _recorder or not is_instance_valid(_recorder):
		return
	var storage_path: String = ProjectSettings.get_setting(
		"spectator/recording/storage_path", "user://spectator_recordings/")
	var interval: int = ProjectSettings.get_setting(
		"spectator/recording/capture_interval", 1)
	var max_frames: int = ProjectSettings.get_setting(
		"spectator/recording/max_frames", 36000)
	var id: String = _recorder.start_recording("", storage_path, interval, max_frames)
	if not id.is_empty():
		_recording_active = true
		_update_recording_controls()


func _on_stop_pressed() -> void:
	if not _recorder or not is_instance_valid(_recorder):
		return
	_recorder.stop_recording()
	_recording_active = false
	_library_dirty = true
	_update_recording_controls()


func _on_marker_pressed() -> void:
	if not _recorder or not is_instance_valid(_recorder):
		return
	_recorder.add_marker("human", "")


func _update_recording_controls() -> void:
	record_btn.disabled = _recording_active
	stop_btn.disabled = not _recording_active
	marker_btn.disabled = not _recording_active
	recording_stats.visible = _recording_active


func _on_recording_stopped(_id: String, _frames: int) -> void:
	_recording_active = false
	_library_dirty = true
	_update_recording_controls()


func _refresh_library() -> void:
	for child in recording_library.get_children():
		child.queue_free()

	if not _recorder or not is_instance_valid(_recorder):
		return

	var storage_path: String = ProjectSettings.get_setting(
		"spectator/recording/storage_path", "user://spectator_recordings/")
	var recordings: Array = _recorder.list_recordings(storage_path)

	recordings.sort_custom(func(a: Dictionary, b: Dictionary) -> bool:
		return a.get("created_at_ms", 0) > b.get("created_at_ms", 0)
	)

	for rec: Dictionary in recordings:
		var entry := HBoxContainer.new()

		var name_label := Label.new()
		name_label.text = rec.get("name", "?")
		name_label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		entry.add_child(name_label)

		var dur_sec: float = rec.get("duration_ms", 0) / 1000.0
		var dur_label := Label.new()
		dur_label.text = "%0.1fs" % dur_sec
		entry.add_child(dur_label)

		var del_btn := Button.new()
		del_btn.text = "x"
		var rec_id: String = rec.get("id", "")
		del_btn.pressed.connect(func() -> void:
			_delete_recording(rec_id)
		)
		entry.add_child(del_btn)

		recording_library.add_child(entry)


func _delete_recording(recording_id: String) -> void:
	if not _recorder or not is_instance_valid(_recorder):
		return
	var storage_path: String = ProjectSettings.get_setting(
		"spectator/recording/storage_path", "user://spectator_recordings/")
	_recorder.delete_recording(storage_path, recording_id)
	_library_dirty = true


func _on_activity_received(entry_type: String, summary: String, tool_name: String, active_watches: int) -> void:
	if active_watches >= 0:
		_active_watches = active_watches
	_add_activity_entry({
		"entry_type": entry_type,
		"summary": summary,
		"tool": tool_name,
		"timestamp": Time.get_unix_time_from_system(),
	})


func _add_activity_entry(entry: Dictionary) -> void:
	var label := RichTextLabel.new()
	label.bbcode_enabled = true
	label.fit_content = true
	label.scroll_active = false

	var time_str := _format_timestamp(entry.get("timestamp", 0.0))
	var summary: String = entry.get("summary", "")
	var entry_type: String = entry.get("entry_type", "query")

	var color: String
	match entry_type:
		"action":
			color = "yellow"
		"watch":
			color = "blue"
		"recording":
			color = "blue"
		_:
			color = "gray"

	label.text = "[color=gray]%s[/color]  [color=%s]%s[/color]" % [
		time_str, color, summary,
	]

	activity_list.add_child(label)

	# Trim excess entries
	while activity_list.get_child_count() > MAX_ACTIVITY_ENTRIES:
		var old := activity_list.get_child(0)
		activity_list.remove_child(old)
		old.queue_free()

	# Auto-scroll to bottom
	await get_tree().process_frame
	activity_scroll.scroll_vertical = activity_scroll.get_v_scroll_bar().max_value


func _toggle_collapse() -> void:
	_collapsed = not _collapsed
	activity_scroll.visible = not _collapsed
	collapse_btn.text = "▲" if _collapsed else "▼"


static func _format_timestamp(unix: float) -> String:
	var dt := Time.get_datetime_dict_from_unix_time(int(unix))
	return "%02d:%02d:%02d" % [dt.hour, dt.minute, dt.second]


static func _format_elapsed(seconds: float) -> String:
	var mins := int(seconds) / 60
	var secs := fmod(seconds, 60.0)
	return "%02d:%04.1f" % [mins, secs]
