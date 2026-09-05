use std::sync::Arc;
use std::time::Duration;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};
use stage_protocol::{
    codec::async_io, handshake::Handshake, messages::Message, runtime::RuntimeIdentity,
};
use stage_server::{
    mcp::runtime_status::RuntimeStatusParams,
    server::StageServer,
    tcp::{self, SessionState},
};
use tempfile::TempDir;
use tokio::{net::TcpListener, sync::Mutex};

fn identity(project: &std::path::Path) -> RuntimeIdentity {
    RuntimeIdentity {
        project_path: project.to_string_lossy().into_owned(),
        process_id: 42,
        run_id: "run_peer".into(),
    }
}

fn state(project: &std::path::Path) -> Arc<Mutex<SessionState>> {
    Arc::new(Mutex::new(SessionState {
        project_dir: project.into(),
        ..Default::default()
    }))
}

async fn status(state: &Arc<Mutex<SessionState>>) -> Value {
    let server = StageServer::new(state.clone());
    serde_json::from_str(
        &server
            .runtime_status(Parameters(RuntimeStatusParams {}))
            .await
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn wrong_project_fails_before_ack_or_config_and_preserves_diagnostic() {
    let selected = TempDir::new().unwrap();
    let actual = TempDir::new().unwrap();
    std::fs::write(selected.path().join("project.godot"), "").unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handshake = Handshake::new("4.7".into(), 3, 60, "Wrong".into(), identity(actual.path()));
    let peer = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        async_io::write_message(&mut stream, &Message::Handshake(handshake))
            .await
            .unwrap();
        assert!(
            async_io::read_message::<Message>(&mut stream)
                .await
                .is_err(),
            "rejected project must receive neither ACK nor config"
        );
    });
    let state = state(selected.path());
    state.lock().await.config.dashcam_explicit = true;
    let error = tcp::connect_once(&state, port)
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains("project mismatch"), "{error}");
    assert!(error.contains(&actual.path().display().to_string()));
    assert!(error.contains("THEATRE_PROJECT_DIR"));
    peer.await.unwrap();
    let result = status(&state).await;
    assert_eq!(result["connected"], false);
    assert_eq!(result["ready"], false);
    assert!(result["identity"].is_null());
    assert!(result["session_id"].is_null());
    assert!(result["current_scene"].is_null());
    assert!(
        result["diagnostic"]
            .as_str()
            .unwrap()
            .contains("project mismatch")
    );
}

async fn current_scene_journey(selected: &std::path::Path, actual: &std::path::Path) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let identity = identity(actual);
    let expected = identity.clone();
    let (close_tx, mut close_rx) = tokio::sync::oneshot::channel::<()>();
    let peer = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let handshake = Handshake::new("4.7".into(), 3, 60, "Actual".into(), identity.clone());
        async_io::write_message(&mut stream, &Message::Handshake(handshake))
            .await
            .unwrap();
        assert!(matches!(
            async_io::read_message::<Message>(&mut stream)
                .await
                .unwrap(),
            Message::HandshakeAck(_)
        ));
        let mut scene_number = 0;
        loop {
            tokio::select! {
                _ = &mut close_rx => break,
                msg = async_io::read_message::<Message>(&mut stream) => {
                    match msg.unwrap() {
                        Message::Query { request_id, method, .. } => {
                            assert_eq!(method, "runtime_status");
                            scene_number += 1;
                            async_io::write_message(&mut stream, &Message::Response {
                                request_id,
                                data: json!({"identity": identity, "ready": true, "current_scene": format!("res://scene_{scene_number}.tscn")}),
                            }).await.unwrap();
                        }
                        Message::Event { .. } => {}, // best-effort MCP activity log
                        other => panic!("unexpected {other:?}"),
                    }
                }
            }
        }
    });
    let state = state(selected);
    tcp::connect_once(&state, port).await.unwrap();
    let first = status(&state).await;
    assert_eq!(first["identity"], json!(expected));
    assert_eq!(first["connected"], true);
    assert_eq!(first["ready"], true);
    assert_eq!(first["current_scene"], "res://scene_1.tscn");
    assert_ne!(first["session_id"], first["identity"]["run_id"]);
    let second = status(&state).await;
    assert_eq!(
        second["current_scene"], "res://scene_2.tscn",
        "scene must be queried, not captured at launch"
    );
    close_tx.send(()).unwrap();
    peer.await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while state.lock().await.connected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let disconnected = status(&state).await;
    assert_eq!(disconnected["connected"], false);
    assert_eq!(disconnected["ready"], false);
    assert!(disconnected["identity"].is_null());
    assert!(disconnected["session_id"].is_null());
    assert!(disconnected["current_scene"].is_null());
    assert!(state.lock().await.handshake_info.is_none());
}

#[tokio::test]
async fn discovery_outside_a_project_reports_actual_identity_and_clears_on_disconnect() {
    let outside = TempDir::new().unwrap();
    let actual = TempDir::new().unwrap();
    current_scene_journey(outside.path(), actual.path()).await;
}

#[cfg(unix)]
#[tokio::test]
async fn selected_project_symlink_alias_connects() {
    let actual = TempDir::new().unwrap();
    std::fs::write(actual.path().join("project.godot"), "").unwrap();
    let aliases = TempDir::new().unwrap();
    let alias = aliases.path().join("alias");
    std::os::unix::fs::symlink(actual.path(), &alias).unwrap();
    current_scene_journey(&alias, actual.path()).await;
}

#[tokio::test]
async fn disconnect_during_status_query_does_not_return_retained_identity() {
    let project = TempDir::new().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handshake = Handshake::new(
        "4.7".into(),
        3,
        60,
        "Actual".into(),
        identity(project.path()),
    );
    let peer = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        async_io::write_message(&mut stream, &Message::Handshake(handshake))
            .await
            .unwrap();
        assert!(matches!(
            async_io::read_message::<Message>(&mut stream)
                .await
                .unwrap(),
            Message::HandshakeAck(_)
        ));
        assert!(
            matches!(async_io::read_message::<Message>(&mut stream).await.unwrap(), Message::Query { method, .. } if method == "runtime_status")
        );
        // Consume the request, then disconnect without returning state.
    });
    let state = state(project.path());
    tcp::connect_once(&state, port).await.unwrap();
    let result = status(&state).await;
    peer.await.unwrap();
    assert_eq!(result["connected"], false);
    assert_eq!(result["ready"], false);
    assert!(result["identity"].is_null());
    assert!(result["current_scene"].is_null());
    assert!(
        result["diagnostic"]
            .as_str()
            .unwrap()
            .contains("disconnected")
    );
}

#[test]
fn runtime_status_has_typed_input_and_output_schema() {
    let router = StageServer::router_with_schemas();
    let tool = &router.map["runtime_status"].attr;
    assert!(tool.output_schema.is_some());
    let schema = tool.output_schema.as_ref().unwrap();
    assert!(schema["properties"]["identity"].is_object());
}

#[test]
fn cli_runtime_status_reports_disconnected_instead_of_stale_identity() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let project = TempDir::new().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_stage"))
        .args(["runtime_status", "{}"])
        .env("THEATRE_PROJECT_DIR", project.path())
        .env("THEATRE_PORT", port.to_string())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["connected"], false);
    assert!(result["identity"].is_null());
    assert!(
        result["diagnostic"]
            .as_str()
            .unwrap()
            .contains("TCP connection failed")
    );
}
