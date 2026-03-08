## Tests signal emission from SpectatorRecorder.
##
## Verifies that dashcam signals fire correctly from GDScript.
extends RefCounted

var _root: Window


func setup(root: Window) -> void:
	_root = root


func test_recorder_emits_marker_added() -> String:
	var collector := SpectatorCollector.new()
	_root.add_child(collector)

	var recorder := SpectatorRecorder.new()
	recorder.set_collector(collector)
	_root.add_child(recorder)
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
		recorder.queue_free()
		collector.queue_free()
		return err

	err = Assert.eq(marker_data.get("source"), "test", "marker source")
	if not err:
		err = Assert.eq(marker_data.get("label"), "my_label", "marker label")

	recorder.queue_free()
	collector.queue_free()
	return err


func test_tcp_server_emits_activity_received() -> String:
	var server := SpectatorTCPServer.new()
	return Assert.obj_has_signal(server, "activity_received")
