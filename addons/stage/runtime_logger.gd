extends Logger

## Current-process diagnostic capture for Stage.
##
## The queue retains the newest 128 diagnostics. Individual text fields and
## backtraces are bounded before retention so a logging storm cannot grow the
## game process without limit. Logger callbacks can run on worker threads: they
## only build owned dictionaries, and the mutex protects only bounded queue data.
const MAX_ENTRIES := 128
const MAX_MESSAGE_CHARS := 2048
const MAX_FILE_CHARS := 512
const MAX_FUNCTION_CHARS := 256
const MAX_BACKTRACE_FRAMES := 16

var _mutex := Mutex.new()
var _entries: Array = []
var _omitted_count: int = 0
var _next_sequence: int = 1


func _log_error(
		function: String,
		file: String,
		line: int,
		code: String,
		rationale: String,
		_editor_notify: bool,
		error_type: int,
		script_backtraces: Array[ScriptBacktrace]
) -> void:
	var message := rationale if not rationale.is_empty() else code
	var entry := {
		"sequence": 0,
		"kind": _kind_name(error_type),
		"message": _bounded(message, MAX_MESSAGE_CHARS),
		"origin": {
			"function": _bounded(function, MAX_FUNCTION_CHARS),
			"file": _bounded(file, MAX_FILE_CHARS),
			"line": line,
		},
		"backtrace": _bounded_backtrace(script_backtraces),
	}
	_retain(entry)


## Called only from the Stage query bridge on the Godot main thread.
func snapshot() -> Dictionary:
	_mutex.lock()
	var result := {
		"entries": _entries.duplicate(true),
		"retained_count": _entries.size(),
		"omitted_count": _omitted_count,
		"limits": {
			"queue_capacity": MAX_ENTRIES,
			"message_max_chars": MAX_MESSAGE_CHARS,
			"file_max_chars": MAX_FILE_CHARS,
			"function_max_chars": MAX_FUNCTION_CHARS,
			"backtrace_max_frames": MAX_BACKTRACE_FRAMES,
		},
	}
	_mutex.unlock()
	return result


func _retain(entry: Dictionary) -> void:
	_mutex.lock()
	entry["sequence"] = _next_sequence
	_next_sequence += 1
	_entries.append(entry)
	if _entries.size() > MAX_ENTRIES:
		_entries.pop_front()
		_omitted_count += 1
	_mutex.unlock()


static func _kind_name(error_type: int) -> String:
	match error_type:
		0: return "error"
		1: return "warning"
		2: return "script_error"
		3: return "shader_error"
	return "error"


static func _bounded(value: String, max_chars: int) -> String:
	if value.length() <= max_chars:
		return value
	return value.substr(0, max_chars)


static func _bounded_backtrace(script_backtraces: Array[ScriptBacktrace]) -> Array:
	var frames: Array = []
	for script_backtrace in script_backtraces:
		for frame_index in script_backtrace.get_frame_count():
			if frames.size() >= MAX_BACKTRACE_FRAMES:
				return frames
			frames.append({
				"file": _bounded(script_backtrace.get_frame_file(frame_index), MAX_FILE_CHARS),
				"function": _bounded(script_backtrace.get_frame_function(frame_index), MAX_FUNCTION_CHARS),
				"line": script_backtrace.get_frame_line(frame_index),
			})
	return frames
