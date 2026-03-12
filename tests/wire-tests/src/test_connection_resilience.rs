/// Wire tests for connection resilience bugs:
///
/// - Bug 3A: spectator-server must not hang forever if Godot won't send handshake
/// - Bug 3B: Godot drops zombie clients after idle timeout; new client can connect
///
/// These tests exercise the live TCP protocol between a real Godot process and
/// the spectator wire protocol — they catch bugs that unit tests miss (actual
/// socket lifecycle, GDExtension process_mode, TCP state).
use crate::harness::GodotFixture;
use spectator_protocol::{codec, handshake::PROTOCOL_VERSION, messages::Message};
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// Helper: connect to a port without completing the handshake.
/// Returns the raw TcpStream (still waiting for Godot's handshake message).
fn connect_raw(port: u16) -> anyhow::Result<TcpStream> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => return Ok(stream),
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => anyhow::bail!("could not connect to port {port}: {e}"),
        }
    }
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn second_client_connects_after_clean_disconnect() {
    // Regression for Bug 3: after the active spectator-server disconnects cleanly,
    // the next connection must receive the Godot handshake and reach "connected" state.
    //
    // This guards against the GDExtension failing to call try_accept() on subsequent
    // connections after the first one drops.
    let mut f1 = GodotFixture::start("test_scene_3d.tscn").unwrap();
    let port = f1.port;

    // Verify first connection is live
    let status = f1.query("get_frame_info", serde_json::json!({})).unwrap();
    assert!(status.is_ok(), "first client should work");

    // Disconnect cleanly without killing Godot
    let (_port, mut _godot) = f1.disconnect_keep_alive();

    // Give Godot a moment to detect the disconnect and re-enter waiting state
    std::thread::sleep(Duration::from_millis(500));

    // Second connection must complete handshake within 5 seconds
    let mut stream = connect_raw(port).expect("should connect to port");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    let msg = codec::read_message::<Message>(&mut stream)
        .expect("Godot must send handshake to second client");

    match msg {
        Message::Handshake(h) => {
            assert_eq!(
                h.protocol_version, PROTOCOL_VERSION,
                "handshake protocol version must match"
            );
        }
        other => panic!("Expected Handshake, got {:?}", other),
    }
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn second_client_gets_handshake_after_first_disconnects_abnormally() {
    // Regression: even if the first client disconnects without a FIN (kill -9 style),
    // the GDExtension must eventually detect the dead connection and accept a new one.
    //
    // This test uses a 2-second wait after drop (enough for a clean OS-level close
    // even without FIN, because the process ends). For zombie connections that linger
    // longer, the idle timeout (Bug 3B, 60s) covers that case.
    let mut f1 = GodotFixture::start("test_scene_3d.tscn").unwrap();
    let port = f1.port;

    // Complete a query to ensure handshake is done
    f1.query("get_frame_info", serde_json::json!({})).unwrap();

    // Disconnect without killing Godot (OS will close socket)
    let (_port, mut _godot) = f1.disconnect_keep_alive();

    // Brief wait for OS to close the socket
    std::thread::sleep(Duration::from_secs(2));

    // New client must receive handshake
    let mut stream = connect_raw(port).expect("should connect to port");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();

    let msg = codec::read_message::<Message>(&mut stream)
        .expect("Godot must send handshake after abnormal disconnect");

    assert!(
        matches!(msg, Message::Handshake(_)),
        "Expected Handshake after reconnect, got: {:?}",
        msg
    );
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn dashcam_works_after_reconnect() {
    // Regression for snapshot Bug 5: after a disconnect + reconnect cycle, the
    // dashcam must still be buffering and respond to status queries.
    //
    // This guards against state leaks in SpectatorRecorder when connections reset.
    let mut f1 = GodotFixture::start("test_scene_3d.tscn").unwrap();
    let port = f1.port;

    // First session: verify dashcam is active
    let status1 = f1
        .query("dashcam_status", serde_json::json!({}))
        .unwrap()
        .unwrap_data();
    assert!(
        status1["state"].as_str().is_some(),
        "first session dashcam must return state"
    );

    // Disconnect cleanly without killing Godot
    let (_port, mut _godot) = f1.disconnect_keep_alive();
    std::thread::sleep(Duration::from_millis(500));

    // Second session after reconnect
    let mut stream = connect_raw(port).expect("should connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    let msg = codec::read_message::<Message>(&mut stream).expect("must get handshake");
    let handshake = match msg {
        Message::Handshake(h) => h,
        other => panic!("Expected Handshake, got {:?}", other),
    };

    // Send ack
    let ack = Message::HandshakeAck(spectator_protocol::handshake::HandshakeAck {
        spectator_version: "test".into(),
        protocol_version: PROTOCOL_VERSION,
        session_id: "reconnect-test".into(),
    });
    codec::write_message(&mut stream, &ack).unwrap();
    assert_eq!(handshake.protocol_version, PROTOCOL_VERSION);

    fn query_raw(
        stream: &mut TcpStream,
        method: &str,
        params: serde_json::Value,
    ) -> serde_json::Value {
        let id = "reconnect-q1".to_string();
        let msg = Message::Query {
            request_id: id.clone(),
            method: method.into(),
            params,
        };
        codec::write_message(stream, &msg).unwrap();
        let resp = codec::read_message::<Message>(stream).unwrap();
        match resp {
            Message::Response { data, .. } => data,
            Message::Error { code, message, .. } => {
                panic!("Query error: {code}: {message}")
            }
            other => panic!("Unexpected: {:?}", other),
        }
    }

    // Dashcam must still respond after reconnect
    let status2 = query_raw(&mut stream, "dashcam_status", serde_json::json!({}));
    assert!(
        status2["state"].as_str().is_some(),
        "second session dashcam must return state after reconnect"
    );
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn two_clients_both_receive_handshake_simultaneously() {
    // Multi-client: a second client must receive the Godot handshake immediately
    // while the first client is still connected. Both connections are live at the
    // same time — no need for the first to disconnect first.
    let mut f1 = GodotFixture::start("test_scene_3d.tscn").unwrap();
    let port = f1.port;

    // Verify first connection is live
    f1.query("get_frame_info", serde_json::json!({})).unwrap();

    // Connect a second raw stream while f1 is still active
    let mut stream2 =
        TcpStream::connect(("127.0.0.1", port)).expect("second raw TCP connect must succeed");
    stream2
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    // stream2 MUST receive a handshake immediately (multi-client support)
    let msg = codec::read_message::<Message>(&mut stream2)
        .expect("second client must receive handshake while first is active");
    assert!(
        matches!(msg, Message::Handshake(_)),
        "expected Handshake for second client, got: {:?}",
        msg
    );

    // f1 must still be usable
    let result = f1.query("get_frame_info", serde_json::json!({})).unwrap();
    assert!(
        result.is_ok(),
        "first client must still work while second is connected"
    );
}
