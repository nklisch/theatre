extends Window
## Native capture controls. Runtime owns recorder calls; this view only presents
## authoritative status and emits deliberate human actions.

signal toggle_requested
signal marker_requested
signal save_requested
signal preset_requested(preset: String)
signal feedback_requested
signal continue_requested
signal discard_requested
signal cancel_requested
signal shortcut_requested(event: InputEvent)

const PRESETS := {
	"Minimal": "minimal", "Light": "light", "Standard": "standard", "Heavy": "heavy",
}

var _toggle: Button
var _marker: Button
var _save: Button
var _continue: Button
var _discard: Button
var _status: Label
var _last_saved: Label
var _copy: Button
var _preset: OptionButton
var _panel: PanelContainer
var _body: VBoxContainer
var _heading: Control
var _minimize: Button
var _minimized := false
var _moved := false
var _dragging := false
var _drag_offset := Vector2.ZERO
var _last_clip: Dictionary = {}
var _placement := "bottom_right"
var _marker_binding := "F9"
var _has_draft := false
var _acknowledgement := ""
var _acknowledgement_until := 0


func _init() -> void:
	name = "StageCaptureControls"
	title = "Stage capture"
	borderless = true
	unresizable = true
	wrap_controls = true
	transient = true
	visible = false
	# A non-exclusive child Window receives its pointer events before the game's
	# _input handlers. Plain Controls receive them too late to prevent recapture.
	_panel = PanelContainer.new()
	_panel.custom_minimum_size.x = 284
	add_child(_panel)
	_panel.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_theme_font_size_override("font_size", 13)
	var margin := MarginContainer.new()
	for edge in ["margin_left", "margin_right", "margin_top", "margin_bottom"]:
		margin.add_theme_constant_override(edge, 8)
	_panel.add_child(margin)
	var rows := VBoxContainer.new()
	margin.add_child(rows)
	var header := HBoxContainer.new()
	rows.add_child(header)
	_heading = HBoxContainer.new()
	_heading.name = "CaptureDragHandle"
	_heading.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_heading.mouse_filter = Control.MOUSE_FILTER_STOP
	_heading.mouse_default_cursor_shape = Control.CURSOR_MOVE
	_heading.tooltip_text = "Drag to move capture controls"
	_heading.gui_input.connect(_drag_input)
	header.add_child(_heading)
	var heading_label := Label.new()
	heading_label.text = "Stage capture"
	heading_label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_heading.add_child(heading_label)
	_body = VBoxContainer.new()
	rows.add_child(_body)
	_preset = OptionButton.new()
	_preset.name = "CapturePreset"
	for label in PRESETS:
		_preset.add_item(label)
		_preset.set_item_metadata(_preset.item_count - 1, PRESETS[label])
	_preset.tooltip_text = "Settings for the next segment. Light and Standard require explicit node paths or groups in the launch options."
	_preset.item_selected.connect(func(index: int) -> void:
		var preset: String = _preset.get_item_metadata(index)
		if not preset.is_empty():
			preset_requested.emit(preset)
	)
	header.add_child(_preset)
	_minimize = _button("−", "MinimizeCaptureControls", header, _toggle_minimized)
	_minimize.custom_minimum_size.x = 28
	_minimize.size_flags_horizontal = Control.SIZE_SHRINK_END
	_minimize.tooltip_text = "Minimize capture controls (recording continues)"
	var actions := HBoxContainer.new()
	_body.add_child(actions)
	_toggle = _button("Start new", "ToggleDashcam", actions, func() -> void: toggle_requested.emit())
	_marker = _button("Mark", "Mark", actions, func() -> void: marker_requested.emit())
	_marker.tooltip_text = "Annotate this moment. Automatic saving requires rolling retention and on_trigger saving."
	var review := HBoxContainer.new()
	_body.add_child(review)
	_continue = _button("Continue", "ContinueCapture", review, func() -> void: continue_requested.emit())
	_continue.tooltip_text = "New segment in the same recording. The uncaptured gap remains visible."
	_save = _button("Keep", "SaveNow", review, func() -> void: save_requested.emit())
	_save.tooltip_text = "Save the stopped draft. Failed saves retain the draft for retry."
	_discard = _button("Discard", "DiscardCapture", review, func() -> void:
		if _has_draft:
			discard_requested.emit()
		else:
			cancel_requested.emit()
	)
	_status = Label.new()
	_status.name = "CaptureStatus"
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_status.max_lines_visible = 2
	_status.text = "Recorder unavailable"
	_body.add_child(_status)
	_last_saved = Label.new()
	_last_saved.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_last_saved.max_lines_visible = 1
	_last_saved.text_overrun_behavior = TextServer.OVERRUN_TRIM_ELLIPSIS
	_last_saved.text = "No clip saved this run"
	_body.add_child(_last_saved)
	var footer := HBoxContainer.new()
	_body.add_child(footer)
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
	_marker_binding = marker_binding
	_marker.text = "Mark (%s)" % marker_binding
	if placement == "hidden":
		hide()
	elif not visible:
		# Showing the overlay must not take the game's keyboard focus.
		unfocusable = true
		show()
		unfocusable = false
	_placement = placement
	_moved = false
	if placement not in ["top_left", "top_right", "bottom_left", "bottom_right", "hidden"]:
		push_warning("[Stage] Unknown capture_controls placement '%s'; using bottom_right" % placement)
		_placement = "bottom_right"
	_place()


