@tool
extends EditorDebuggerPlugin

## Receives pushed status/activity/recording messages from the running game
## (via EngineDebugger.send_message) and forwards them to the dock.
## Also sends record/stop/marker commands back to the game.

var _dock: Control        ## wired by plugin.gd after instantiation
var _last_session_id := 0


func _has_capture(prefix: String) -> bool:
	return prefix == "spectator"


func _capture(message: String, data: Array, session_id: int) -> bool:
	_last_session_id = session_id
	if not _dock:
		return true
	match message:
		"spectator:status":
			# [status_str, port_int, tracked_int, groups_int, frame_int, fps_int]
			if data.size() == 6:
				_dock.receive_status(data[0], data[1], data[2], data[3], data[4], data[5])
		"spectator:activity":
			# [entry_type, summary, tool_name, active_watches_int]
			if data.size() == 4:
				_dock.receive_activity(data[0], data[1], data[2], data[3])
		"spectator:recording":
			# [is_recording_bool, elapsed_ms_int, frames_int, buffer_kb_int]
			if data.size() == 4:
				_dock.receive_recording(data[0], data[1], data[2], data[3])
		_:
			return false
	return true


## Send a control command to the game process.
func send_command(command: String, args: Array = []) -> void:
	var session := get_session(_last_session_id)
	if session and session.is_active():
		session.send_message("spectator:command", [command] + args)
