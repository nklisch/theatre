extends SceneTree

const Feedback := preload("res://addons/theatre_shared/feedback.gd")
const Composer := preload("res://addons/theatre_shared/feedback_composer.gd")

func _initialize() -> void:
	call_deferred("run")

func run() -> void:
	root.size = Vector2i(1920, 1080)
	root.content_scale_size = Vector2i(960, 540)
	root.content_scale_mode = Window.CONTENT_SCALE_MODE_CANVAS_ITEMS
	var scene := Node2D.new()
	scene.name = "FeedbackTest"
	root.add_child(scene)
	current_scene = scene
	var color := ColorRect.new()
	color.color = Color(0.1, 0.3, 0.8)
	color.size = Vector2(960, 540)
	scene.add_child(color)
	await process_frame
	await process_frame
	if DisplayServer.get_name() != "headless":
		await RenderingServer.frame_post_draw
	if DisplayServer.get_name() != "headless":
		root.warp_mouse(Vector2(480, 270))
		await process_frame
	var capture := Feedback.capture(root, "runtime", "res://runtime.gd", [], "root_viewport", "run_fixture")
	if DisplayServer.get_name() != "headless":
		assert(capture.item.pointer.status == "inside")
		assert(absf(capture.item.pointer.position[0] - 960) < 2, str(capture.item.pointer))
		assert(absf(capture.item.pointer.position[1] - 540) < 2, str(capture.item.pointer))
	paused = true
	var second := Feedback.capture(root, "runtime", "res://runtime.gd", [], "root_viewport", "run_fixture")
	assert(paused)
	var composer := Composer.new()
	root.add_child(composer)
	composer.compose(capture)
	await process_frame
	if DisplayServer.get_name() != "headless":
		assert(composer.size.y <= DisplayServer.screen_get_usable_rect().size.y, "Queue button must fit on screen")
		assert(composer.get_ok_button().is_visible_in_tree())
	composer._note.text = "Blue surface before composer"
	composer._queue()
	assert(paused)
	var result := Feedback.publish(second, "Paused capture")
	assert(result.has("feedback_id"), str(result))
	var oversized := Feedback.publish(second, "x".repeat(Feedback.MAX_NOTE_BYTES + 1))
	assert(oversized.has("error"))
	var path := ProjectSettings.globalize_path(Feedback.ROOT)
	for index in range(Feedback.MAX_ITEMS):
		DirAccess.make_dir_absolute(path.path_join(".pending-capacity_%d" % index))
	var rejected := Feedback.publish(Feedback.capture(root, "runtime", "", [], "root_viewport"), "Keep this unsent note")
	assert(rejected.has("error") and "full" in rejected.error)
	print("FEEDBACK_RUNTIME_OK")
	quit()
