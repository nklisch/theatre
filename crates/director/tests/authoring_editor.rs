use director::{
    editor::EditorHandle,
    mcp::editor_run::{EditorRunAction, EditorRunParams},
    oneshot::OperationResult,
    server::DirectorServer,
};
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};
use std::{
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};

struct Fixture {
    project: tempfile::TempDir,
    user_dirs: tempfile::TempDir,
    child: Child,
    driver: EditorHandle,
    server: DirectorServer,
    editor_port: u16,
    stage_port: u16,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn copy(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
fn publish_pending_feedback(project: &Path) {
    let directory = project.join(".theatre/feedback/feedback_editor_run");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("item.json"),
        json!({
            "feedback_id": "feedback_editor_run",
            "source": "editor",
            "timestamp_ms": 1,
            "project_path": project,
            "process_id": 7,
            "scene": "res://a.tscn",
            "surface": "2d",
            "selection": [],
            "pointer": {"status": "unavailable"},
            "capture": {"status": "unavailable", "reason": "test"},
            "readback_render_frame": 0,
            "readback_physics_frame": 0,
            "note": "Inspect this run"
        })
        .to_string(),
    )
    .unwrap();
}

fn port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
impl Fixture {
    async fn start() -> Self {
        Self::start_with_stage(false).await
    }

    async fn start_with_stage(stage: bool) -> Self {
        Self::start_with_plugins(stage, true).await
    }

