//! Project selection through the persistent MCP transport, including whole-call isolation.
mod support;

use rmcp::{
    ServiceExt,
    model::{CallToolRequestParams, CallToolResult},
};
use serde_json::{Value, json};
use stage_protocol::{handshake::Handshake, runtime::RuntimeIdentity};
use stage_server::{server::StageServer, tcp::SessionState};
use std::{path::Path, sync::Arc, time::Duration};
use support::mock_addon::MockAddon;
use tokio::sync::Mutex;

type Client = rmcp::service::RunningService<rmcp::RoleClient, ()>;

async fn call(client: &Client, name: &str, params: Value) -> Result<CallToolResult, String> {
    client
        .call_tool(CallToolRequestParams {
            name: name.to_owned().into(),
            arguments: params.as_object().cloned(),
            meta: None,
            task: None,
        })
        .await
        .map_err(|error| error.to_string())
}

async fn ok(client: &Client, name: &str, params: Value) -> Value {
    let result = call(client, name, params).await.unwrap();
    assert_ne!(result.is_error, Some(true), "{result:?}");
    let text: Value = serde_json::from_str(&result.content[0].as_text().unwrap().text).unwrap();
    if let Some(value) = result.structured_content {
        assert_eq!(text, value);
    }
    text
}

fn project(label: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("project.godot"), "config_version=5\n").unwrap();
    let feedback = dir
        .path()
        .join(format!(".theatre/feedback/feedback_{label}"));
    std::fs::create_dir_all(&feedback).unwrap();
    std::fs::write(
        feedback.join("item.json"),
        json!({
            "feedback_id":format!("feedback_{label}"), "source":"runtime", "timestamp_ms":1,
            "project_path":dir.path(), "process_id":7, "run_id":"run_7", "scene":"res://main.tscn",
            "surface":"root_viewport", "selection":[], "pointer":{"status":"unavailable"},
            "capture":{"status":"unavailable","reason":"headless"},
            "readback_render_frame":1,"readback_physics_frame":2,"note":label
        })
        .to_string(),
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join(".stage")).unwrap();
    std::fs::create_dir_all(dir.path().join("clips")).unwrap();
    std::fs::write(
        dir.path().join(".stage/clip_storage_path"),
        dir.path().join("clips").to_string_lossy().as_bytes(),
    )
    .unwrap();
    dir
}

async fn peer(project: &Path, label: &str) -> MockAddon {
    let identity = RuntimeIdentity {
        project_path: project.to_string_lossy().into_owned(),
        process_id: 42,
        run_id: format!("run_{label}"),
    };
    let handshake = Handshake::new("4.7".into(), 3, 60, label.into(), identity.clone());
    MockAddon::start_with_handshake(
        handshake,
        Arc::new(move |method, _params| {
            if method == "recording_list" {
                return Err(("unavailable".into(), "Use saved clip storage".into()));
            }
            Ok(match method {
                "runtime_status" => {
                    json!({"identity":identity,"ready":true,"current_scene":"res://main.tscn"})
                }
                "get_snapshot_data" => {
                    serde_json::to_value(support::fixtures::mock_scene_3d()).unwrap()
                }
                "dashcam_config" => json!({}),
                _ => panic!("unexpected {method}"),
            })
        }),
    )
    .await
}

