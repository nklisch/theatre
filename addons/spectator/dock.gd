@tool
extends VBoxContainer

## Maximum number of activity entries displayed.
const MAX_ACTIVITY_ENTRIES := 20

## Wired by plugin.gd — used to send record/stop/marker commands to the game.
var _debugger_plugin: Object = null

## State pushed from the game via EditorDebuggerPlugin.
var _collapsed := false
var _recording_active := false
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
@onready var record_btn: Button = %RecordBtn
@onready var stop_btn: Button = %StopBtn
@onready var marker_btn: Button = %MarkerBtn
@onready var recording_stats: Label = %RecordingStats


func _ready() -> void:
	collapse_btn.pressed.connect(_toggle_collapse)
	record_btn.pressed.connect(_on_record_pressed)
	stop_btn.pressed.connect(_on_stop_pressed)
	marker_btn.pressed.connect(_on_marker_pressed)
	_update_recording_controls()


# ---------------------------------------------------------------------------
# Push-driven updates from the game (called by debugger_plugin.gd)
# ---------------------------------------------------------------------------

func receive_status(status: String, port: int, tracked: int, groups: int, frame: int, fps: int) -> void:
	match status:
		"connected":
			_set_status("connected", "Connected")
			tracking_label.text = "Tracking: %d nodes (%d groups)" % [tracked, groups]
			frame_label.text = "Frame: %d | %d fps" % [frame, fps]
		"waiting":
			_set_status("waiting", "Waiting...")
			tracking_label.text = "Tracking: —"
			frame_label.text = "Frame: —"
		_:
			_set_status("stopped", "Stopped")
			tracking_label.text = "Tracking: —"
			frame_label.text = "Frame: —"
	port_label.text = "Port %d" % port
	watches_label.text = "Watches: %d active" % _active_watches


func receive_activity(entry_type: String, summary: String, tool_name: String, active_watches: int) -> void:
	if active_watches >= 0:
		_active_watches = active_watches
	_add_activity_entry({
		"entry_type": entry_type,
		"summary": summary,
		"tool": tool_name,
		"timestamp": Time.get_unix_time_from_system(),
	})


func receive_recording(is_recording: bool, elapsed_ms: int, frames: int, buffer_kb: int) -> void:
	_recording_active = is_recording
	_update_recording_controls()
	if is_recording:
		var elapsed_sec := elapsed_ms / 1000.0
		recording_stats.text = "  %s  |  Frame %d  |  %d KB" % [
			_format_elapsed(elapsed_sec), frames, buffer_kb,
		]
	recording_stats.visible = is_recording


# ---------------------------------------------------------------------------
# Button handlers — send commands to the game via debugger_plugin
# ---------------------------------------------------------------------------

func _on_record_pressed() -> void:
	if _debugger_plugin:
		_debugger_plugin.send_command("start_recording")


func _on_stop_pressed() -> void:
	if _debugger_plugin:
		_debugger_plugin.send_command("stop_recording")


func _on_marker_pressed() -> void:
	if _debugger_plugin:
		_debugger_plugin.send_command("add_marker")


func _update_recording_controls() -> void:
	record_btn.disabled = _recording_active
	stop_btn.disabled = not _recording_active
	marker_btn.disabled = not _recording_active
	recording_stats.visible = _recording_active


func _set_status(state: String, text: String) -> void:
	status_label.text = text
	match state:
		"connected":
			status_dot.color = Color(0.2, 0.8, 0.2)
		"waiting":
			status_dot.color = Color(0.9, 0.8, 0.1)
		_:
			status_dot.color = Color(0.7, 0.2, 0.2)


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
		"watch", "recording":
			color = "cyan"
		_:
			color = "gray"

	label.text = "[color=gray]%s[/color]  [color=%s]%s[/color]" % [
		time_str, color, summary,
	]

	activity_list.add_child(label)

	while activity_list.get_child_count() > MAX_ACTIVITY_ENTRIES:
		var old := activity_list.get_child(0)
		activity_list.remove_child(old)
		old.queue_free()

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
