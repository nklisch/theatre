use super::*;
use stage_protocol::codec::async_io;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

struct Peer {
    port: u16,
    operations: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<()>,
}

impl Peer {
    async fn start(project: &Path, disconnect_on_mutation: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let operations = Arc::new(Mutex::new(Vec::new()));
        let seen = operations.clone();
        let project = project.to_path_buf();
        let task = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                while let Ok(request) =
                    async_io::read_message::<serde_json::Value>(&mut stream).await
                {
                    let operation = request["operation"].as_str().unwrap();
                    seen.lock().await.push(operation.into());
                    if disconnect_on_mutation
                        && matches!(operation, "node_set_properties" | "editor_run")
                    {
                        break;
                    }
                    let data = if operation == "ping" {
                        serde_json::json!({"project_path": project, "process_id": 42})
                    } else {
                        serde_json::json!({"applied": true})
                    };
                    if async_io::write_message(
                        &mut stream,
                        &serde_json::json!({"success": true, "data": data}),
                    )
                    .await
                    .is_err()
                    {
                        break;
                    }
                }
            }
        });
        Self {
            port,
            operations,
            task,
        }
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn select(project: &Path, port: u16) {
    std::fs::write(
        project.join("project.godot"),
        format!("[director]\nconnection/editor_port={port}\n"),
    )
    .unwrap();
}

#[tokio::test]
async fn fresh_wrong_project_receives_only_identity_ping() {
    let requested = TempDir::new().unwrap();
    let actual = TempDir::new().unwrap();
    let peer = Peer::start(actual.path(), false).await;
    select(requested.path(), peer.port);
    let backend = Backend::new();
    let error = backend
        .try_editor(
            requested.path(),
            "node_set_properties",
            &serde_json::json!({}),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, EditorError::Identity(_)));
    assert!(
        error
            .to_string()
            .contains(&actual.path().display().to_string())
    );
    assert_eq!(*peer.operations.lock().await, ["ping"]);
}

#[tokio::test]
async fn cached_connection_cannot_mutate_a_different_requested_project() {
    let first = TempDir::new().unwrap();
    let second = TempDir::new().unwrap();
    let peer = Peer::start(first.path(), false).await;
    select(first.path(), peer.port);
    select(second.path(), peer.port);
    let backend = Backend::new();
    backend
        .try_editor(first.path(), "editor_status", &serde_json::json!({}))
        .await
        .unwrap();
    let error = backend
        .try_editor(second.path(), "node_set_properties", &serde_json::json!({}))
        .await
        .unwrap_err();
    assert!(matches!(error, EditorError::Identity(_)));
    assert_eq!(
        *peer.operations.lock().await,
        ["ping", "editor_status", "ping"]
    );
}

#[tokio::test]
async fn changed_port_selects_a_new_verified_connection() {
    let project = TempDir::new().unwrap();
    let first = Peer::start(project.path(), false).await;
    let second = Peer::start(project.path(), false).await;
    select(project.path(), first.port);
    let backend = Backend::new();
    backend
        .try_editor(project.path(), "editor_status", &serde_json::json!({}))
        .await
        .unwrap();
    select(project.path(), second.port);
    backend
        .try_editor(
            project.path(),
            "node_set_properties",
            &serde_json::json!({}),
        )
        .await
        .unwrap();
    assert_eq!(*first.operations.lock().await, ["ping", "editor_status"]);
    assert_eq!(
        *second.operations.lock().await,
        ["ping", "node_set_properties"]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_alias_reuses_verified_project_connection() {
    let project = TempDir::new().unwrap();
    let aliases = TempDir::new().unwrap();
    let alias = aliases.path().join("alias");
    std::os::unix::fs::symlink(project.path(), &alias).unwrap();
    let peer = Peer::start(project.path(), false).await;
    select(project.path(), peer.port);
    let backend = Backend::new();
    backend
        .try_editor(&alias, "editor_status", &serde_json::json!({}))
        .await
        .unwrap();
    backend
        .try_editor(
            project.path(),
            "node_set_properties",
            &serde_json::json!({}),
        )
        .await
        .unwrap();
    assert_eq!(
        *peer.operations.lock().await,
        ["ping", "editor_status", "node_set_properties"]
    );
}

async fn assert_unknown_outcome(cached: bool) {
    let project = TempDir::new().unwrap();
    let peer = Peer::start(project.path(), true).await;
    select(project.path(), peer.port);
    let backend = Backend::new();
    if cached {
        backend
            .try_editor(project.path(), "editor_status", &serde_json::json!({}))
            .await
            .unwrap();
    }
    // A fallback would fail to launch this executable and produce a different error.
    let error = backend
        .run_operation(
            Path::new("/no-godot-must-be-launched"),
            project.path(),
            "node_set_properties",
            &serde_json::json!({}),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("unknown outcome"), "{error}");
    let expected = if cached {
        vec!["ping", "editor_status", "node_set_properties"]
    } else {
        vec!["ping", "node_set_properties"]
    };
    assert_eq!(
        *peer.operations.lock().await,
        expected,
        "no reconnect or replay"
    );
}

#[tokio::test]
async fn editor_only_operation_requires_a_reachable_editor() {
    let project = TempDir::new().unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    select(project.path(), port);
    let error = Backend::new()
        .run_editor_operation(
            project.path(),
            "editor_run",
            &serde_json::json!({"action":"status"}),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("requires the Godot editor"));
    assert!(
        error
            .to_string()
            .contains("no headless backend was started")
    );
}

#[tokio::test]
async fn editor_run_postdispatch_disconnect_is_not_replayed() {
    let project = TempDir::new().unwrap();
    let peer = Peer::start(project.path(), true).await;
    select(project.path(), peer.port);
    let error = Backend::new()
        .run_editor_operation(
            project.path(),
            "editor_run",
            &serde_json::json!({"action":"start","scene_path":"main.tscn"}),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("unknown outcome"), "{error}");
    assert_eq!(*peer.operations.lock().await, ["ping", "editor_run"]);
}

#[tokio::test]
async fn fresh_postdispatch_disconnect_never_retries_or_falls_back() {
    assert_unknown_outcome(false).await;
}

#[tokio::test]
async fn cached_postdispatch_disconnect_never_retries_or_falls_back() {
    assert_unknown_outcome(true).await;
}