#[tokio::test]
async fn switch_resets_state_routes_retained_evidence_and_never_falls_back() {
    let a = project("a");
    let b = project("b");
    let first_peer = peer(a.path(), "a").await;
    let second_peer = peer(b.path(), "b").await;
    std::fs::write(
        b.path().join("stage.toml"),
        format!(
            "[connection]\nport={}\n[tracking]\ntoken_hard_cap=4321\n[dashcam]\nenabled=false\n",
            second_peer.port
        ),
    )
    .unwrap();
    let state = Arc::new(Mutex::new(SessionState::default()));
    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let server_task = tokio::spawn(StageServer::new(state.clone()).serve(server_io));
    let client = ().serve(client_io).await.unwrap();
    let server = server_task.await.unwrap().unwrap();
    let tools = client.list_all_tools().await.unwrap();
    let select = tools.iter().find(|t| t.name == "project_select").unwrap();
    assert!(select.description.as_ref().unwrap().contains("DISCARDS"));
    assert!(select.output_schema.as_ref().unwrap()["properties"]["project_path"].is_object());

    let first = ok(
        &client,
        "project_select",
        json!({"project_path":a.path(),"port":first_peer.port}),
    )
    .await;
    assert_eq!(first["identity"]["run_id"], "run_a");
    assert_eq!(first["ready"], true);
    ok(
        &client,
        "spatial_watch",
        json!({"action":"add","watch":{"node":"player"}}),
    )
    .await;
    ok(&client, "spatial_config", json!({"token_hard_cap":1234})).await;
    ok(&client, "spatial_snapshot", json!({})).await;
    ok(&client, "clips", json!({"action":"list"})).await;
    assert!(state.lock().await.delta_engine.has_baseline());
    assert!(state.lock().await.clip_storage_path.is_some());

    for invalid in [
        json!({"project_path":"relative"}),
        json!({"project_path":a.path().join("missing")}),
        json!({"project_path":a.path().join("clips")}),
        json!({"project_path":a.path(),"port":0}),
    ] {
        assert!(call(&client, "project_select", invalid).await.is_err());
        let status = ok(&client, "runtime_status", json!({})).await;
        assert_eq!(status["session_id"], first["session_id"]);
        assert!(state.lock().await.delta_engine.has_baseline());
    }
    let second = ok(&client, "project_select", json!({"project_path":b.path()})).await;
    assert_eq!(second["project_path"], b.path().to_str().unwrap());
    assert_eq!(second["port"], second_peer.port);
    assert_eq!(second["identity"]["run_id"], "run_b");
    assert!(
        second["message"]
            .as_str()
            .unwrap()
            .contains("fresh spatial_snapshot")
    );
    assert_eq!(second["cleared"].as_array().unwrap().len(), 5);
    assert_eq!(
        ok(&client, "spatial_watch", json!({"action":"list"})).await["watches"],
        json!([])
    );
    assert_eq!(
        ok(&client, "spatial_config", json!({})).await["config"]["token_hard_cap"],
        4321
    );
    assert!(call(&client, "spatial_delta", json!({})).await.is_err());
    assert!(state.lock().await.clip_storage_path.is_none());
    ok(&client, "clips", json!({"action":"list"})).await;
    assert_eq!(
        state.lock().await.clip_storage_path.as_deref(),
        b.path().join("clips").to_str()
    );
    let feedback = ok(
        &client,
        "feedback",
        json!({"action":"retrieve","feedback_id":"feedback_b"}),
    )
    .await;
    assert!(feedback.to_string().contains("feedback_b"));
    assert!(
        call(
            &client,
            "feedback",
            json!({"action":"retrieve","feedback_id":"feedback_a"})
        )
        .await
        .is_err()
    );

    // A wrong runtime must be rejected before any configuration is dispatched.
    let wrong_peer = peer(a.path(), "wrong").await;
    let wrong = ok(
        &client,
        "project_select",
        json!({"project_path":b.path(),"port":wrong_peer.port}),
    )
    .await;
    assert_eq!(wrong["project_path"], b.path().to_str().unwrap());
    assert_eq!(wrong["connected"], false);
    assert!(wrong["identity"].is_null());
    assert!(
        wrong["diagnostic"]
            .as_str()
            .unwrap()
            .contains("project mismatch")
    );

    // Reserve a free TCP port then release it, matching the existing test harness.
    let socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let unavailable_port = socket.local_addr().unwrap().port();
    drop(socket);
    let unavailable = ok(
        &client,
        "project_select",
        json!({"project_path":b.path(),"port":unavailable_port}),
    )
    .await;
    assert_eq!(unavailable["connected"], false);
    assert!(unavailable["identity"].is_null());
    assert_eq!(
        ok(&client, "runtime_status", json!({})).await["project_path"],
        b.path().to_str().unwrap()
    );

    let returning_peer = peer(a.path(), "return").await;
    let back = ok(
        &client,
        "project_select",
        json!({"project_path":a.path(),"port":returning_peer.port}),
    )
    .await;
    assert_eq!(back["identity"]["run_id"], "run_return");
    assert_eq!(
        ok(&client, "spatial_watch", json!({"action":"list"})).await["watches"],
        json!([])
    );
    assert_eq!(
        ok(&client, "spatial_config", json!({})).await["config"]["token_hard_cap"],
        5000
    );
    assert!(call(&client, "spatial_delta", json!({})).await.is_err());
    ok(&client, "spatial_snapshot", json!({})).await;
    ok(&client, "spatial_delta", json!({})).await;
    client.cancel().await.unwrap();
    server.cancel().await.unwrap();
}

