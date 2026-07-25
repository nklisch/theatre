use jpeg_encoder::{ColorType, Encoder};
use serde_json::json;
use stage_protocol::recording::{CameraFrameData, FrameEntityData};
use stage_server::{clip_artifacts, tcp::SessionState};
use std::sync::Arc;
use tokio::sync::Mutex;

const SCHEMA: &str = r#"
CREATE TABLE recording (id TEXT PRIMARY KEY, name TEXT, started_at_frame INTEGER,
 ended_at_frame INTEGER, started_at_ms INTEGER, ended_at_ms INTEGER,
 scene_dimensions INTEGER, physics_ticks_per_sec INTEGER, capture_config TEXT,
 created_at_unix_ms INTEGER);
CREATE TABLE frames (frame INTEGER PRIMARY KEY, timestamp_ms INTEGER, data BLOB);
CREATE TABLE camera_frames (frame INTEGER PRIMARY KEY, timestamp_ms INTEGER,
 camera_path TEXT, data BLOB);
CREATE TABLE screenshots (frame INTEGER PRIMARY KEY, timestamp_ms INTEGER,
 image_data BLOB, width INTEGER, height INTEGER);
CREATE TABLE markers (id INTEGER PRIMARY KEY AUTOINCREMENT, frame INTEGER,
 timestamp_ms INTEGER, source TEXT, label TEXT);
CREATE TABLE screenshot_gaps (start_frame INTEGER, end_frame INTEGER, reason TEXT,
 dropped INTEGER);
CREATE TABLE artifacts (cache_key TEXT PRIMARY KEY, kind TEXT, params_json TEXT,
 manifest_json TEXT, dims TEXT, png BLOB, created_at_ms INTEGER);
"#;

fn jpeg_with_dot(width: u32, height: u32, x: f64, y: f64) -> Vec<u8> {
    let mut rgb = vec![128u8; (width * height * 3) as usize];
    let cx = x.round() as i32;
    let cy = y.round() as i32;
    for yy in (cy - 1).max(0)..=(cy + 1).min(height as i32 - 1) {
        for xx in (cx - 1).max(0)..=(cx + 1).min(width as i32 - 1) {
            let i = ((yy as u32 * width + xx as u32) * 3) as usize;
            rgb[i..i + 3].copy_from_slice(&[255, 255, 255]);
        }
    }
    let mut jpeg = Vec::new();
    Encoder::new(&mut jpeg, 100)
        .encode(&rgb, width as u16, height as u16, ColorType::Rgb)
        .unwrap();
    jpeg
}

fn make_clip(dir: &std::path::Path, id: &str, with_camera: bool) {
    let db = rusqlite::Connection::open(dir.join(format!("{id}.sqlite"))).unwrap();
    db.execute_batch(SCHEMA).unwrap();
    db.execute(
        "INSERT INTO recording VALUES (?1, ?1, 1, 4, 1000, 1060, 3, 60, '{}', 1000)",
        [id],
    )
    .unwrap();
    let pose = stage_core::projection::CameraPose {
        position: [0.0, 0.0, 5.0],
        quaternion: [0.0, 0.0, 0.0, 1.0],
        projection: stage_core::projection::CameraProjection::Perspective,
        fov_deg: 70.0,
        ortho_size: 10.0,
        keep_aspect: stage_core::projection::KeepAspect::KeepHeight,
    };
    for (n, x) in [(1u64, -0.75), (2, -0.25), (3, 0.25), (4, 0.75)] {
        let entity = FrameEntityData {
            path: "Enemies/Patrol".into(),
            class: "CharacterBody3D".into(),
            position: vec![x, 0.0, 0.0],
            rotation_deg: vec![0.0; 3],
            velocity: vec![1.0, 0.0, 0.0],
            groups: vec![],
            visible: true,
            state: serde_json::Map::new(),
        };
        let data = rmp_serde::to_vec(&vec![entity]).unwrap();
        db.execute(
            "INSERT INTO frames VALUES (?1, ?2, ?3)",
            rusqlite::params![n, 1000 + (n - 1) * 20, data],
        )
        .unwrap();
        if with_camera {
            let camera = CameraFrameData {
                position: vec![0.0, 0.0, 5.0],
                quaternion: vec![0.0, 0.0, 0.0, 1.0],
                projection: 0,
                fov_deg: 70.0,
                ortho_size: 10.0,
                keep_aspect: 0,
            };
            let camera_data = rmp_serde::to_vec(&camera).unwrap();
            db.execute(
                "INSERT INTO camera_frames VALUES (?1, ?2, 'Camera3D', ?3)",
                rusqlite::params![n, 1000 + (n - 1) * 20, camera_data],
            )
            .unwrap();
        }
        let projected =
            stage_core::projection::project_world_to_screen(pose, [x, 0.0, 0.0], 160.0, 100.0);
        let (px, py) = match projected {
            stage_core::projection::ScreenProjection::OnScreen { px, py } => (px, py),
            _ => panic!("fixture point should be on screen"),
        };
        let jpeg = jpeg_with_dot(160, 100, px, py);
        db.execute(
            "INSERT INTO screenshots VALUES (?1, ?2, ?3, 160, 100)",
            rusqlite::params![n, 1000 + (n - 1) * 20, jpeg],
        )
        .unwrap();
    }
}

