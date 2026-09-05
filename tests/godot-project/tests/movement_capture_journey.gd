extends SceneTree

class Player extends CharacterBody3D:
	func _physics_process(_delta: float) -> void:
		velocity = Vector3(Input.get_action_strength("attempt_right") * 4.0, -2.0, 0.0)
		move_and_slide()

var recorder: Node
var player: CharacterBody3D
var failures: Array[String] = []
var ranges: Dictionary = {}

func _initialize() -> void:
	GDExtensionManager.load_extension("res://addons/stage/stage.gdextension")
	InputMap.add_action("attempt_right")
	var scene := Node3D.new()
	scene.name = "MovementJourney"
	root.add_child(scene)
	current_scene = scene
	player = Player.new()
	player.name = "Player"
	player.position = Vector3(0, 0.51, 0)
	add_box(player, Vector3.ONE)
	scene.add_child(player)
	for specification in [[Vector3(0, -0.5, 0), Vector3(20, 1, 20)], [Vector3(2, 1, 0), Vector3(1, 3, 10)]]:
		var obstacle := StaticBody3D.new()
		obstacle.position = specification[0]
		add_box(obstacle, specification[1])
		scene.add_child(obstacle)
	var outside := CharacterBody3D.new()
	outside.name = "OutsideScene"
	root.add_child(outside)
	var collector = ClassDB.instantiate("StageCollector")
	root.add_child(collector)
	recorder = ClassDB.instantiate("StageRecorder")
	root.add_child(recorder)
	recorder.set_collector(collector)
	# This fixture constructs the recorder without StageRuntime startup glue.
	# Stop it before deferred setup can follow a default-cadence physics tick.
	check(patch({"enabled": false}).get("result") == "ok", "hold capture until configured")
	call_deferred("exercise")

func add_box(body: Node3D, size: Vector3) -> void:
	var shape := BoxShape3D.new()
	shape.size = size
	var collision := CollisionShape3D.new()
	collision.shape = shape
	body.add_child(collision)

func check(value: bool, message: String) -> void:
	if not value:
		failures.append(message)

func patch(value: Dictionary) -> Dictionary:
	return JSON.parse_string(recorder.apply_dashcam_config(JSON.stringify(value)))

func frames(count: int) -> void:
	for index in count:
		await physics_frame
	await process_frame

func exercise() -> void:
	check(patch({"enabled": true, "screenshot_enabled": false, "anomaly_enabled": false, "capture_interval": 2}).get("result") == "ok", "start with the tested cadence")
	await frames(8)
	ranges.disabled = Engine.get_physics_frames() - 2
	var original: String = recorder.get_dashcam_config_json()
	check(patch({"movement_nodes": ["Missing"], "enabled": false}).has("error"), "unknown node rejected")
	check(recorder.get_dashcam_config_json() == original, "node rejection is nonmutating")
	check(patch({"movement_nodes": ["/root/OutsideScene"]}).has("error"), "out-of-scene target rejected rather than silently omitted")
	check(recorder.get_dashcam_config_json() == original, "out-of-scene rejection is nonmutating")
	check(patch({"movement_nodes": ["Player"], "input_actions": ["unknown_action"]}).has("error"), "unknown action rejected")
	check(recorder.get_dashcam_config_json() == original, "action rejection is nonmutating")
	var applied := patch({"movement_nodes": ["./Player"], "input_actions": ["attempt_right"]})
	check(applied.get("result") == "ok", "movement configuration accepted")
	check(applied.config.movement_nodes == [str(player.get_path())], "Godot canonical target path")
	await frames(10)
	ranges.idle = Engine.get_physics_frames() - 2
	Input.action_press("attempt_right")
	await frames(6)
	ranges.attempted = Engine.get_physics_frames() - 2
	await frames(60)
	ranges.blocked = Engine.get_physics_frames() - 2
	Input.action_release("attempt_right")
	await frames(6)
	var clip_id: String = recorder.flush_dashcam_clip("Movement evidence")
	check(not clip_id.is_empty(), "clip saved")
	player.queue_free()
	await frames(3)
	check(patch({"enabled": false}).get("result") == "ok", "freed target does not prevent stopping capture")
	print("MOVEMENT_REPORT:" + JSON.stringify({"failures": failures, "clip_id": clip_id, "storage": ProjectSettings.globalize_path("user://stage_recordings"), "ranges": ranges}))
	quit(0 if failures.is_empty() else 1)
