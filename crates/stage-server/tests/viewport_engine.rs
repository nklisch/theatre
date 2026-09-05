use base64::Engine as _;
use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;
use stage_protocol::viewport::ViewportParams;
use stage_server::{
    mcp::viewport::handle_viewport_cli,
    server::StageServer,
    tcp::{self, SessionState},
};
use std::{
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

struct Game {
    port: u16,
    child: Child,
    _dir: tempfile::TempDir,
}
impl Drop for Game {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

async fn start(headless: bool) -> (Game, Arc<Mutex<SessionState>>) {
    start_with_connection(headless, true).await
}

async fn start_with_connection(headless: bool, connect: bool) -> (Game, Arc<Mutex<SessionState>>) {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    // An isolated project uses the deployed addon without altering the shared test project.
    std::fs::create_dir_all(dir.path().join("addons/stage/bin/linux")).unwrap();
    let addon = repo.join("addons/stage");
    std::fs::copy(
        addon.join("stage.gdextension"),
        dir.path().join("addons/stage/stage.gdextension"),
    )
    .unwrap();
    let (platform, library) = if cfg!(target_os = "windows") {
        ("windows", "stage_godot.dll")
    } else if cfg!(target_os = "macos") {
        ("macos", "libstage_godot.dylib")
    } else {
        ("linux", "libstage_godot.so")
    };
    let bin_dir = dir.path().join("addons/stage/bin").join(platform);
    std::fs::create_dir_all(&bin_dir).unwrap();
    let executable = std::env::current_exe().unwrap();
    let build_dir = executable.parent().unwrap().parent().unwrap();
    std::fs::copy(build_dir.join(library), bin_dir.join(library)).unwrap();
    std::fs::write(dir.path().join("project.godot"), "config_version=5\n[application]\nconfig/name=\"Viewport test\"\n[display]\nwindow/size/viewport_width=2560\nwindow/size/viewport_height=1440\n[rendering]\nrenderer/rendering_method=\"gl_compatibility\"\n").unwrap();
    std::fs::write(
        dir.path().join("main.gd"),
        r#"extends SceneTree
func _initialize():
    GDExtensionManager.load_extension("res://addons/stage/stage.gdextension")
    root.size = Vector2i(1280, 720)
    root.content_scale_size = Vector2i(2560, 1440)
    root.content_scale_mode = Window.CONTENT_SCALE_MODE_VIEWPORT
    var scene = Node2D.new()
    scene.name = "Visual"
    root.add_child(scene)
    current_scene = scene
    var color = ColorRect.new()
    color.name = "Color"
    color.color = Color.RED
    color.size = Vector2(2560, 1440)
    scene.add_child(color)
    var collector = ClassDB.instantiate("StageCollector")
    root.add_child(collector)
    var recorder = ClassDB.instantiate("StageRecorder")
    recorder.set_dashcam_enabled(false)
    root.add_child(recorder)
    recorder.set_collector(collector)
    var server = ClassDB.instantiate("StageTCPServer")
    root.add_child(server)
    server.set_collector(collector)
    server.set_recorder(recorder)
    server.start(int(OS.get_environment("THEATRE_PORT")))
    physics_frame.connect(server.poll)
"#,
    )
    .unwrap();
    let godot = std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into());
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut cmd = Command::new(godot);
    if headless {
        cmd.arg("--headless");
    }
    let child = cmd
        .args(["--path"])
        .arg(dir.path())
        .args(["--script", "res://main.gd"])
        .env("THEATRE_PORT", port.to_string())
        .env("XDG_DATA_HOME", dir.path().join("user-data"))
        .env("APPDATA", dir.path().join("user-data"))
        .stdout(Stdio::from(
            std::fs::File::create(dir.path().join("godot.log")).unwrap(),
        ))
        .spawn()
        .unwrap();
    let game = Game {
        port,
        child,
        _dir: dir,
    };
    let state = Arc::new(Mutex::new(SessionState {
        project_dir: game._dir.path().into(),
        ..Default::default()
    }));
    if !connect {
        // Wait for the fixture's listener without consuming its single client slot.
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if TcpListener::bind(("127.0.0.1", port))
                    .is_err_and(|error| error.kind() == std::io::ErrorKind::AddrInUse)
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("fixture listener became ready");
        return (game, state);
    }
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if tcp::connect_once(&state, port).await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap_or_else(|error| {
        panic!(
            "{error}: {}",
            std::fs::read_to_string(game._dir.path().join("godot.log")).unwrap_or_default()
        )
    });
    (game, state)
}

fn decode(result: &serde_json::Value) -> (Vec<u8>, jpeg_decoder::ImageInfo) {
    assert_eq!(result["status"], "available");
    assert_eq!(result["mime_type"], "image/jpeg");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(result["image_base64"].as_str().unwrap())
        .unwrap();
    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(bytes));
    let pixels = decoder.decode().unwrap();
    (pixels, decoder.info().unwrap())
}

