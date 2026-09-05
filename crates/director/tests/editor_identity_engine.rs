use director::editor::{EditorError, EditorHandle};
use serde_json::json;
use std::{
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};

struct EditorProcess(Child);
impl Drop for EditorProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn copy_addon(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_addon(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[tokio::test]
#[ignore = "requires Godot editor"]
async fn real_editor_ping_and_status_identify_actual_project_and_process() {
    let project = tempfile::TempDir::new().unwrap();
    let wrong = tempfile::TempDir::new().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/director");
    copy_addon(&source, &project.path().join("addons/director"));
    copy_addon(
        &source.with_file_name("theatre_shared"),
        &project.path().join("addons/theatre_shared"),
    );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    std::fs::write(project.path().join("project.godot"), format!(
        "config_version=5\n[application]\nconfig/name=\"IdentityJourney\"\n[editor_plugins]\nenabled=PackedStringArray(\"res://addons/director/plugin.cfg\")\n[director]\nconnection/editor_port={port}\n"
    )).unwrap();
    let stderr_path = project.path().join("editor.stderr");
    let child = Command::new(std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into()))
        .args(["--headless", "--editor", "--path"])
        .arg(project.path())
        .env("DIRECTOR_EDITOR_PORT", port.to_string())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&stderr_path).unwrap())
        .spawn()
        .unwrap();
    let editor = EditorProcess(child);
    let mut handle = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            match EditorHandle::connect_verified(port, project.path()).await {
                Ok(handle) => break handle,
                Err(EditorError::NotReachable(_)) => {
                    tokio::time::sleep(Duration::from_millis(100)).await
                }
                Err(error) => panic!(
                    "{error}\n{}",
                    std::fs::read_to_string(&stderr_path).unwrap()
                ),
            }
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "Editor did not become ready: {}",
            std::fs::read_to_string(&stderr_path).unwrap()
        )
    });
    let ping = handle.send_operation("ping", &json!({})).await.unwrap();
    let status = handle
        .send_operation("editor_status", &json!({}))
        .await
        .unwrap();
    assert!(status.success, "{status:?}");
    assert_eq!(status.data["editor_connected"], true);
    assert_eq!(status.data["process_id"], editor.0.id());
    assert_eq!(status.data["project_path"], ping.data["project_path"]);
    assert_eq!(status.data["process_id"], ping.data["process_id"]);
    assert_eq!(
        Path::new(status.data["project_path"].as_str().unwrap())
            .canonicalize()
            .unwrap(),
        project.path().canonicalize().unwrap()
    );
    drop(handle);
    assert!(matches!(
        EditorHandle::connect_verified(port, wrong.path()).await,
        Err(EditorError::Identity(_))
    ));
    let editor_process_id = editor.0.id();
    drop(editor);
    let headless = director::oneshot::run_oneshot(
        &director::resolve::resolve_godot_bin().unwrap(),
        project.path(),
        "editor_status",
        &json!({}),
    )
    .await
    .unwrap();
    assert!(headless.success, "{headless:?}");
    assert_eq!(headless.data["editor_connected"], false);
    assert_eq!(headless.data["project_path"], ping.data["project_path"]);
    assert!(headless.data["process_id"].as_u64().unwrap() > 0);
    assert_ne!(headless.data["process_id"], editor_process_id);
}
