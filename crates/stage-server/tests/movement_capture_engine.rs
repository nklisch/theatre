use serde_json::{Value, json};
use stage_protocol::recording::FrameEntityData;
use stage_server::clip_analysis::{snapshot_at, trajectory};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
#[ignore = "requires Godot and built GDExtension"]
fn saved_movement_distinguishes_idle_attempted_and_blocked() {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (platform, library) = if cfg!(target_os = "windows") {
        ("windows", "stage_godot.dll")
    } else if cfg!(target_os = "macos") {
        ("macos", "libstage_godot.dylib")
    } else {
        ("linux", "libstage_godot.so")
    };
    let addon = dir.path().join("addons/stage");
    let binary = addon.join("bin").join(platform);
    std::fs::create_dir_all(&binary).unwrap();
    let executable = std::env::current_exe().unwrap();
    let build = executable.parent().unwrap().parent().unwrap();
    std::fs::copy(build.join(library), binary.join(library)).unwrap();
    std::fs::copy(
        repo.join("addons/stage/stage.gdextension"),
        addon.join("stage.gdextension"),
    )
    .unwrap();
    std::fs::copy(
        repo.join("tests/godot-project/tests/movement_capture_journey.gd"),
        dir.path().join("journey.gd"),
    )
    .unwrap();
    std::fs::write(
        dir.path().join("project.godot"),
        "config_version=5\n[application]\nconfig/name=\"Movement evidence journey\"\n",
    )
    .unwrap();
    let log = dir.path().join("godot.log");
    let output = std::fs::File::create(&log).unwrap();
    let mut child = Command::new(std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into()))
        .args(["--headless", "--path"])
        .arg(dir.path())
        .args(["--script", "res://journey.gd"])
        .env("XDG_DATA_HOME", dir.path().join("user-data"))
        .env("APPDATA", dir.path().join("user-data"))
        .stdout(Stdio::from(output.try_clone().unwrap()))
        .stderr(Stdio::from(output))
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "Movement journey timed out: {}",
                std::fs::read_to_string(&log).unwrap()
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let output = std::fs::read_to_string(log).unwrap();
    assert!(
        status.success() && !output.contains("SCRIPT ERROR"),
        "{output}"
    );
    let report: Value = serde_json::from_str(
        output
            .lines()
            .find_map(|line| line.strip_prefix("MOVEMENT_REPORT:"))
            .expect(&output),
    )
    .unwrap();
    assert_eq!(report["failures"], json!([]), "{report}");
    let db = rusqlite::Connection::open(format!(
        "{}/{}.sqlite",
        report["storage"].as_str().unwrap(),
        report["clip_id"].as_str().unwrap()
    ))
    .unwrap();
    for phase in ["disabled", "idle", "attempted", "blocked"] {
        let frame = report["ranges"][phase].as_u64().unwrap();
        let bytes: Vec<u8> = db
            .query_row(
                "SELECT data FROM frames WHERE frame<=?1 ORDER BY frame DESC LIMIT 1",
                [frame],
                |row| row.get(0),
            )
            .unwrap();
        let entities: Vec<FrameEntityData> = rmp_serde::from_slice(&bytes).unwrap();
        assert!(
            entities
                .iter()
                .filter(|entity| entity.path != "Player")
                .all(|entity| entity.movement.is_none())
        );
        let player = entities
            .iter()
            .find(|entity| entity.path == "Player")
            .unwrap();
        if phase == "disabled" {
            assert!(player.movement.is_none());
            continue;
        }
        let movement = player.movement.as_ref().unwrap();
        assert_eq!(
            movement.input_actions["attempt_right"],
            if phase == "idle" { 0.0 } else { 1.0 }
        );
        assert!(movement.on_floor, "{phase}: {movement:?}");
        assert!(movement.floor_normal.unwrap()[1] > 0.9);
        assert!(
            movement.slide_contact_normals.len()
                <= stage_protocol::recording::MAX_SLIDE_CONTACT_NORMALS
        );
        if movement.slide_contacts_truncated {
            assert_eq!(
                movement.slide_contact_normals.len(),
                stage_protocol::recording::MAX_SLIDE_CONTACT_NORMALS
            );
        }
        eprintln!("{phase}: {movement:?}");
        if phase == "attempted" {
            assert!(movement.real_velocity[0] > 3.0, "{movement:?}");
            assert!(!movement.on_wall);
        } else {
            assert!(
                movement.real_velocity[0].abs() < 0.05,
                "{phase}: {movement:?}"
            );
        }
        if phase == "blocked" {
            assert!(movement.on_wall);
            assert!(
                movement
                    .slide_contact_normals
                    .iter()
                    .any(|normal| normal[0] < -0.9)
            );
        }
    }
    // Verify the persisted evidence is exposed by the actual saved-analysis readers.
    let frame = report["ranges"]["blocked"].as_u64().unwrap();
    let frame: u64 = db
        .query_row(
            "SELECT MAX(frame) FROM frames WHERE frame<=?1",
            [frame],
            |row| row.get(0),
        )
        .unwrap();
    let mut statement = db
        .prepare("SELECT frame FROM frames ORDER BY frame")
        .unwrap();
    let frames: Vec<u64> = statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(
        frames.windows(2).all(|pair| pair[1] - pair[0] == 2),
        "spatial cadence: {frames:?}"
    );
    let snapshot = snapshot_at(&db, frame, "standard", 10000, 10000).unwrap();
    let player = snapshot["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entity| entity["path"] == "Player")
        .unwrap();
    assert_eq!(player["movement"]["on_wall"], true);
    let samples = trajectory(&db, "Player", frame, frame, &["movement".into()], 1, 10000).unwrap();
    assert_eq!(
        samples["samples"][0]["movement"]["input_actions"]["attempt_right"],
        1.0
    );
}