fn state(dir: &std::path::Path) -> Arc<Mutex<SessionState>> {
    Arc::new(Mutex::new(SessionState {
        clip_storage_path: Some(dir.to_string_lossy().into_owned()),
        ..Default::default()
    }))
}

async fn artifact(
    state: &Arc<Mutex<SessionState>>,
    clip: &str,
    node: &str,
) -> Result<clip_artifacts::ArtifactOutput, rmcp::model::ErrorData> {
    clip_artifacts::generate_artifact(
        state,
        Some(clip),
        "node_filmstrip",
        None,
        None,
        None,
        Some(4),
        Some(node),
        Some(0.25),
        true,
        1500,
        5000,
    )
    .await
}

#[tokio::test]
async fn node_filmstrip_fixture_projects_deterministically_and_caches() {
    let tmp = tempfile::TempDir::new().unwrap();
    make_clip(tmp.path(), "clip_track", true);
    let state = state(tmp.path());
    let first = artifact(&state, "clip_track", "Enemies/Patrol")
        .await
        .unwrap();
    assert_eq!(first.manifest["kind"], json!("node_filmstrip"));
    assert!(
        first.manifest["projection"]["counts"]["on_screen"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(first.manifest["tiles"].is_array());
    assert_eq!(first.cache, "stored");
    let second = artifact(&state, "clip_track", "Enemies/Patrol")
        .await
        .unwrap();
    assert_eq!(second.cache, "hit");
    assert_eq!(first.png, second.png);
    assert_ne!(
        clip_artifacts::cache_key("node_filmstrip", &json!({"node":"A"}), "fp"),
        clip_artifacts::cache_key("node_filmstrip", &json!({"node":"B"}), "fp")
    );
}

#[tokio::test]
async fn node_filmstrip_fixture_reports_content_degradations() {
    let tmp = tempfile::TempDir::new().unwrap();
    make_clip(tmp.path(), "clip_missing_node", true);
    make_clip(tmp.path(), "clip_no_camera", false);
    let state = state(tmp.path());
    let missing = artifact(&state, "clip_missing_node", "Enemies/Ghost")
        .await
        .unwrap_err();
    assert_eq!(
        missing.data.as_ref().unwrap()["error"],
        json!("node_not_found")
    );
    assert!(
        !missing.data.as_ref().unwrap()["sample_paths"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let no_camera = artifact(&state, "clip_no_camera", "Enemies/Patrol")
        .await
        .unwrap_err();
    assert_eq!(
        no_camera.data.as_ref().unwrap()["error"],
        json!("no_camera_data")
    );
}
