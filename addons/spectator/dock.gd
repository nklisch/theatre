@tool
extends VBoxContainer

## Maximum number of activity entries displayed.
const MAX_ACTIVITY_ENTRIES := 20

var _tcp_server: Object  # SpectatorTCPServer (typed loosely for editor safety)
var _update_timer := 0.0
var _collapsed := false
var _active_watches := 0

@onready var status_dot: ColorRect = %StatusDot
@onready var status_label: Label = %StatusLabel
@onready var port_label: Label = %PortLabel
@onready var tracking_label: Label = %TrackingLabel
@onready var watches_label: Label = %WatchesLabel
@onready var frame_label: Label = %FrameLabel
@onready var activity_list: VBoxContainer = %ActivityList
@onready var activity_scroll: ScrollContainer = %ActivityScroll
@onready var collapse_btn: Button = %CollapseBtn


func _ready() -> void:
	collapse_btn.pressed.connect(_toggle_collapse)
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
		return
	# Try to get the runtime via its static instance var (set when game is running)
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


func _set_status(state: String, text: String) -> void:
	status_label.text = text
	match state:
		"connected":
			status_dot.color = Color(0.2, 0.8, 0.2)
		"waiting":
			status_dot.color = Color(0.9, 0.8, 0.1)
		_:
			status_dot.color = Color(0.7, 0.2, 0.2)


func _on_activity_received(entry_type: String, summary: String, tool_name: String) -> void:
	if entry_type == "watch":
		if summary.begins_with("Watching "):
			_active_watches += 1
		elif summary.begins_with("Removed watch "):
			_active_watches = max(0, _active_watches - 1)
		elif summary == "Cleared all watches":
			_active_watches = 0
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
			color = "cyan"
		"recording":
			color = "cyan"
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