#[tokio::test]
#[ignore = "requires graphical Godot and built GDExtension"]
async fn graphical_viewport_changes_without_recording_and_reports_bounded_latency() {
    let (game, state) = start(false).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let capture = |size| {
        handle_viewport_cli(
            ViewportParams {
                max_dimension: size,
            },
            &state,
        )
    };
    let started = Instant::now();
    let first: serde_json::Value = serde_json::from_str(&capture(1280).await.unwrap()).unwrap();
    eprintln!(
        "Default viewport request (2560x1440 -> 1280x720): {:?}",
        started.elapsed()
    );
    assert_eq!(first["identity"]["process_id"], game.child.id());
    assert_eq!(
        PathBuf::from(first["identity"]["project_path"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        game._dir.path().canonicalize().unwrap()
    );
    let (red, info) = decode(&first);
    assert_eq!((info.width, info.height), (1280, 720));
    let center =
        (usize::from(info.height) / 2 * usize::from(info.width) + usize::from(info.width) / 2) * 3;
    assert!(
        red[center] > 220 && red[center + 1] < 30 && red[center + 2] < 30,
        "Recognizable red viewport"
    );
    tcp::query_addon(
        &state,
        "execute_action",
        json!({"action":"call_method", "path":"/root/Visual/Color", "method":"hide", "args":[]}),
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    let started = Instant::now();
    let second: serde_json::Value = serde_json::from_str(&capture(2048).await.unwrap()).unwrap();
    eprintln!(
        "Largest viewport request (source 2560x1440 -> 2048x1152): {:?}",
        started.elapsed()
    );
    let (background, info) = decode(&second);
    assert_eq!((info.width, info.height), (2048, 1152));
    let center =
        (usize::from(info.height) / 2 * usize::from(info.width) + usize::from(info.width) / 2) * 3;
    assert!(
        background[center].abs_diff(background[center + 1]) < 10
            && background[center + 1].abs_diff(background[center + 2]) < 10,
        "Hiding red rectangle must reveal neutral background"
    );
    assert_eq!(first["identity"], second["identity"]);
    assert!(second["frames_drawn"].as_u64() > first["frames_drawn"].as_u64());
    assert!(second["readback_physics_frame"].as_u64() >= first["readback_physics_frame"].as_u64());
    let status = tcp::query_addon(&state, "dashcam_status", json!({}))
        .await
        .unwrap();
    assert_eq!(status["state"], "disabled");
}

#[tokio::test]
#[ignore = "requires Godot and built GDExtension"]
async fn headless_pixels_unavailable_but_spatial_observation_works() {
    let (game, state) = start(true).await;
    let cli = Command::new(env!("CARGO_BIN_EXE_stage"))
        .args(["viewport", "{}"])
        .env("THEATRE_PORT", game.port.to_string())
        .env("THEATRE_PROJECT_DIR", game._dir.path())
        .output()
        .unwrap();
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stdout)
    );
    let cli_result: serde_json::Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(cli_result["reason"], "headless");
    assert!(cli_result.get("image_base64").is_none());
    let result = StageServer::new(state.clone())
        .viewport(Parameters(ViewportParams {
            max_dimension: 1280,
        }))
        .await
        .unwrap();
    let metadata = result.structured_content.unwrap();
    assert_eq!(metadata["status"], "unavailable");
    assert_eq!(metadata["reason"], "headless");
    assert_eq!(result.content.len(), 1);
    let spatial = tcp::query_addon(
        &state,
        "get_snapshot_data",
        json!({"perspective":{"type":"point", "position":[0.0,0.0]}, "radius":100.0, "include_offscreen":true, "detail":"standard"}),
    )
    .await
    .unwrap();
    assert!(spatial["entities"].is_array());
}

#[tokio::test]
#[ignore = "requires graphical Godot and built Stage extension"]
async fn saved_clip_survives_shutdown_without_an_agent_storage_path_query() {
    let (mut game, state) = start(false).await;
    tcp::query_addon(
        &state,
        "dashcam_config",
        json!({
            "enabled":true, "capture_interval":1, "screenshot_interval_frames":2,
            "screenshot_max_dimension":320, "anomaly_enabled":false
        }),
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let status = tcp::query_addon(&state, "dashcam_status", json!({}))
                .await
                .unwrap();
            if status["screenshot_buffer_count"].as_u64().unwrap_or(0) >= 4 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    assert!(state.lock().await.clip_storage_path.is_none());
    let saved = tcp::query_addon(
        &state,
        "dashcam_flush",
        json!({"marker_label":"offline handoff"}),
    )
    .await
    .unwrap();
    let clip_id = saved["clip_id"].as_str().unwrap();
    assert!(state.lock().await.clip_storage_path.is_none());
    assert!(game._dir.path().join(".stage/clip_storage_path").is_file());
    game.child.kill().unwrap();
    game.child.wait().unwrap();

    let run = |params: serde_json::Value| {
        let output = Command::new(env!("CARGO_BIN_EXE_stage"))
            .args(["clips", &params.to_string()])
            .env("THEATRE_PROJECT_DIR", game._dir.path())
            .env("THEATRE_PORT", "0")
            .output()
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(output.status.success(), "offline call failed: {value}");
        value
    };
    let list = run(json!({"action":"list"}));
    let clip = list["clips"]
        .as_array()
        .unwrap()
        .iter()
        .find(|clip| clip["clip_id"] == clip_id)
        .unwrap();
    let frame = clip["frame_range"][0].as_u64().unwrap();
    let markers = run(json!({"action":"markers", "clip_id":clip_id}));
    assert!(markers.to_string().contains("offline handoff"));
    run(json!({"action":"snapshot_at", "clip_id":clip_id, "at_frame":frame}));
    let image = run(
        json!({"action":"visual_artifact", "clip_id":clip_id, "artifact":"storyboard", "tile_limit":3}),
    );
    assert!(
        image["image_base64"]
            .as_str()
            .is_some_and(|image| !image.is_empty()),
        "{image}"
    );
    run(json!({"action":"delete", "clip_id":clip_id}));

    for action in ["status", "save", "add_marker", "config"] {
        let output = Command::new(env!("CARGO_BIN_EXE_stage"))
            .args(["clips", &json!({"action":action, "config":{}}).to_string()])
            .env("THEATRE_PROJECT_DIR", game._dir.path())
            .env("THEATRE_PORT", "0")
            .output()
            .unwrap();
        assert!(!output.status.success());
        let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["error"], "connection_failed");
    }
}

#[tokio::test]
#[ignore = "requires Godot and built GDExtension"]
async fn agent_stop_reports_failed_pending_save_without_claiming_rollback() {
    let (_game, state) = start(true).await;
    tcp::query_addon(
        &state,
        "dashcam_config",
        json!({"enabled":true,"screenshot_enabled":false,"anomaly_enabled":false}),
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    tcp::query_addon(
        &state,
        "recording_marker",
        json!({"source":"agent","label":"stop failure"}),
    )
    .await
    .unwrap();
    let storage = tcp::query_addon(&state, "recording_resolve_path", json!({}))
        .await
        .unwrap();
    let path = PathBuf::from(
        storage["path"]
            .as_str()
            .unwrap()
            .trim_end_matches(['/', '\\']),
    );
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "deliberate storage obstruction").unwrap();
    let server = StageServer::new(state.clone());
    let result = server
        .clips(Parameters(
            serde_json::from_value(json!({"action":"config","config":{"enabled":false}})).unwrap(),
        ))
        .await
        .unwrap();
    let result: serde_json::Value =
        serde_json::from_str(&result.content[0].as_text().unwrap().text).unwrap();
    assert_eq!(result["result"], "ok");
    assert_eq!(result["config"]["enabled"], false);
    assert_eq!(result["stop_save"]["result"], "error", "{result}");
    assert!(
        result["stop_save"]["message"]
            .as_str()
            .unwrap()
            .contains("not saved")
    );
    let status = tcp::query_addon(&state, "dashcam_status", json!({}))
        .await
        .unwrap();
    assert_eq!(status["state"], "disabled");
    assert!(status["last_saved_clip"].is_null());
}

#[tokio::test]
#[ignore = "requires Godot and built GDExtension"]
async fn invalid_project_recorder_settings_are_reported_without_blocking_cli_status() {
    for (patch, evidence) in [
        ("[dashcam]\ncapture_interval=0\n", "capture_interval"),
        (
            "[dashcam]\nenabled=true\nmovement_nodes=['MissingBody']\n",
            "Movement target",
        ),
    ] {
        let (game, _state) = start_with_connection(true, false).await;
        std::fs::write(game._dir.path().join("stage.toml"), patch).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_stage"))
            .args(["clips", "{\"action\":\"status\"}"])
            .env("THEATRE_PROJECT_DIR", game._dir.path())
            .env("THEATRE_PORT", game.port.to_string())
            .env("RUST_LOG", "warn")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stderr}");
        assert!(
            stderr.contains("Project dashcam settings were not applied")
                && stderr.contains(evidence),
            "{stderr}"
        );
        let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(status["config"]["enabled"], false);
        assert_ne!(status["config"]["capture_interval"], 0);
    }
}
