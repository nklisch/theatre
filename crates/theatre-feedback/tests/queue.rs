use serde_json::json;
use std::fs;
use theatre_feedback::{Operation, Queue, Response};

fn project() -> tempfile::TempDir {
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("project.godot"), "config_version=5\n").unwrap();
    project
}
fn publish(project: &std::path::Path) -> String {
    let id = "feedback_test";
    let dir = project.join(".theatre/feedback").join(id);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("item.json"),
        json!({
            "feedback_id": id, "source": "runtime", "timestamp_ms": 1,
            "project_path": project, "process_id": 7, "run_id": "run_7",
            "scene": "res://main.tscn", "surface": "root_viewport", "selection": [],
            "pointer": {"status": "inside", "position": [42.0, 20.0]},
            "capture": {"status": "unavailable", "reason": "headless"},
            "readback_render_frame": 4, "readback_physics_frame": 5, "note": "Look here"
        })
        .to_string(),
    )
    .unwrap();
    id.into()
}

#[test]
fn feedback_schemas_are_mcp_root_objects() {
    assert_eq!(
        schemars::schema_for!(Operation).get("type"),
        Some(&json!("object"))
    );
    assert_eq!(
        schemars::schema_for!(Response).get("type"),
        Some(&json!("object"))
    );
}

#[test]
fn two_readers_share_only_explicit_handling_and_deletion() {
    let other = project();
    let project = project();
    let id = publish(project.path());
    let a = Queue::open(project.path()).unwrap();
    let b = Queue::open(project.path()).unwrap();
    let before = fs::read(
        project
            .path()
            .join(".theatre/feedback/feedback_test/item.json"),
    )
    .unwrap();
    for reader in [&a, &b] {
        assert_eq!(reader.status().unwrap().pending_count, 1);
        assert!(
            theatre_feedback::pending_notice(project.path())
                .unwrap()
                .contains("1 pending")
        );
        let Response::Retrieve { item, handled, .. } = reader
            .execute(Operation::Retrieve {
                feedback_id: id.clone(),
            })
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(item.note, "Look here");
        assert!(!handled);
    }
    assert_eq!(
        Queue::open(other.path())
            .unwrap()
            .status()
            .unwrap()
            .pending_count,
        0
    );
    a.execute(Operation::Handle {
        feedback_id: id.clone(),
    })
    .unwrap();
    assert_eq!(b.status().unwrap().pending_count, 0);
    assert!(b.item(&id).is_ok());
    assert_eq!(
        before,
        fs::read(
            project
                .path()
                .join(".theatre/feedback/feedback_test/item.json")
        )
        .unwrap()
    );
    b.execute(Operation::Delete { feedback_id: id }).unwrap();
    assert!(a.status().unwrap().items.is_empty());
}

#[test]
fn interrupted_publications_are_not_feedback_and_cleanup_is_deliberate() {
    let project = project();
    let dir = project
        .path()
        .join(".theatre/feedback/.pending-feedback_interrupted");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("image.jpg"), [0xff, 0xd8, 0]).unwrap();
    let queue = Queue::open(project.path()).unwrap();
    let status = queue.status().unwrap();
    assert_eq!(status.pending_count, 0);
    assert_eq!(status.storage_bytes, 3);
    assert_eq!(status.incomplete.len(), 1);
    assert!(dir.exists());
    assert!(
        queue
            .execute(Operation::Cleanup {
                directory: "../..".into()
            })
            .is_err()
    );
    queue
        .execute(Operation::Cleanup {
            directory: status.incomplete[0].directory.clone(),
        })
        .unwrap();
    assert_eq!(queue.status().unwrap().storage_bytes, 0);
}

#[test]
fn notices_preserve_success_images_structured_content_and_error_meaning() {
    let project = project();
    publish(project.path());
    let mut original = rmcp::model::CallToolResult::success(vec![rmcp::model::Content::image(
        "YWJj",
        "image/jpeg",
    )]);
    original.structured_content = Some(json!({"success": true, "data": {"written": "scene.tscn"}}));
    let mut result = Ok(original.clone());
    theatre_feedback::mcp::append_notice(&mut result, project.path());
    let result = result.unwrap();
    assert_eq!(result.content[0], original.content[0]);
    assert_eq!(result.structured_content, original.structured_content);
    assert_eq!(result.is_error, original.is_error);
    let mut error = Err(rmcp::ErrorData::invalid_params(
        "invalid node",
        Some(json!({"partial": ["saved"]})),
    ));
    theatre_feedback::mcp::append_notice(&mut error, project.path());
    let error = error.unwrap_err();
    assert_eq!(error.code, rmcp::model::ErrorCode::INVALID_PARAMS);
    assert_eq!(error.message, "invalid node");
    assert_eq!(error.data.as_ref().unwrap()["partial"], json!(["saved"]));
    let mut missing = Ok(original.clone());
    theatre_feedback::mcp::append_notice(&mut missing, &project.path().join("missing"));
    assert_eq!(missing.unwrap(), original);
}

#[test]
fn identifiers_and_cross_project_metadata_are_rejected() {
    let a = project();
    let b = project();
    let id = publish(a.path());
    let queue = Queue::open(a.path()).unwrap();
    assert!(
        queue
            .execute(Operation::Delete {
                feedback_id: "../project.godot".into()
            })
            .is_err()
    );
    fs::create_dir_all(b.path().join(".theatre/feedback/feedback_test")).unwrap();
    fs::copy(
        a.path().join(".theatre/feedback/feedback_test/item.json"),
        b.path().join(".theatre/feedback/feedback_test/item.json"),
    )
    .unwrap();
    assert!(Queue::open(b.path()).unwrap().item(&id).is_err());
}
