extends PanelContainer
## Native capture controls. Runtime owns recorder calls; this view only presents
## authoritative status and emits deliberate human actions.

signal toggle_requested
signal marker_requested
signal save_requested
signal preset_requested(preset: String)
signal feedback_requested

var _toggle: Button
var _marker: Button
var _save: Button
var _status: Label
var _last_saved: Label
var _copy: Button
var _preset: OptionButton
var _last_clip: Dictionary = {}
var _placement := "bottom_right"
var _acknowledgement := ""
var _acknowledgement_until := 0


func _init() -> void:
	name = "StageCaptureControls"
	custom_minimum_size.x = 284
	size.x = 284
	add_theme_font_size_override("font_size", 13)
	var margin := MarginContainer.new()
	for edge in ["margin_left", "margin_right", "margin_top", "margin_bottom"]:
		margin.add_theme_constant_override(edge, 8)
	add_child(margin)
	var rows := VBoxContainer.new()
	margin.add_child(rows)
	var heading := HBoxContainer.new()
	rows.add_child(heading)
	var title := Label.new()
	title.text = "Stage capture"
	title.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	heading.add_child(title)
	_preset = OptionButton.new()
	_preset.name = "CapturePreset"
	for label in ["Custom", "Lightweight", "Detailed"]:
		_preset.add_item(label)
	_preset.tooltip_text = "Sampling and image size only. Choosing a preset does not start recording."
	_preset.item_selected.connect(func(index: int) -> void:
		if index > 0:
			preset_requested.emit("lightweight" if index == 1 else "detailed")
	)
	heading.add_child(_preset)
	var actions := HBoxContainer.new()
	rows.add_child(actions)
	_toggle = _button("Start", "ToggleDashcam", actions, func() -> void: toggle_requested.emit())
	_marker = _button("Mark", "Mark", actions, func() -> void: marker_requested.emit())
	_marker.tooltip_text = "Mark this moment and retain the configured before/after window."
	_save = _button("Save now", "SaveNow", actions, func() -> void: save_requested.emit())
	_save.tooltip_text = "Save the available buffer immediately, without waiting for the remaining post-window."
	_status = Label.new()
	_status.name = "CaptureStatus"
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_status.max_lines_visible = 3
	_status.text = "Recorder unavailable"
	rows.add_child(_status)
	_last_saved = Label.new()
	_last_saved.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_last_saved.text = "No clip saved this run"
	rows.add_child(_last_saved)
	var footer := HBoxContainer.new()
	rows.add_child(footer)
	_copy = _button("Copy reference", "CopyClipReference", footer, _copy_reference)
	_copy.disabled = true
	var feedback := _button("Share note + still", "ShareFeedback", footer,
		func() -> void: feedback_requested.emit())
	feedback.tooltip_text = "Ctrl+Shift+F8 · Separate from dashcam clips. Compose a note with a captured still image."


func _button(text: String, node_name: String, parent: Node, callback: Callable) -> Button:
	var button := Button.new()
	button.name = node_name
	button.text = text
	button.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	button.pressed.connect(callback)
	parent.add_child(button)
	return button


func configure(marker_binding: String, placement: String) -> void:
	_marker.text = "Mark (%s)" % marker_binding
	visible = placement != "hidden"
	_placement = placement
	if placement not in ["top_left", "top_right", "bottom_left", "bottom_right", "hidden"]:
		push_warning("[Stage] Unknown capture_controls placement '%s'; using bottom_right" % placement)
		_placement = "bottom_right"
	_place()


func _ready() -> void:
	# Container minimum sizes settle after configuration. Reposition from the
	# actual size rather than anchoring the earlier, smaller minimum rectangle.
	resized.connect(_place)
	minimum_size_changed.connect(func() -> void: call_deferred("_resize_to_content"))
	call_deferred("_resize_to_content")
	get_viewport().size_changed.connect(_place)
	_place()


func _resize_to_content() -> void:
	reset_size()
	_place()


func _place() -> void:
	if not is_inside_tree():
		return
	var available := get_viewport().get_visible_rect().size
	var target := Vector2(12, 12)
	if _placement.ends_with("right"):
		target.x = maxf(0, available.x - size.x - 12)
	if _placement.begins_with("bottom"):
		target.y = maxf(0, available.y - size.y - 12)
	position = target


func acknowledge(message: String) -> void:
	# Human confirmation must not depend on the agent-notification preference.
	_acknowledgement = message
	_acknowledgement_until = Time.get_ticks_msec() + 4000
	_status.text = message
	_status.tooltip_text = message


func refresh(status: Dictionary) -> void:
	var available := not status.is_empty()
	var active := bool(status.get("dashcam_enabled", false))
	_toggle.disabled = not available
	_toggle.text = "Stop" if active else "Start"
	_marker.disabled = not active
	_save.disabled = not active or int(status.get("buffer_frames", 0)) == 0
	_preset.disabled = not available
	if not active:
		_marker.tooltip_text = "Start dashcam before marking. The marker shortcut will explain this too."
	else:
		_marker.tooltip_text = "Retain the configured before/after window around this moment."
	match status.get("preset"):
		"lightweight": _preset.select(1)
		"detailed": _preset.select(2)
		_: _preset.select(0)
	var coverage: Dictionary = status.get("coverage", {})
	var buffered := float(coverage.get("buffered_seconds") if coverage.get("buffered_seconds") != null else 0.0)
	var message := "Dashcam stopped · Start to retain gameplay"
	if not available:
		message = "Recorder unavailable · Check Stage auto-start and addon loading"
	elif status.get("last_save_error") != null:
		message = "Save failed · %s" % status.last_save_error
	elif status.get("state") == "post_capture":
		message = "Marked · collecting %.1f s after the marker" % float(coverage.get("post_window_remaining_seconds", 0.0))
	elif active:
		message = "Buffering · %.1f s retained · images %s" % [buffered,
			"available" if status.get("screenshots_available", false) else "unavailable"]
	if Time.get_ticks_msec() < _acknowledgement_until:
		message = _acknowledgement
	_status.text = message
	_status.tooltip_text = message
	var clip: Variant = status.get("last_saved_clip")
	if clip is Dictionary:
		_last_clip = clip
		_last_saved.text = "Last saved: %s" % str(clip.get("clip_id", ""))
		_copy.disabled = false


func _copy_reference() -> void:
	if _last_clip.is_empty():
		return
	var runtime: Dictionary = _last_clip.get("runtime", {})
	DisplayServer.clipboard_set("Saved Stage clip %s; run %s; frames %s; scene at save %s (clips can span scenes). Inspect with clips(list) and clips(markers); retained inspection also works after the game stops." % [
		_last_clip.get("clip_id", ""), runtime.get("run_id", "unknown"),
		str(_last_clip.get("frame_range", [])), str(_last_clip.get("scene_at_save", "unknown"))])
	acknowledge("Clip reference copied")
