@tool
extends EditorDebuggerPlugin

## Receives pushed status/activity messages from the running game
## (via EngineDebugger.send_message) and forwards them to the dock.

var _dock: Control        ## wired by plugin.gd after instantiation


func _has_capture(prefix: String) -> bool:
	return prefix == "spectator"


func _capture(message: String, data: Array, _session_id: int) -> bool:
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
		_:
			return false
	return true
