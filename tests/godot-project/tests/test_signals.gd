## Tests signal emission from SpectatorRecorder.
##
## Verifies that recording lifecycle signals fire correctly from GDScript,
## which is distinct from whether the GDExtension emits them internally.
extends RefCounted

var _root: Window


func setup(root: Window) -> void:
	_root = root


func test_recorder_emits_recording_started() -> String:
	var collector := SpectatorCollector.new()
	_root.add_child(collector)

	var recorder := SpectatorRecorder.new()
	recorder.set_collector(collector)
	_root.add_child(recorder)
	await _root.get_tree().process_frame

	var signal_data := {}
	recorder.recording_started.connect(func(id: String, name: String):
		signal_data["id"] = id
		signal_data["name"] = name
	)

	recorder.start_recording("test_signal", "/tmp/spectator-gdtest/", 1, 100)
	await _root.get_tree().process_frame

	var err := Assert.not_null(signal_data.get("id"), "recording_started fired")

	if recorder.is_recording():
		recorder.stop_recording()
	recorder.queue_free()
	collector.queue_free()
	return err


func test_recorder_emits_recording_stopped() -> String:
	var collector := SpectatorCollector.new()
	_root.add_child(collector)

	var recorder := SpectatorRecorder.new()
	recorder.set_collector(collector)
	_root.add_child(recorder)
	await _root.get_tree().process_frame

	recorder.start_recording("test_signal_stop", "/tmp/spectator-gdtest/", 1, 100)
	await _root.get_tree().process_frame

	var stopped := false
	recorder.recording_stopped.connect(func(_id, _frames): stopped = true)
	recorder.stop_recording()
	await _root.get_tree().process_frame

	recorder.queue_free()
	collector.queue_free()
	return Assert.true_(stopped, "recording_stopped fired")


func test_recorder_emits_marker_added() -> String:
	var collector := SpectatorCollector.new()
	_root.add_child(collector)

	var recorder := SpectatorRecorder.new()
	recorder.set_collector(collector)
	_root.add_child(recorder)
	await _root.get_tree().process_frame

	recorder.start_recording("test_marker", "/tmp/spectator-gdtest/", 1, 100)
	await _root.get_tree().process_frame

	var marker_data := {}
	recorder.marker_added.connect(func(frame: int, source: String, label: String):
		marker_data["frame"] = frame
		marker_data["source"] = source
		marker_data["label"] = label
	)

	recorder.add_marker("test", "my_label")
	await _root.get_tree().process_frame

	var err := Assert.not_null(marker_data.get("frame"), "marker_added fired")
	if err:
		if recorder.is_recording():
			recorder.stop_recording()
		recorder.queue_free()
		collector.queue_free()
		return err

	err = Assert.eq(marker_data.get("source"), "test", "marker source")
	if not err:
		err = Assert.eq(marker_data.get("label"), "my_label", "marker label")

	if recorder.is_recording():
		recorder.stop_recording()
	recorder.queue_free()
	collector.queue_free()
	return err


func test_tcp_server_emits_activity_received() -> String:
	var server := SpectatorTCPServer.new()
	return Assert.obj_has_signal(server, "activity_received")
