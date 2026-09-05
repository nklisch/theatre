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
    // Fixture geometry, computed INDEPENDENTLY of the production projection
    // code (this is what lets the test catch projection regressions):
    //   camera at (0,0,5), identity rotation (facing -Z), fov 70°,
    //   keep_aspect = KEEP_HEIGHT, image 160x100.
    //   depth = 5; tan(35°) = 0.7002075; half_h = 3.5010377;
    //   half_w = half_h * 1.6 = 5.6016603; nx = x / half_w;
    //   px = (nx + 1) * 80; py = 50.
    //   x=-0.75 -> px=69.29, x=-0.25 -> 76.43, x=0.25 -> 83.57, x=0.75 -> 90.71
    const EXPECTED_PX: [f64; 4] = [69.288909, 76.429637, 83.570363, 90.711091];
    const EXPECTED_PY: f64 = 50.0;
    for (i, (n, x)) in [(1u64, -0.75), (2, -0.25), (3, 0.25), (4, 0.75)]
        .iter()
        .enumerate()
    {
        let entity = FrameEntityData {
            movement: None,
            path: "Enemies/Patrol".into(),
            class: "CharacterBody3D".into(),
            position: vec![*x, 0.0, 0.0],
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
                // Godot 4 Camera3D.KeepAspect: KEEP_WIDTH = 0, KEEP_HEIGHT = 1.
                keep_aspect: 1,
                camera_path: "/root/Camera3D".into(),
            };
            let camera_data = rmp_serde::to_vec(&camera).unwrap();
            db.execute(
                "INSERT INTO camera_frames VALUES (?1, ?2, 'Camera3D', ?3)",
                rusqlite::params![n, 1000 + (n - 1) * 20, camera_data],
            )
            .unwrap();
        }
        let jpeg = jpeg_with_dot(160, 100, EXPECTED_PX[i], EXPECTED_PY);
        db.execute(
            "INSERT INTO screenshots VALUES (?1, ?2, ?3, 160, 100)",
            rusqlite::params![n, 1000 + (n - 1) * 20, jpeg],
        )
        .unwrap();
    }
}

fn make_moving_subpixel_clip(dir: &std::path::Path, id: &str) {
    const WIDTH: u32 = 480;
    const HEIGHT: u32 = 270;
    const CENTERS: [(f64, f64); 4] = [
        (240.0, 135.0),
        (240.25, 135.25),
        (240.5, 135.5),
        (240.75, 135.75),
    ];

    let db = rusqlite::Connection::open(dir.join(format!("{id}.sqlite"))).unwrap();
    db.execute_batch(SCHEMA).unwrap();
    db.execute(
        "INSERT INTO recording VALUES (?1, ?1, 1, 4, 1000, 1060, 3, 60, '{}', 1000)",
        [id],
    )
    .unwrap();
    for (index, (px, py)) in CENTERS.into_iter().enumerate() {
        let frame = index as u64 + 1;
        // With a 270-unit orthographic keep-height camera, one world unit is
        // exactly one pixel at 480x270. This independently places the node at
        // centers whose fractional X and Y components change every frame.
        let entity = FrameEntityData {
            movement: None,
            path: "Enemies/Patrol".into(),
            class: "CharacterBody3D".into(),
            position: vec![px - 240.0, 135.0 - py, 0.0],
            rotation_deg: vec![0.0; 3],
            velocity: vec![1.0, -1.0, 0.0],
            groups: vec![],
            visible: true,
            state: serde_json::Map::new(),
        };
        db.execute(
            "INSERT INTO frames VALUES (?1, ?2, ?3)",
            rusqlite::params![
                frame,
                1000 + index as u64 * 20,
                rmp_serde::to_vec(&vec![entity]).unwrap()
            ],
        )
        .unwrap();
        let camera = CameraFrameData {
            position: vec![0.0, 0.0, 5.0],
            quaternion: vec![0.0, 0.0, 0.0, 1.0],
            projection: 1,
            fov_deg: 70.0,
            ortho_size: HEIGHT as f64,
            keep_aspect: 1,
            camera_path: "/root/Camera3D".into(),
        };
        db.execute(
            "INSERT INTO camera_frames VALUES (?1, ?2, 'Camera3D', ?3)",
            rusqlite::params![
                frame,
                1000 + index as u64 * 20,
                rmp_serde::to_vec(&camera).unwrap()
            ],
        )
        .unwrap();
        db.execute(
            "INSERT INTO screenshots VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                frame,
                1000 + index as u64 * 20,
                jpeg_with_dot(WIDTH, HEIGHT, px, py),
                WIDTH,
                HEIGHT
            ],
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
    // Anti-circularity: the crop centers in the manifest must land on the
    // independently hand-computed screen positions from make_clip (see the
    // geometry comment there), not on whatever the production projection
    // happens to compute.
    const EXPECTED_PX: [f64; 4] = [69.288909, 76.429637, 83.570363, 90.711091];
    let tiles = first.manifest["tiles"].as_array().unwrap();
    assert_eq!(tiles.len(), 4);
    for (tile, expected) in tiles.iter().zip(EXPECTED_PX) {
        let px = tile["px"].as_f64().unwrap();
        let py = tile["py"].as_f64().unwrap();
        assert!(
            (px - expected).abs() < 1.0,
            "tile px {px} deviates from analytic expectation {expected}"
        );
        assert!((py - 50.0).abs() < 1.0, "tile py {py} deviates from 50");
    }
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
async fn node_filmstrip_fixture_keeps_crop_dimensions_fixed_across_subpixel_motion() {
    let tmp = tempfile::TempDir::new().unwrap();
    make_moving_subpixel_clip(tmp.path(), "clip_subpixel");
    let output = artifact(&state(tmp.path()), "clip_subpixel", "Enemies/Patrol")
        .await
        .unwrap();

    assert_eq!(output.manifest["kind"], json!("node_filmstrip"));
    assert_eq!(output.manifest["frames_analyzed"], json!(4));
    let tiles = output.manifest["tiles"].as_array().unwrap();
    for (tile, (expected_x, expected_y)) in tiles.iter().zip([
        (240.0, 135.0),
        (240.25, 135.25),
        (240.5, 135.5),
        (240.75, 135.75),
    ]) {
        assert!((tile["px"].as_f64().unwrap() - expected_x).abs() < 1e-6);
        assert!((tile["py"].as_f64().unwrap() - expected_y).abs() < 1e-6);
    }
    assert!(!output.png.is_empty());
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
