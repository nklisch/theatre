## Tests that GDExtension classes are registered and instantiate correctly.
##
## This is the most basic smoke test: if these fail, nothing else will work.
extends RefCounted


func test_classes_registered() -> String:
	for cls in ["StageTCPServer", "StageCollector", "StageRecorder"]:
		if not ClassDB.class_exists(cls):
			return "class %s not registered — GDExtension may not be loaded" % cls
	return ""


func test_collector_instantiates() -> String:
	var c := StageCollector.new()
	return Assert.not_null(c, "StageCollector.new()")


func test_tcp_server_instantiates() -> String:
	var s := StageTCPServer.new()
	return Assert.not_null(s, "StageTCPServer.new()")


func test_recorder_instantiates() -> String:
	var r := StageRecorder.new()
	return Assert.not_null(r, "StageRecorder.new()")


func test_tcp_server_starts_and_stops() -> String:
	## Note: get_port() returns the configured port (0 for ephemeral), not the
	## OS-assigned port — so we only verify status transitions, not the port number.
	var s := StageTCPServer.new()
	s.start(0)  # port 0 → OS assigns an ephemeral port; status goes to "waiting"
	var err := Assert.eq(s.get_connection_status(), "waiting", "status after start")
	if err:
		return err
	s.stop()
	return Assert.eq(s.get_connection_status(), "stopped", "status after stop")


func test_tcp_server_set_idle_timeout() -> String:
	var s := StageTCPServer.new()
	s.set_idle_timeout(30)
	s.set_idle_timeout(0)   # 0 = disabled
	return ""  # no crash = pass


func test_tcp_server_has_activity_signal() -> String:
	var s := StageTCPServer.new()
	return Assert.obj_has_signal(s, "activity_received")


func test_recorder_has_signals() -> String:
	var r := StageRecorder.new()
	for sig in ["marker_added", "dashcam_clip_saved", "dashcam_clip_started"]:
		var err := Assert.obj_has_signal(r, sig)
		if err:
			return err
	return ""


func test_collector_initial_counts() -> String:
	var c := StageCollector.new()
	var err := Assert.eq(c.get_tracked_count(), 0, "tracked count")
	if err:
		return err
	return Assert.eq(c.get_group_count(), 0, "group count")