#[tokio::test]
async fn switching_waits_for_a_snapshot_handler_before_discarding_its_baseline() {
    use stage_protocol::{codec::async_io, messages::Message};
    let a = project("a");
    let b = project("b");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let identity = RuntimeIdentity {
        project_path: a.path().to_string_lossy().into_owned(),
        process_id: 1,
        run_id: "run_a".into(),
    };
    let (received_tx, received_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let peer = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        async_io::write_message(
            &mut stream,
            &Message::Handshake(Handshake::new(
                "4.7".into(),
                3,
                60,
                "A".into(),
                identity.clone(),
            )),
        )
        .await
        .unwrap();
        let _: Message = async_io::read_message(&mut stream).await.unwrap();
        let mut received_tx = Some(received_tx);
        let mut release_rx = Some(release_rx);
        while let Ok(message) = async_io::read_message::<Message>(&mut stream).await {
            if let Message::Query {
                request_id, method, ..
            } = message
            {
                let data = match method.as_str() {
                    "runtime_status" => {
                        json!({"identity":identity,"ready":true,"current_scene":"res://a.tscn"})
                    }
                    "get_snapshot_data" => {
                        received_tx.take().unwrap().send(()).unwrap();
                        release_rx.take().unwrap().await.unwrap();
                        serde_json::to_value(support::fixtures::mock_scene_3d()).unwrap()
                    }
                    _ => panic!("unexpected {method}"),
                };
                async_io::write_message(&mut stream, &Message::Response { request_id, data })
                    .await
                    .unwrap();
            }
        }
    });
    let state = Arc::new(Mutex::new(SessionState::default()));
    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let task = tokio::spawn(StageServer::new(state.clone()).serve(server_io));
    let client = ().serve(client_io).await.unwrap();
    let server = task.await.unwrap().unwrap();
    ok(
        &client,
        "project_select",
        json!({"project_path":a.path(),"port":port}),
    )
    .await;
    {
        let snapshot = call(&client, "spatial_snapshot", json!({}));
        tokio::pin!(snapshot);
        tokio::select! { _ = &mut snapshot => panic!("snapshot must wait"), _ = received_rx => {} }
        let new_peer = self::peer(b.path(), "b").await;
        let selection = ok(
            &client,
            "project_select",
            json!({"project_path":b.path(),"port":new_peer.port}),
        );
        tokio::pin!(selection);
        tokio::select! {
            _ = &mut selection => panic!("switch must drain the old call"),
            _ = &mut snapshot => panic!("snapshot still waits"),
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
        release_tx.send(()).unwrap();
        let (snapshot, selected) = tokio::join!(snapshot, selection);
        assert_ne!(snapshot.unwrap().is_error, Some(true));
        assert_eq!(selected["identity"]["run_id"], "run_b");
        assert!(
            !state.lock().await.delta_engine.has_baseline(),
            "old handler must not repopulate B's baseline"
        );
        assert!(call(&client, "spatial_delta", json!({})).await.is_err());
        peer.await.unwrap();
    }
    client.cancel().await.unwrap();
    server.cancel().await.unwrap();
}

