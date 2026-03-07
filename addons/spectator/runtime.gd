extends Node

## Class-level reference for dock access (set in _ready, cleared in _exit_tree).
static var instance: Node = null

var tcp_server: SpectatorTCPServer
var collector: SpectatorCollector

var _overlay: CanvasLayer
var _pause_label: Label
var _toast_container: VBoxContainer
var _toasts: Array[Control] = []

const MAX_TOASTS := 3
const TOAST_DURATION := 3.0


func _ready() -> void:
	instance = self

	if not ClassDB.class_exists(&"SpectatorTCPServer"):
		push_error("[Spectator] GDExtension not loaded — SpectatorTCPServer class not found. Check that the spectator.gdextension binary exists for your platform.")
		return

	var auto_start: bool = ProjectSettings.get_setting(
		"spectator/connection/auto_start", true)
	if not auto_start:
		return

	collector = SpectatorCollector.new()
	add_child(collector)

	tcp_server = SpectatorTCPServer.new()
	add_child(tcp_server)
	tcp_server.set_collector(collector)
	tcp_server.activity_received.connect(_on_activity_received)

	var port: int = ProjectSettings.get_setting("spectator/connection/port", 9077)
	tcp_server.start(port)

	_setup_overlay()


func _setup_overlay() -> void:
	_overlay = CanvasLayer.new()
	_overlay.layer = 128
	add_child(_overlay)

	_pause_label = Label.new()
	_pause_label.text = "⏸ PAUSED"
	_pause_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_pause_label.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	_pause_label.add_theme_font_size_override("font_size", 48)
	_pause_label.modulate = Color(1.0, 1.0, 1.0, 0.7)
	_pause_label.set_anchors_preset(Control.PRESET_CENTER)
	_pause_label.visible = false
	_overlay.add_child(_pause_label)

	_toast_container = VBoxContainer.new()
	_toast_container.set_anchors_preset(Control.PRESET_TOP_RIGHT)
	_toast_container.anchor_left = 1.0
	_toast_container.anchor_right = 1.0
	_toast_container.offset_left = -350
	_toast_container.offset_top = 20
	_toast_container.offset_right = -20
	_overlay.add_child(_toast_container)


func _physics_process(_delta: float) -> void:
	if tcp_server:
		tcp_server.poll()


func _shortcut_input(event: InputEvent) -> void:
	if not event.is_pressed() or event.is_echo():
		return
	if event is InputEventKey:
		match event.keycode:
			KEY_F10:
				_toggle_pause()
				get_viewport().set_input_as_handled()


func _toggle_pause() -> void:
	var tree := get_tree()
	tree.paused = not tree.paused
	if _pause_label:
		_pause_label.visible = tree.paused


func _on_activity_received(entry_type: String, summary: String, _tool: String) -> void:
	if entry_type == "action":
		_show_toast(summary)


func _show_toast(text: String) -> void:
	if not ProjectSettings.get_setting("spectator/display/show_agent_notifications", true):
		return
	if not _toast_container:
		return

	var panel := PanelContainer.new()
	panel.modulate = Color(1.0, 1.0, 1.0, 0.9)

	var label := Label.new()
	label.text = text
	label.autowrap_mode = TextServer.AUTOWRAP_WORD
	panel.add_child(label)

	_toast_container.add_child(panel)
	_toasts.append(panel)

	# Remove oldest if over limit
	while _toasts.size() > MAX_TOASTS:
		var old: Control = _toasts.pop_front()
		if is_instance_valid(old):
			old.queue_free()

	# Auto-dismiss
	get_tree().create_timer(TOAST_DURATION).timeout.connect(func() -> void:
		if is_instance_valid(panel):
			_toasts.erase(panel)
			panel.queue_free()
	)


func _exit_tree() -> void:
	instance = null
	if tcp_server:
		tcp_server.stop()
