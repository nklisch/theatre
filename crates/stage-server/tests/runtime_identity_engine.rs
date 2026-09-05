#[path = "support/godot_process.rs"]
mod godot_process;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::Value;
use stage_server::{
    mcp::{runtime_diagnostics::RuntimeDiagnosticsParams, runtime_status::RuntimeStatusParams},
    server::StageServer,
    tcp::{self, SessionState},
};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::{io::AsyncWriteExt, sync::Mutex};

async fn status(state: &Arc<Mutex<SessionState>>) -> Value {
    let server = StageServer::new(state.clone());
    server
        .runtime_status(Parameters(RuntimeStatusParams {}))
        .await
        .unwrap()
        .structured_content
        .unwrap()
}

async fn disconnected(state: &Arc<Mutex<SessionState>>) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while state.lock().await.connected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let result = status(state).await;
    assert_eq!(result["connected"], false);
    assert!(result["identity"].is_null());
    assert!(result["current_scene"].is_null());
}

#[tokio::test]
#[ignore = "requires Godot and deployed GDExtension"]
async fn real_engine_identity_survives_reconnect_changes_on_restart_and_rejects_wrong_project() {
    let game = godot_process::GodotProcess::start_3d().await.unwrap();
    let wrong = tempfile::TempDir::new().unwrap();
    std::fs::write(wrong.path().join("project.godot"), "").unwrap();
    let wrong_state = Arc::new(Mutex::new(SessionState {
        project_dir: wrong.path().into(),
        ..Default::default()
    }));
    wrong_state.lock().await.project_dashcam_config =
        Some(stage_protocol::dashcam::DashcamConfigPatch {
            enabled: Some(false),
            ..Default::default()
        });
    let error = tcp::connect_once(&wrong_state, game.port())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("project mismatch"), "{error}");
    assert!(status(&wrong_state).await["identity"].is_null());

    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/godot-project")
        .canonicalize()
        .unwrap();
    let state = Arc::new(Mutex::new(SessionState {
        project_dir: project.clone(),
        ..Default::default()
    }));
    tcp::connect_once(&state, game.port()).await.unwrap();
    let first = status(&state).await;
    assert_eq!(first["ready"], true);
    assert_eq!(first["current_scene"], "res://test_scene_3d.tscn");
    assert_eq!(
        PathBuf::from(first["identity"]["project_path"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        project
    );
    assert!(first["identity"]["process_id"].as_u64().unwrap() > 0);

    state
        .lock()
        .await
        .tcp_writer
        .as_mut()
        .unwrap()
        .writer
        .shutdown()
        .await
        .unwrap();
    disconnected(&state).await;
    let disconnected_diagnostics = StageServer::new(state.clone())
        .runtime_diagnostics(rmcp::handler::server::wrapper::Parameters(
            RuntimeDiagnosticsParams {
                max_entries: 20,
                before_sequence: None,
                token_budget: None,
            },
        ))
        .await
        .unwrap_err();
    assert!(
        disconnected_diagnostics.message.contains("not connected"),
        "disconnected diagnostics must not return retained run evidence: {}",
        disconnected_diagnostics.message
    );
    tcp::connect_once(&state, game.port()).await.unwrap();
    let reconnected = status(&state).await;
    assert_eq!(first["identity"], reconnected["identity"]);
    assert_ne!(first["session_id"], reconnected["session_id"]);

    drop(game);
    disconnected(&state).await;
    let restarted = godot_process::GodotProcess::start_2d().await.unwrap();
    tcp::connect_once(&state, restarted.port()).await.unwrap();
    let second = status(&state).await;
    assert_ne!(first["identity"]["run_id"], second["identity"]["run_id"]);
    assert_eq!(second["current_scene"], "res://test_scene_2d.tscn");
    assert_eq!(second["ready"], true);
}
