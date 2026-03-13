use crate::harness::DirectorFixture;
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn journey_uid_workflow() {
    let f = DirectorFixture::new();
    let scene_a = DirectorFixture::journey_scene_path("uid_scene_a");
    let scene_b = DirectorFixture::journey_scene_path("uid_scene_b");
    let scene_c = DirectorFixture::journey_scene_path("uid_scene_c");

    // 1. Create 3 scenes
    for scene in [&scene_a, &scene_b, &scene_c] {
        f.run(
            "scene_create",
            json!({"scene_path": scene, "root_type": "Node2D"}),
        )
        .unwrap()
        .unwrap_data();
    }

    // 2. uid_get — Resolve UID for each scene
    let uid_a = f
        .run("uid_get", json!({"file_path": scene_a}))
        .unwrap()
        .unwrap_data();
    let uid_b = f
        .run("uid_get", json!({"file_path": scene_b}))
        .unwrap()
        .unwrap_data();
    let uid_c = f
        .run("uid_get", json!({"file_path": scene_c}))
        .unwrap()
        .unwrap_data();

    // 3. Verify all UIDs are unique and start with "uid://"
    let ua = uid_a["uid"].as_str().unwrap();
    let ub = uid_b["uid"].as_str().unwrap();
    let uc = uid_c["uid"].as_str().unwrap();

    assert!(
        ua.starts_with("uid://"),
        "UID A should start with uid://, got: {ua}"
    );
    assert!(
        ub.starts_with("uid://"),
        "UID B should start with uid://, got: {ub}"
    );
    assert!(
        uc.starts_with("uid://"),
        "UID C should start with uid://, got: {uc}"
    );

    assert_ne!(ua, ub, "UIDs A and B should be unique");
    assert_ne!(ub, uc, "UIDs B and C should be unique");
    assert_ne!(ua, uc, "UIDs A and C should be unique");

    // 4. uid_update_project — Scan tmp/ directory
    let scan = f
        .run("uid_update_project", json!({"directory": "tmp"}))
        .unwrap()
        .unwrap_data();

    // 5. Verify files_scanned > 0
    assert!(
        scan["files_scanned"].as_u64().unwrap() > 0,
        "uid_update_project should scan at least 1 file"
    );
    assert!(
        scan.get("uids_registered").is_some(),
        "Should report uids_registered"
    );
}