func _ready() -> void:
	# Container minimum sizes settle after configuration. Reposition from the
	# actual size rather than anchoring the earlier, smaller minimum rectangle.
	size_changed.connect(_place)
	_panel.minimum_size_changed.connect(func() -> void: call_deferred("_resize_to_content"))
	call_deferred("_resize_to_content")
	get_parent().get_viewport().size_changed.connect(_place)
	close_requested.connect(_toggle_minimized)
	focus_exited.connect(func() -> void: _dragging = false)
	# Events in a child Window do not reach the runtime's parent viewport.
	window_input.connect(func(event: InputEvent) -> void: shortcut_requested.emit(event))
	_place()


func _resize_to_content() -> void:
	reset_size()
	_place()


func _process(_delta: float) -> void:
	# A captured gameplay pointer must not hit an overlaid panel at screen center.
	# Observe the game's choice; never change its pointer mode.
	var captured := Input.mouse_mode == Input.MOUSE_MODE_CAPTURED
	if unfocusable != captured:
		unfocusable = captured
		mouse_passthrough = captured


func _place() -> void:
	if not is_inside_tree():
		return
	var parent_viewport := get_parent().get_viewport()
	var available := parent_viewport.get_visible_rect().size
	var origin := Vector2.ZERO if is_embedded() else parent_viewport.get_screen_transform().origin
	if not is_embedded():
		available = Vector2(get_parent().get_window().size)
	var target := Vector2(12, 12)
	if _moved:
		target = Vector2(position) - origin
	elif _placement.ends_with("right"):
		target.x = maxf(0, available.x - size.x - 12)
	if not _moved and _placement.begins_with("bottom"):
		target.y = maxf(0, available.y - size.y - 12)
	target.x = clampf(target.x, 0, maxf(0, available.x - size.x))
	target.y = clampf(target.y, 0, maxf(0, available.y - size.y))
	position = Vector2i(target + origin)


func _drag_input(event: InputEvent) -> void:
	if event is InputEventMouseButton and event.button_index == MOUSE_BUTTON_LEFT:
		_dragging = event.pressed
		_drag_offset = event.position
		_heading.accept_event()
	elif event is InputEventMouseMotion and _dragging:
		_moved = true
		position += Vector2i(event.position - _drag_offset)
		_place()
		_heading.accept_event()


func _toggle_minimized() -> void:
	_minimized = not _minimized
	_body.visible = not _minimized
	_heading.visible = not _minimized
	_preset.visible = not _minimized
	_panel.custom_minimum_size.x = 0 if _minimized else 284
	_minimize.text = "Stage +" if _minimized else "−"
	_minimize.tooltip_text = "Restore capture controls" if _minimized else "Minimize capture controls (recording continues)"
	_resize_to_content()


func acknowledge(message: String) -> void:
	# Human confirmation must not depend on the agent-notification preference.
	_acknowledgement = message
	_acknowledgement_until = Time.get_ticks_msec() + 4000
	_status.text = message
	_status.tooltip_text = message


func clear_acknowledgement() -> void:
	_acknowledgement = ""
	_acknowledgement_until = 0