#[tokio::test]
#[ignore = "requires Godot and deployed GDExtension"]
async fn real_games_switch_in_one_mcp_session_and_return_to_the_original_run() {
    // Copy only source fixture content, reusing the deployed platform payload.
    // Hard links avoid copying large binaries; copying is the cross-device fallback.
    fn copy_tree(source: &Path, destination: &Path) {
        std::fs::create_dir_all(destination).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            if matches!(
                name.to_str(),
                Some(".godot" | ".stage" | ".theatre" | "tmp")
            ) {
                continue;
            }
            let target = destination.join(name);
            if entry.path().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                let binary = matches!(
                    entry.path().extension().and_then(|ext| ext.to_str()),
                    Some("so" | "dylib" | "dll")
                );
                if !binary || std::fs::hard_link(entry.path(), &target).is_err() {
                    std::fs::copy(entry.path(), &target).unwrap();
                }
            }
        }
    }
    let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/godot-project")
        .canonicalize()
        .unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    // This is a runtime-selection journey, not an editor-import journey. Reuse
    // the fixture's generated res:// discovery metadata in both initialized copies.
    // A clean checkout still initializes the source through the existing harness.
    if !source.join(".godot/extension_list.cfg").exists() {
        drop(
            support::godot_process::GodotProcess::start_3d()
                .await
                .unwrap(),
        );
    }
    for project in [a.path(), b.path()] {
        copy_tree(&source, project);
        std::fs::create_dir_all(project.join(".godot")).unwrap();
        for cache in ["extension_list.cfg", "global_script_class_cache.cfg"] {
            std::fs::copy(
                source.join(".godot").join(cache),
                project.join(".godot").join(cache),
            )
            .unwrap();
        }
    }
    let game_a = support::godot_process::GodotProcess::start_in_project(
        a.path(),
        "res://test_scene_3d.tscn",
    )
    .await
    .unwrap();
    let game_b = support::godot_process::GodotProcess::start_in_project(
        b.path(),
        "res://test_scene_2d.tscn",
    )
    .await
    .unwrap();
    let state = Arc::new(Mutex::new(SessionState::default()));
    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let task = tokio::spawn(StageServer::new(state).serve(server_io));
    let client = ().serve(client_io).await.unwrap();
    let server = task.await.unwrap().unwrap();
    let first = ok(
        &client,
        "project_select",
        json!({"project_path":a.path(),"port":game_a.port()}),
    )
    .await;
    assert_eq!(first["ready"], true, "{first}");
    assert_eq!(first["current_scene"], "res://test_scene_3d.tscn");
    ok(&client, "spatial_snapshot", json!({})).await;
    ok(
        &client,
        "spatial_watch",
        json!({"action":"add","watch":{"node":"Player"}}),
    )
    .await;
    let second = ok(
        &client,
        "project_select",
        json!({"project_path":b.path(),"port":game_b.port()}),
    )
    .await;
    assert_eq!(second["ready"], true, "{second}");
    assert_eq!(second["current_scene"], "res://test_scene_2d.tscn");
    assert_ne!(first["identity"]["run_id"], second["identity"]["run_id"]);
    assert_eq!(
        std::path::PathBuf::from(second["identity"]["project_path"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        b.path()
    );
    let tree = ok(&client, "scene_tree", json!({"action":"roots"})).await;
    assert!(tree.to_string().contains("TestScene2D"), "{tree}");
    ok(
        &client,
        "spatial_inspect",
        json!({"node":"/root/TestScene2D/Player","include":["transform"]}),
    )
    .await;
    assert!(call(&client, "spatial_delta", json!({})).await.is_err());
    ok(&client, "spatial_snapshot", json!({})).await;
    ok(&client, "spatial_delta", json!({})).await;
    let returned = ok(
        &client,
        "project_select",
        json!({"project_path":a.path(),"port":game_a.port()}),
    )
    .await;
    assert_eq!(
        returned["identity"], first["identity"],
        "switching must not stop or restart the old game"
    );
    assert_ne!(returned["session_id"], first["session_id"]);
    assert_eq!(
        ok(&client, "spatial_watch", json!({"action":"list"})).await["watches"],
        json!([])
    );
    assert!(call(&client, "spatial_delta", json!({})).await.is_err());
    ok(&client, "spatial_snapshot", json!({})).await;
    let reselected = ok(
        &client,
        "project_select",
        json!({"project_path":a.path(),"port":game_a.port()}),
    )
    .await;
    assert_eq!(reselected["identity"], first["identity"]);
    assert_ne!(reselected["session_id"], returned["session_id"]);
    assert!(call(&client, "spatial_delta", json!({})).await.is_err());
    client.cancel().await.unwrap();
    server.cancel().await.unwrap();
}
