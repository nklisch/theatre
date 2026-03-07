extends Node

var tcp_server: SpectatorTCPServer
var collector: SpectatorCollector


func _ready() -> void:
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

	var port: int = ProjectSettings.get_setting("spectator/connection/port", 9077)
	tcp_server.start(port)


func _physics_process(_delta: float) -> void:
	if tcp_server:
		tcp_server.poll()


func _exit_tree() -> void:
	if tcp_server:
		tcp_server.stop()