func refresh(status: Dictionary) -> void:
	var state: String = status.get("state", "off")
	var available := state != "off"
	var active := state == "recording"
	var draft := state == "review"
	_has_draft = draft
	var next_enabled: bool = status.get("next_config", {}).get("enabled", false)
	_toggle.disabled = not available or draft or (not active and (not status.get("ready", false) or not next_enabled))
	_toggle.text = "Stop" if active else "Start new"
	_marker.disabled = not active
	_save.disabled = not draft
	_discard.text = "Discard" if draft else "Cancel"
	_discard.disabled = not available or active or (not draft and not next_enabled)
	_discard.tooltip_text = "Remove only this unsaved draft" if draft else "Cancel scheduled play capture for this run; saved evidence is retained"
	_continue.disabled = _toggle.disabled or active or not status.get("can_continue", false)
	_preset.disabled = not available
	var auto_save: bool = status.get("config", {}).get("save") == "on_trigger"
	_marker.text = ("Mark + save (%s)" if active and auto_save else "Mark (%s)") % _marker_binding
	if not active:
		_marker.tooltip_text = "Start capture before marking."
	else:
		_marker.tooltip_text = "Mark, save after the five-second post-window, then resume rolling capture." if auto_save else "Annotate this moment without saving."
	_preset.select(0)
	for index in _preset.item_count:
		if _preset.get_item_metadata(index) == status.get("next_config", {}).get("preset"):
			_preset.select(index)
			break
	var message := "Ready · No capture running" if status.get("ready", false) else "Waiting for project readiness"
	if not available:
		message = "Capture off for this launch"
	elif status.get("last_save_error") != null:
		message = "Save failed · Draft retained; retry Keep or Discard"
	elif draft:
		message = "Stopped (%s) · Unsaved draft; Keep or Discard before exit" % str(status.get("stop_reason", "manual_stop"))
	elif active:
		message = "%s · %.1f s · %d metrics / %d spatial / %d images" % [str(status.get("phase", "Capture")).capitalize(), float(status.get("active_duration_usec", 0)) / 1000000.0, int(status.get("metric_samples", 0)), int(status.get("spatial_samples", 0)), int(status.get("image_samples", 0))]
	if available:
		message += " · " + _image_status(status)
	var tooltip := message
	if status.get("last_save_error") != null:
		tooltip += "\n" + str(status.last_save_error)
	if available:
		var bounds: Dictionary = status.get("config", status.get("next_config", {}))
		tooltip += "\nLimits: %s seconds / %s records / %s MiB encoded payload" % [bounds.get("duration_secs", "?"), bounds.get("max_records", "?"), bounds.get("payload_mib", "?")]
	var screenshot_capture: Dictionary = status.get("screenshot_capture", {})
	if screenshot_capture.get("reason") != null:
		tooltip += "\n" + str(screenshot_capture.reason)
	if Time.get_ticks_msec() < _acknowledgement_until:
		message = _acknowledgement
		tooltip = message
	_status.text = message
	_status.tooltip_text = tooltip
	if _minimized:
		_minimize.tooltip_text = "Restore capture controls\n" + tooltip
	var clip: Variant = status.get("last_saved_clip")
	if clip is Dictionary:
		_last_clip = clip
		_last_saved.text = "Saved · …%s" % str(clip.get("clip_id", "")).right(16)
		_last_saved.tooltip_text = str(clip.get("clip_id", ""))
		_copy.disabled = false


func _image_status(status: Dictionary) -> String:
	var config: Dictionary = status.get("config", {})
	var capture: Dictionary = status.get("screenshot_capture", {})
	var message := "Images unavailable"
	if not config.get("images", false):
		message = "Images off"
	elif status.get("state") != "recording":
		message = "Images stopped"
	elif capture.get("available", false):
		message = "Images pending" if capture.get("pending", false) else "Images on"
	elif capture.get("backend") == "initializing":
		message = "Images initializing"
	var retained := int(status.get("image_samples", 0))
	if retained > 0:
		message += " · %d images retained" % retained
	return message


func _copy_reference() -> void:
	if _last_clip.is_empty():
		return
	var runtime: Dictionary = _last_clip.get("runtime", {})
	DisplayServer.clipboard_set("Saved Stage clip %s; run %s; frames %s; scene at save %s (clips can span scenes). Inspect with clips(list) and clips(markers); retained inspection also works after the game stops." % [
		_last_clip.get("clip_id", ""), runtime.get("run_id", "unknown"),
		str(_last_clip.get("frame_range", [])), str(_last_clip.get("scene_at_save", "unknown"))])
	acknowledge("Clip reference copied")