    async fn start_with_plugins(stage: bool, director: bool) -> Self {
        let started = std::time::Instant::now();
        let project = tempfile::TempDir::new().unwrap();
        copy(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/director"),
            &project.path().join("addons/director"),
        );
        copy(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/theatre_shared"),
            &project.path().join("addons/theatre_shared"),
        );
        if stage {
            copy(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/stage"),
                &project.path().join("addons/stage"),
            );
        }
        let driver_path = project.path().join("addons/driver");
        std::fs::create_dir_all(&driver_path).unwrap();
        std::fs::write(
            driver_path.join("driver.gd"),
            include_str!("fixtures/authoring_driver.gd"),
        )
        .unwrap();
        std::fs::write(driver_path.join("plugin.cfg"), "[plugin]\nname=\"Authoring test driver\"\ndescription=\"Test only\"\nauthor=\"Theatre\"\nversion=\"1\"\nscript=\"driver.gd\"\n").unwrap();
        std::fs::write(project.path().join("exports.gd"), "@tool\nextends Node2D\n@export var speed: float = 3.0\n@export var limited: float = 0.0:\n\tset(value):\n\t\tlimited = minf(value, 5.0)\n").unwrap();
        std::fs::write(
            project.path().join("run_saved.gd"),
            include_str!("fixtures/authoring_run.gd"),
        )
        .unwrap();
        let editor_port = port();
        let driver_port = port();
        let stage_port = port();
        let autoload = if stage {
            "[autoload]\nStageRuntime=\"*res://addons/stage/runtime.gd\"\n"
        } else {
            ""
        };
        let director_plugin = if director {
            "\"res://addons/director/plugin.cfg\","
        } else {
            ""
        };
        std::fs::write(project.path().join("project.godot"), format!("config_version=5\n[application]\nconfig/name=\"AuthoringJourney\"\n{autoload}[rendering]\nrenderer/rendering_method=\"gl_compatibility\"\n[editor_plugins]\nenabled=PackedStringArray({director_plugin}\"res://addons/driver/plugin.cfg\")\n[director]\nconnection/editor_port={editor_port}\n")).unwrap();
        // Native shortcut defaults and run/save settings must not depend on or
        // modify the developer's editor settings, app data, or shader cache.
        let user_dirs = tempfile::TempDir::new().unwrap();
        for directory in ["config", "data", "cache"] {
            std::fs::create_dir_all(user_dirs.path().join(directory)).unwrap();
        }
        let log = project.path().join("editor.log");
        let mut child = Command::new(director::resolve::resolve_godot_bin().unwrap())
            .args(["--editor", "--path"])
            .arg(project.path())
            .env("XDG_CONFIG_HOME", user_dirs.path().join("config"))
            .env("XDG_DATA_HOME", user_dirs.path().join("data"))
            .env("XDG_CACHE_HOME", user_dirs.path().join("cache"))
            .env("DIRECTOR_EDITOR_PORT", editor_port.to_string())
            .env("DIRECTOR_DRIVER_PORT", driver_port.to_string())
            .env("THEATRE_PORT", stage_port.to_string())
            .stdout(std::fs::File::create(&log).unwrap())
            .stderr(Stdio::from(
                std::fs::OpenOptions::new().append(true).open(&log).unwrap(),
            ))
            .spawn()
            .unwrap();
        let driver = tokio::time::timeout(Duration::from_secs(45), async {
            loop {
                if let Ok(handle) =
                    EditorHandle::connect_verified(driver_port, project.path()).await
                {
                    break handle;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "Editor startup failed: {}",
                std::fs::read_to_string(&log).unwrap()
            );
        });
        let mut fixture = Self {
            project,
            user_dirs,
            child,
            driver,
            server: DirectorServer::new(),
            editor_port,
            stage_port,
        };
        eprintln!(
            "fixture connected after {:.2?}; director={director} stage={stage}",
            started.elapsed()
        );
        fixture.wait_for_input_ready().await;
        eprintln!("fixture input ready after {:.2?}", started.elapsed());
        fixture.drive("prepare", json!({})).await;
        eprintln!("fixture prepared after {:.2?}", started.elapsed());
        fixture
    }
    async fn drive(&mut self, operation: &str, params: Value) -> Value {
        let result = self
            .driver
            .send_operation(operation, &params)
            .await
            .unwrap_or_else(|e| panic!("{operation}: {e}: {}", self.log()));
        assert!(result.success, "{operation}: {result:?}: {}", self.log());
        result.data
    }
    fn log(&self) -> String {
        std::fs::read_to_string(self.project.path().join("editor.log")).unwrap()
    }
    async fn raw(&self, operation: &str, params: Value) -> OperationResult {
        self.server
            .backend
            .run_operation(
                &director::resolve::resolve_godot_bin().unwrap(),
                self.project.path(),
                operation,
                &params,
            )
            .await
            .unwrap_or_else(|e| panic!("{operation}: {e}: {}", self.log()))
    }
    async fn ok(&self, operation: &str, params: Value) -> Value {
        let result = self.raw(operation, params).await;
        assert!(result.success, "{operation}: {result:?}: {}", self.log());
        result.data
    }
    async fn inspect(&mut self, node: &str) -> Value {
        self.drive("inspect", json!({"node_path":node})).await
    }
    async fn wait_for_input_ready(&mut self) {
        // The plugin's TCP listener is available during initial import, while
        // Godot's progress UI still consumes every key before shortcut handling.
        let started = std::time::Instant::now();
        let mut readiness = Value::Null;
        let mut polls = 0;
        tokio::time::timeout(Duration::from_secs(90), async {
            loop {
                readiness = self.drive("input_readiness", json!({})).await;
                polls += 1;
                if readiness["scanning"] == false && readiness["input_blocked"] == false {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("Editor not ready for input: {readiness}: {}", self.log()));
        if started.elapsed() >= Duration::from_secs(1) {
            eprintln!(
                "input readiness: {:.2?}, {polls} polls; {readiness}",
                started.elapsed()
            );
        }
    }
    async fn shortcut(&mut self, redo: bool) {
        self.wait_for_input_ready().await;
        self.drive("shortcut", json!({"scene_path":"a.tscn","redo":redo}))
            .await;
    }
    async fn batch(&self, operations: Value, stop: bool) -> Result<Value, rmcp::model::ErrorData> {
        let params = serde_json::from_value(json!({"project_path":self.project.path(),"operations":operations,"stop_on_error":stop})).unwrap();
        self.server
            .batch(Parameters(params))
            .await
            .map(|s| serde_json::from_str(&s).unwrap())
    }
    async fn editor_run(&self, action: EditorRunAction, scene_path: Option<&str>) -> Value {
        let result = self
            .server
            .editor_run(Parameters(EditorRunParams {
                project_path: self.project.path().to_string_lossy().into_owned(),
                action,
                scene_path: scene_path.map(str::to_owned),
            }))
            .await
            .unwrap_or_else(|e| panic!("{}: {}", e.message, self.log()));
        serde_json::from_str(&result).unwrap()
    }
}

async fn stage_status(project: &Path, port: u16) -> Value {
    let stage = Path::new(env!("CARGO_BIN_EXE_director"))
        .with_file_name(format!("stage{}", std::env::consts::EXE_SUFFIX));
    let output = tokio::process::Command::new(stage)
        .args(["runtime_status", "{}"])
        .env("THEATRE_PORT", port.to_string())
        .env("THEATRE_PROJECT_DIR", project)
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "stage runtime_status failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

async fn wait_for_stage(project: &Path, port: u16) -> Value {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let status = stage_status(project, port).await;
            if status["ready"] == true {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Stage did not report a ready current scene")
}

#[tokio::test]
#[ignore = "requires graphical Godot; Director-free comparison for the native AccessKit crash"]
async fn native_only_undo_comparison() {
    let mut f = Fixture::start_with_plugins(false, false).await;
    let started = std::time::Instant::now();
    let human = f.inspect("Human").await;
    for operation in [
        "node_add",
        "scene_add_instance",
        "node_remove",
        "node_reparent",
        "node_set_properties",
    ] {
        // Match the original journey's individual and batch repetitions, but
        // build every action directly with EditorUndoRedoManager, without Director.
        for repetition in 0..2 {
            eprintln!(
                "native {operation} repetition={repetition} at {:.2?}",
                started.elapsed()
            );
            f.drive("native_edit", json!({"operation":operation})).await;
            let changed_node = match operation {
                "node_add" | "scene_add_instance" => "Parent/Added",
                "node_reparent" => "Parent/Moved",
                _ => "Human",
            };
            let changed = f.inspect(changed_node).await;
            assert_eq!(changed["exists"], operation != "node_remove");
            if operation == "node_set_properties" {
                assert_eq!(changed["position"], json!([42.0, 2.0]));
            }
            for redo in [false, true, false] {
                eprintln!(
                    "native {operation} repetition={repetition} redo={redo} at {:.2?}",
                    started.elapsed()
                );
                f.wait_for_input_ready().await;
                // Reproduce the no-control-focus path that actually crashed,
                // rather than comparing it against the newer focused variant.
                f.drive(
                    "shortcut",
                    json!({"scene_path":"a.tscn","redo":redo,"focus_scene":false}),
                )
                .await;
                if redo {
                    let restored = f.inspect(changed_node).await;
                    assert_eq!(restored["exists"], changed["exists"]);
                    assert_eq!(restored["instance_id"], changed["instance_id"]);
                    assert_eq!(restored["position"], changed["position"]);
                } else {
                    let restored = f.inspect("Human").await;
                    for field in ["instance_id", "owner_id", "index", "position"] {
                        assert_eq!(restored[field], human[field], "native {operation}: {field}");
                    }
                    if matches!(
                        operation,
                        "node_add" | "scene_add_instance" | "node_reparent"
                    ) {
                        assert_eq!(f.inspect(changed_node).await["exists"], false);
                    }
                }
            }
        }
    }
    assert!(!f.log().contains("[Director] Editor plugin listening"));
    assert!(!f.log().contains("SCRIPT ERROR"), "{}", f.log());
}

#[tokio::test]
#[ignore = "requires graphical Godot 4.7 editor and deployed Stage GDExtension"]
async fn editor_run_controls_saved_scene_without_saving_human_work() {
    let mut f = Fixture::start_with_stage(true).await;
    let saved = std::fs::read(f.project.path().join("a.tscn")).unwrap();
    let before = f.inspect("Human").await;
    assert_eq!(before["position"], json!([10.0, 0.0]));
    assert!(
        before["dirty"]
            .as_array()
            .unwrap()
            .contains(&json!("res://a.tscn"))
    );

    let missing = f
        .raw(
            "editor_run",
            json!({"action":"start","scene_path":"missing.tscn"}),
        )
        .await;
    assert!(!missing.success);
    assert!(missing.error.unwrap().contains("Saved scene not found"));

    let started = f.editor_run(EditorRunAction::Start, Some("a.tscn")).await;
    assert_eq!(started["action"], "start");
    assert_eq!(started["scene_path"], "a.tscn");
    assert_eq!(started["launch_requested"], true);
    assert_eq!(started["game_running"], true);
    assert_eq!(started["playing_scene"], "a.tscn");
    let playing_status = f.editor_run(EditorRunAction::Status, None).await;
    assert_eq!(playing_status["launch_requested"], false);
    assert_eq!(playing_status["game_running"], true);
    assert_eq!(playing_status["scene_path"], "a.tscn");
    assert_eq!(playing_status["playing_scene"], "a.tscn");
    assert_eq!(
        f.inspect("Human").await["save_before_running"],
        before["save_before_running"]
    );
    assert_eq!(
        std::fs::read(f.project.path().join("a.tscn")).unwrap(),
        saved
    );
    assert_eq!(f.inspect("Human").await["position"], json!([10.0, 0.0]));

    // Director reports the native launch request, while Stage independently
    // distinguishes an unavailable connection from a ready runtime.
    let unavailable_port = port();
    let not_ready = stage_status(f.project.path(), unavailable_port).await;
    assert_eq!(not_ready["connected"], false);
    assert_eq!(not_ready["ready"], false);
    let first_runtime = wait_for_stage(f.project.path(), f.stage_port).await;
    assert_eq!(first_runtime["ready"], true);
    assert_eq!(first_runtime["current_scene"], "res://a.tscn");
    let first_run_id = first_runtime["identity"]["run_id"].clone();

    let duplicate_start = f
        .raw(
            "editor_run",
            json!({"action":"start","scene_path":"a.tscn"}),
        )
        .await;
    assert!(!duplicate_start.success);
    assert!(duplicate_start.error.unwrap().contains("use restart"));

    let restarted = f.editor_run(EditorRunAction::Restart, Some("a.tscn")).await;
    assert_eq!(restarted["launch_requested"], true);
    assert_eq!(restarted["game_running"], true);
    let second_runtime = wait_for_stage(f.project.path(), f.stage_port).await;
    assert_ne!(second_runtime["identity"]["run_id"], first_run_id);
    assert_eq!(second_runtime["current_scene"], "res://a.tscn");
    assert_eq!(
        std::fs::read(f.project.path().join("a.tscn")).unwrap(),
        saved
    );
    assert_eq!(f.inspect("Human").await["position"], json!([10.0, 0.0]));

    let stopped = f.editor_run(EditorRunAction::Stop, None).await;
    assert_eq!(stopped["launch_requested"], false);
    assert_eq!(stopped["game_running"], false);
    assert_eq!(stopped["playing_scene"], "");
    let stopped_again = f.editor_run(EditorRunAction::Stop, None).await;
    assert_eq!(stopped_again["game_running"], false);
    assert_eq!(stopped_again["launch_requested"], false);
    let stopped_status = f.editor_run(EditorRunAction::Status, None).await;
    assert_eq!(stopped_status["game_running"], false);
    assert_eq!(stopped_status["scene_path"], "");

    let wrong = tempfile::TempDir::new().unwrap();
    std::fs::write(
        wrong.path().join("project.godot"),
        format!(
            "config_version=5\n[director]\nconnection/editor_port={}\n",
            f.editor_port
        ),
    )
    .unwrap();
    let wrong_project = f
        .server
        .backend
        .run_editor_operation(wrong.path(), "editor_run", &json!({"action":"status"}))
        .await
        .unwrap_err();
    assert!(wrong_project.to_string().contains("project identity"));

    // A one-shot Director CLI invocation uses the same typed editor-only path.
    publish_pending_feedback(f.project.path());
    let cli = Command::new(env!("CARGO_BIN_EXE_director"))
        .arg("editor_run")
        .arg(json!({"project_path":f.project.path(),"action":"status"}).to_string())
        .output()
        .unwrap();
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let cli_status: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(cli_status["data"]["game_running"], false);
    assert_eq!(cli_status["data"]["playing_scene"], "");
    assert!(
        cli_status["feedback_notice"]
            .as_str()
            .unwrap()
            .contains("1 pending")
    );
}

#[tokio::test]
#[ignore = "requires graphical Godot 4.7 editor"]
async fn authoring_preserves_human_history_and_selected_scene_persistence() {
    let mut f = Fixture::start().await;
    let original_a = std::fs::read(f.project.path().join("a.tscn")).unwrap();
    let original_b = std::fs::read(f.project.path().join("b.tscn")).unwrap();
    let human = f.inspect("Human").await;
    assert_eq!(human["position"], json!([10.0, 0.0]));
    assert_eq!(human["active_scene"], "res://b.tscn");
    assert_eq!(human["dirty"].as_array().unwrap().len(), 2);
    let edited = f.ok("node_set_properties", json!({"scene_path":"a.tscn","node_path":"Human","properties":{"position":{"x":30,"y":4}}})).await;
    assert_eq!(
        edited["persistence"]["unsaved_scene_paths"],
        json!(["a.tscn"])
    );
    assert_eq!(f.inspect("Human").await["active_scene"], "res://b.tscn");
    f.shortcut(false).await;
    assert_eq!(f.inspect("Human").await["position"], json!([10.0, 0.0]));
    f.shortcut(true).await;
    assert_eq!(f.inspect("Human").await["position"], json!([30.0, 4.0]));
    assert_eq!(
        f.inspect("Human").await["instance_id"],
        human["instance_id"]
    );
    f.shortcut(false).await;
    f.shortcut(false).await; // the preceding genuine human action still refers to the same node
    assert_eq!(f.inspect("Human").await["position"], json!([0.0, 0.0]));
    f.shortcut(true).await;
    f.shortcut(true).await;
    assert_eq!(
        std::fs::read(f.project.path().join("a.tscn")).unwrap(),
        original_a
    );

    // Whole property collections are validated before any setter executes.
    let rejected = f.raw("node_set_properties", json!({"scene_path":"a.tscn","node_path":"Human","properties":{"position":{"x":99},"not_a_property":1}})).await;
    assert!(!rejected.success);
    assert!(rejected.persistence.unsaved_scene_paths.is_empty());
    assert_eq!(f.inspect("Human").await["position"], json!([30.0, 4.0]));

    f.ok(
        "node_set_script",
        json!({"scene_path":"a.tscn","node_path":"Human","script_path":"exports.gd"}),
    )
    .await;
    // A custom setter partially applies. Its actual changed value remains undoable and reported.
    let failed = f.batch(json!([
        {"operation":"node_set_meta","params":{"scene_path":"a.tscn","node_path":"Human","meta":{"prior":true}}},
        {"operation":"node_set_properties","params":{"scene_path":"a.tscn","node_path":"Human","properties":{"limited":10}}},
        {"operation":"node_set_meta","params":{"scene_path":"a.tscn","node_path":"Human","meta":{"later":true}}}
    ]), true).await.unwrap_err();
    let error = failed.data.unwrap();
    assert_eq!(error["data"]["completed"], 1);
    assert_eq!(error["data"]["failed"], 1);
    assert_eq!(error["data"]["results"].as_array().unwrap().len(), 2);
    assert_eq!(
        error["data"]["results"][1]["persistence"]["unsaved_scene_paths"],
        json!(["a.tscn"])
    );
    assert_eq!(f.inspect("Human").await["limited"], 5.0);
    f.shortcut(false).await;
    assert_eq!(f.inspect("Human").await["limited"], 0.0);
    let continued = f.batch(json!([
        {"operation":"node_set_properties","params":{"scene_path":"a.tscn","node_path":"Human","properties":{"limited":10}}},
        {"operation":"node_set_meta","params":{"scene_path":"a.tscn","node_path":"Human","meta":{"later":true}}}
    ]), false).await.unwrap_err();
    assert_eq!(continued.data.unwrap()["data"]["completed"], 1);
    assert_eq!(f.inspect("Human").await["meta"]["later"], true);

    f.drive("unowned_child", json!({})).await;
    assert_eq!(f.inspect("Human/Unowned").await["owner_id"], 0);
    f.drive("dirty_resource", json!({})).await;
    let material = f.inspect("Mesh").await;
    assert_eq!(material["material_path"], "res://external.tres");
    assert_eq!(material["material_red"], true);
    let external = std::fs::read(f.project.path().join("external.tres")).unwrap();
    let save_params =
        serde_json::from_value(json!({"project_path":f.project.path(),"scene_path":"a.tscn"}))
            .unwrap();
    let saved: Value =
        serde_json::from_str(&f.server.scene_save(Parameters(save_params)).await.unwrap()).unwrap();
    assert_eq!(saved["persistence"]["saved_paths"], json!(["a.tscn"]));
    assert_eq!(saved["editor_dirty_marker_may_remain"], true);
    assert_eq!(f.inspect("Human/Unowned").await["owner_id"], 0);
    assert_ne!(
        std::fs::read(f.project.path().join("a.tscn")).unwrap(),
        original_a
    );
    assert_eq!(
        std::fs::read(f.project.path().join("b.tscn")).unwrap(),
        original_b
    );
    assert_eq!(
        std::fs::read(f.project.path().join("external.tres")).unwrap(),
        external
    );
    f.shortcut(false).await; // explicit serialization retains native history
    assert!(f.inspect("Human").await["meta"].get("later").is_none());
    f.shortcut(true).await;
    f.drive("reopen", json!({"scene_path":"a.tscn"})).await;
    assert_eq!(f.inspect("Human").await["position"], json!([30.0, 4.0]));
    assert_eq!(f.inspect("Human").await["meta"]["later"], true);
    let run = Command::new(director::resolve::resolve_godot_bin().unwrap())
        .args(["--headless", "--path"])
        .arg(f.project.path())
        .args(["--script", "run_saved.gd"])
        .env("XDG_CONFIG_HOME", f.user_dirs.path().join("config"))
        .env("XDG_DATA_HOME", f.user_dirs.path().join("data"))
        .env("XDG_CACHE_HOME", f.user_dirs.path().join("cache"))
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(String::from_utf8_lossy(&run.stdout).contains("\"saved_scene_ran\":true"));
    assert_eq!(
        f.drive(
            "inspect",
            json!({"scene_path":"b.tscn","node_path":"Human"})
        )
        .await["position"],
        json!([20.0, 0.0])
    );
    f.server.backend.shutdown().await;
    let cli = Command::new(env!("CARGO_BIN_EXE_director"))
        .arg("scene_save")
        .arg(json!({"project_path":f.project.path(),"scene_path":"a.tscn"}).to_string())
        .output()
        .unwrap();
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let cli_result: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(cli_result["persistence"]["saved_paths"], json!(["a.tscn"]));
    assert_eq!(
        std::fs::read(f.project.path().join("external.tres")).unwrap(),
        external
    );
    f.drive("break_save", json!({})).await;
    let failure = f.raw("scene_save", json!({"scene_path":"a.tscn"})).await;
    assert!(!failure.success);
    assert!(failure.persistence.saved_paths.is_empty());
    assert!(!f.log().contains("SCRIPT ERROR"), "{}", f.log());
}

#[tokio::test]
#[ignore = "requires graphical Godot 4.7 editor"]
async fn every_scene_mutator_uses_native_undo_in_single_and_batch_routes() {
    let mut f = Fixture::start().await;
    let original = std::fs::read(f.project.path().join("a.tscn")).unwrap();
    // Test each operation both directly and as an entry, undoing between routes.
    let operations = [
        (
            "node_add",
            json!({"parent_path":"Parent","node_type":"Node2D","node_name":"Added"}),
            "Parent/Added",
        ),
        (
            "scene_add_instance",
            json!({"parent_path":"Parent","instance_scene":"instance.tscn","node_name":"Added"}),
            "Parent/Added",
        ),
        ("node_remove", json!({"node_path":"Human"}), "Human"),
        (
            "node_reparent",
            json!({"node_path":"Human","new_parent_path":"Parent","new_name":"Moved"}),
            "Parent/Moved",
        ),
        (
            "node_set_properties",
            json!({"node_path":"Human","properties":{"position":{"x":42,"y":2}}}),
            "Human",
        ),
        (
            "node_set_groups",
            json!({"node_path":"Human","add":["agents"]}),
            "Human",
        ),
        (
            "node_set_meta",
            json!({"node_path":"Human","meta":{"purpose":"test"}}),
            "Human",
        ),
        (
            "node_set_script",
            json!({"node_path":"Human","script_path":"exports.gd"}),
            "Human",
        ),
        (
            "physics_set_layers",
            json!({"node_path":"Body","collision_layer":7}),
            "Body",
        ),
        (
            "shape_create",
            json!({"node_path":"Body/Shape","shape_type":"CircleShape2D","shape_params":{"radius":5}}),
            "Body/Shape",
        ),
        (
            "signal_connect",
            json!({"source_path":"Button","signal_name":"pressed","target_path":"Button","method_name":"hide"}),
            "Button",
        ),
        (
            "tilemap_set_cells",
            json!({"node_path":"Tiles","cells":[{"coords":[2,3],"source_id":0,"atlas_coords":[0,0],"alternative_tile":4096}]}),
            "Tiles",
        ),
        ("tilemap_clear", json!({"node_path":"Tiles"}), "Tiles"),
        (
            "gridmap_set_cells",
            json!({"node_path":"Grid","cells":[{"position":[1,2,3],"item":0,"orientation":12}]}),
            "Grid",
        ),
        ("gridmap_clear", json!({"node_path":"Grid"}), "Grid"),
    ];
    let mutation_started = std::time::Instant::now();
    for (operation, params, node) in operations {
        for batch in [false, true] {
            eprintln!(
                "mutator {operation} batch={batch} at {:.2?}",
                mutation_started.elapsed()
            );
            let human_before = f.inspect("Human").await;
            let mut args = params.clone();
            args["scene_path"] = json!("a.tscn");
            let before = f.ok("scene_read", json!({"scene_path":"a.tscn"})).await;
            let read_op = if operation.starts_with("tilemap_") {
                Some("tilemap_get_cells")
            } else if operation.starts_with("gridmap_") {
                Some("gridmap_get_cells")
            } else if operation.starts_with("signal_") {
                Some("signal_list")
            } else {
                None
            };
            let read_args = json!({"scene_path":"a.tscn", "node_path":node});
            let state_before = if let Some(op) = read_op {
                f.ok(op, read_args.clone()).await
            } else {
                Value::Null
            };
            let result = if batch {
                f.batch(json!([{"operation":operation,"params":args}]), true)
                    .await
                    .unwrap()
            } else {
                f.ok(operation, args).await
            };
            assert_eq!(
                result["persistence"]["unsaved_scene_paths"],
                json!(["a.tscn"]),
                "{operation} batch={batch}: {}",
                f.log()
            );
            let after = f.inspect(node).await;
            let instance_leaf = if operation == "scene_add_instance" {
                let leaf = f.inspect("Parent/Added/Leaf").await;
                assert_eq!(leaf["owner_id"], after["instance_id"]);
                Some(leaf)
            } else {
                None
            };
            let state_after = if let Some(op) = read_op {
                f.ok(op, read_args.clone()).await
            } else {
                Value::Null
            };
            if read_op.is_some() {
                assert_ne!(state_after, state_before, "{operation} changes state");
            }
            eprintln!("mutator {operation} batch={batch}: undo");
            f.shortcut(false).await;
            let human_after = f.inspect("Human").await;
            for field in ["instance_id", "owner_id", "index", "position"] {
                assert_eq!(
                    human_after[field], human_before[field],
                    "{operation} {field}"
                );
            }
            assert_eq!(
                f.ok("scene_read", json!({"scene_path":"a.tscn"})).await,
                before,
                "{operation} undo"
            );
            if let Some(op) = read_op {
                assert_eq!(
                    f.ok(op, read_args.clone()).await,
                    state_before,
                    "{operation} undo state"
                );
            }
            eprintln!("mutator {operation} batch={batch}: redo");
            f.shortcut(true).await;
            if let Some(op) = read_op {
                assert_eq!(
                    f.ok(op, read_args.clone()).await,
                    state_after,
                    "{operation} redo state"
                );
            }
            assert_eq!(
                f.inspect(node).await["instance_id"],
                after["instance_id"],
                "{operation} redo identity"
            );
            if let Some(leaf) = instance_leaf {
                let restored = f.inspect("Parent/Added/Leaf").await;
                assert_eq!(restored["instance_id"], leaf["instance_id"]);
                assert_eq!(restored["owner_id"], leaf["owner_id"]);
            }
            f.shortcut(false).await;
            assert_eq!(
                std::fs::read(f.project.path().join("a.tscn")).unwrap(),
                original
            );
        }
    }
    for batch in [false, true] {
        let body = f.inspect("Body").await;
        let shape = f.inspect("Body/Shape").await;
        let args = json!({"scene_path":"a.tscn","node_path":"Body"});
        let removed = if batch {
            f.batch(json!([{"operation":"node_remove","params":args}]), true)
                .await
                .unwrap()["results"][0]["data"]
                .clone()
        } else {
            f.ok("node_remove", args).await
        };
        assert_eq!(removed["children_removed"], 1);
        f.shortcut(false).await;
        let restored_body = f.inspect("Body").await;
        let restored_shape = f.inspect("Body/Shape").await;
        for field in ["instance_id", "owner_id", "index"] {
            assert_eq!(restored_body[field], body[field]);
            assert_eq!(restored_shape[field], shape[field]);
        }
        f.shortcut(true).await;
        assert_eq!(f.inspect("Body").await["exists"], false);
        f.shortcut(false).await;
    }
    // Detaching a script restores both the script object and stored exports on undo.
    f.ok(
        "node_set_script",
        json!({"scene_path":"a.tscn","node_path":"Human","script_path":"exports.gd"}),
    )
    .await;
    f.ok(
        "node_set_properties",
        json!({"scene_path":"a.tscn","node_path":"Human","properties":{"speed":17}}),
    )
    .await;
    for batch in [false, true] {
        let args = json!({"scene_path":"a.tscn","node_path":"Human"});
        if batch {
            f.batch(json!([{"operation":"node_set_script","params":args}]), true)
                .await
                .unwrap();
        } else {
            f.ok("node_set_script", args).await;
        }
        assert!(f.inspect("Human").await.get("script").is_none());
        f.shortcut(false).await;
        assert_eq!(f.inspect("Human").await["speed"], 17.0);
        f.shortcut(true).await;
        f.shortcut(false).await;
    }
    // Bound persistent connections retain their exact binds and deferred flag.
    f.ok("signal_connect", json!({"scene_path":"a.tscn","source_path":"Button","signal_name":"pressed","target_path":"Human","method_name":"set_meta","binds":["pressed",true],"flags":1})).await;
    let connections = f.ok("signal_list", json!({"scene_path":"a.tscn"})).await;
    for batch in [false, true] {
        let args = json!({"scene_path":"a.tscn","source_path":"Button","signal_name":"pressed","target_path":"Human","method_name":"set_meta"});
        if batch {
            f.batch(
                json!([{"operation":"signal_disconnect","params":args}]),
                true,
            )
            .await
            .unwrap();
        } else {
            f.ok("signal_disconnect", args).await;
        }
        assert!(
            f.ok("signal_list", json!({"scene_path":"a.tscn"})).await["connections"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        f.shortcut(false).await;
        assert_eq!(
            f.ok("signal_list", json!({"scene_path":"a.tscn"})).await,
            connections
        );
    }
    // An invalid later cell must not apply an earlier one.
    for (op, node, cells, read) in [
        (
            "tilemap_set_cells",
            "Tiles",
            json!([{"coords":[4,4],"source_id":0,"atlas_coords":[0,0]}, {"coords":[3]}]),
            "tilemap_get_cells",
        ),
        (
            "gridmap_set_cells",
            "Grid",
            json!([{"position":[4,4,4],"item":0}, {"position":[3]}]),
            "gridmap_get_cells",
        ),
    ] {
        let before = f
            .ok(read, json!({"scene_path":"a.tscn","node_path":node}))
            .await;
        let failure = f
            .raw(
                op,
                json!({"scene_path":"a.tscn","node_path":node,"cells":cells}),
            )
            .await;
        assert!(!failure.success);
        assert!(failure.persistence.unsaved_scene_paths.is_empty());
        assert_eq!(
            f.ok(read, json!({"scene_path":"a.tscn","node_path":node}))
                .await,
            before
        );
    }
    f.drive("ephemeral_group", json!({})).await;
    f.ok(
        "node_set_groups",
        json!({"scene_path":"a.tscn","node_path":"Human","remove":["ephemeral"]}),
    )
    .await;
    f.shortcut(false).await;
    let groups = f.inspect("Human").await;
    assert!(
        groups["groups"]
            .as_array()
            .unwrap()
            .contains(&json!("ephemeral"))
    );
    assert!(
        !groups["persistent_groups"]
            .as_array()
            .unwrap()
            .contains(&json!("ephemeral"))
    );

    // Validation must not poison a retained editor resource that a later edit saves.
    f.ok(
        "animation_create",
        json!({"resource_path":"animation.tres","length":1}),
    )
    .await;
    f.drive("hold_animation", json!({})).await;
    let mut track = json!({"resource_path":"animation.tres","track_type":"value","node_path":"Human:position","keyframes":[{"time":0,"value":1}],"interpolation":"invalid"});
    assert!(!f.raw("animation_add_track", track.clone()).await.success);
    assert_eq!(f.inspect("Human").await["cached_tracks"], 0);
    track["interpolation"] = json!("linear");
    let written = f.ok("animation_add_track", track).await;
    assert_eq!(
        written["persistence"]["saved_paths"],
        json!(["animation.tres"])
    );
    assert_eq!(
        f.ok("animation_read", json!({"resource_path":"animation.tres"}))
            .await["tracks"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    f.ok(
        "scene_create",
        json!({"scene_path":"closed.tscn","root_type":"Node2D"}),
    )
    .await;
    f.ok(
        "node_set_script",
        json!({"scene_path":"closed.tscn","node_path":".","script_path":"exports.gd"}),
    )
    .await;
    let partial = f
        .raw(
            "node_set_properties",
            json!({"scene_path":"closed.tscn","node_path":".","properties":{"limited":10}}),
        )
        .await;
    assert!(!partial.success);
    assert_eq!(partial.persistence.saved_paths, vec!["closed.tscn"]);
    assert_eq!(
        f.ok("scene_read", json!({"scene_path":"closed.tscn"}))
            .await["root"]["properties"]["limited"],
        5.0
    );

    // Path spelling cannot bypass the selected-scene boundary or root protection.
    for operation in ["node_remove", "node_reparent"] {
        let rejected = f
            .raw(
                operation,
                json!({"scene_path":"./a.tscn","node_path":"Human/..","new_parent_path":"Parent"}),
            )
            .await;
        assert!(!rejected.success);
        assert_eq!(f.inspect(".").await["exists"], true);
    }
    assert!(
        !f.raw(
            "scene_create",
            json!({"scene_path":"./a.tscn","root_type":"Node"})
        )
        .await
        .success
    );
    let normalized = f
        .ok(
            "node_set_meta",
            json!({"scene_path":"./a.tscn","node_path":"Human","meta":{"normalized":true}}),
        )
        .await;
    assert_eq!(
        normalized["persistence"]["unsaved_scene_paths"],
        json!(["a.tscn"])
    );
    f.shortcut(false).await;

    let bad_shape = f.raw("shape_create", json!({"scene_path":"a.tscn","node_path":"Human","save_path":"bad.tres","shape_type":"CircleShape2D"})).await;
    assert!(!bad_shape.success);
    assert!(!f.project.path().join("bad.tres").exists());
    assert!(!f.log().contains("SCRIPT ERROR"), "{}", f.log());
    let replacement = f
        .raw(
            "scene_create",
            json!({"scene_path":"a.tscn","root_type":"Node"}),
        )
        .await;
    assert!(!replacement.success);
    assert_eq!(
        std::fs::read(f.project.path().join("a.tscn")).unwrap(),
        original
    );
}
