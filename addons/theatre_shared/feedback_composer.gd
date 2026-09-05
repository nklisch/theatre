@tool
extends ConfirmationDialog

const Feedback := preload("res://addons/theatre_shared/feedback.gd")
var _composition: Dictionary
var _note: TextEdit
var _status: RichTextLabel

func _init() -> void:
	title = "Share feedback with agent"
	ok_button_text = "Queue feedback"
	dialog_hide_on_ok = false
	process_mode = Node.PROCESS_MODE_ALWAYS
	# A small game viewport must not clip the explicit Queue action. A native
	# dialog can extend beyond that window without resizing or pausing gameplay.
	force_native = DisplayServer.get_name() != "headless"
	min_size = Vector2i(480, 480)
	confirmed.connect(_queue)
	canceled.connect(queue_free)
	close_requested.connect(queue_free)

func compose(composition: Dictionary) -> void:
	_composition = composition
	var layout := VBoxContainer.new()
	add_child(layout)
	var preview := TextureRect.new()
	preview.custom_minimum_size = Vector2(440, 180)
	preview.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	preview.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
	if composition.image != null:
		preview.texture = ImageTexture.create_from_image(composition.image)
	layout.add_child(preview)
	# Scroll long paths instead of growing a native dialog beyond the game window.
	# Fixed-height text also avoids zero-width first-layout wrap inflation.
	var context := RichTextLabel.new()
	context.custom_minimum_size = Vector2(440, 80)
	context.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	var item: Dictionary = composition.item
	context.text = "%s · %s\n%d selected node(s) · pointer %s" % [item.source, item.scene, item.selection.size(), item.pointer.status]
	if item.pointer.status == "inside":
		context.text += " at (%.1f, %.1f) source pixels" % item.pointer.position
	for selected in item.selection.slice(0, 5):
		context.text += "\n" + str(selected.path)
	if item.selection.size() > 5:
		context.text += "\n… %d more selected nodes retained" % (item.selection.size() - 5)
	context.text += "\nLatest completed render; no pause or save."
	if item.capture.status == "unavailable":
		context.text += "\nImage unavailable: " + str(item.capture.reason)
	layout.add_child(context)
	_note = TextEdit.new()
	_note.placeholder_text = "Optional note: what should the agent notice or change?"
	_note.custom_minimum_size = Vector2(440, 100)
	layout.add_child(_note)
	_status = RichTextLabel.new()
	_status.custom_minimum_size = Vector2(440, 48)
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	layout.add_child(_status)
	popup_centered()
	_note.grab_focus()

func _queue() -> void:
	get_ok_button().disabled = true
	var result := Feedback.publish(_composition, _note.text)
	if result.has("error"):
		_status.text = result.error
		get_ok_button().disabled = false
		return
	queue_free()
