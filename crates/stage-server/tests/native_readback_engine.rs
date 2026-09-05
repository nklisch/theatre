//! Exercises the actual recorder/backend in an isolated graphical Godot process.
use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
#[ignore = "requires graphical Godot 4.7 Compatibility and built GDExtension"]
fn native_readback_is_reduced_delayed_and_oriented() {
    run_journey("auto", false);
}

#[test]
#[ignore = "requires graphical Godot 4.7 Compatibility and built GDExtension"]
fn native_readback_retires_on_separate_render_thread() {
    run_journey("auto", true);
}

#[test]
#[ignore = "requires graphical Godot and built GDExtension"]
fn explicit_synchronous_recovery_preserves_pixels() {
    run_journey("synchronous", false);
}

#[test]
#[ignore = "requires graphical Godot 4.7 Forward+ and built GDExtension"]
fn forward_plus_auto_is_unavailable_until_explicit_synchronous_recovery() {
    run_journey("forward_plus", false);
}

#[test]
#[ignore = "requires headless Godot and built GDExtension"]
fn unavailable_rendering_keeps_spatial_capture() {
    run_journey("headless", false);
}

fn run_journey(mode: &str, separate_render_thread: bool) {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let library = if cfg!(target_os = "windows") {
        "stage_godot.dll"
    } else if cfg!(target_os = "macos") {
        "libstage_godot.dylib"
    } else {
        "libstage_godot.so"
    };
    let exe = std::env::current_exe().unwrap();
    std::fs::copy(
        exe.parent().unwrap().parent().unwrap().join(library),
        dir.path().join(library),
    )
    .unwrap();
    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    std::fs::write(dir.path().join("stage.gdextension"), format!("[configuration]\nentry_symbol=\"gdext_rust_init\"\ncompatibility_minimum=\"4.7\"\n[libraries]\n{platform}.debug=\"res://{library}\"\n{platform}.release=\"res://{library}\"\n")).unwrap();
    std::fs::write(dir.path().join("project.godot"), "config_version=5\n[application]\nconfig/name=\"Native readback journey\"\n[rendering]\nrenderer/rendering_method=\"gl_compatibility\"\n").unwrap();
    std::fs::copy(
        repo.join("tests/godot-project/tests/native_readback_journey.gd"),
        dir.path().join("journey.gd"),
    )
    .unwrap();
    let log_path = dir.path().join("engine.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let mut command = Command::new(std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into()));
    if mode == "headless" {
        command.arg("--headless");
    } else if mode == "forward_plus" {
        // Exercise non-OpenGL capture without changing any project renderer.
        command.args(["--rendering-method", "forward_plus"]);
    }
    if separate_render_thread {
        command.args(["--render-thread", "separate"]);
    }
    let mut child = command
        .env("READBACK_MODE", mode)
        .args(["--path"])
        .arg(dir.path())
        .args(["--script", "res://journey.gd"])
        .env("XDG_DATA_HOME", dir.path().join("user-data"))
        .env("APPDATA", dir.path().join("user-data"))
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .unwrap();
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "Native journey timed out: {}",
                std::fs::read_to_string(&log_path).unwrap()
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let output = std::fs::read_to_string(log_path).unwrap();
    eprintln!("{output}");
    assert!(
        status.success() && !output.contains("SCRIPT ERROR") && !output.contains("ERROR:"),
        "{output}"
    );
    let report: serde_json::Value = serde_json::from_str(
        output
            .lines()
            .find_map(|line| line.strip_prefix("NATIVE_READBACK_REPORT:"))
            .expect("readback report"),
    )
    .unwrap();
    assert_eq!(report["failures"], serde_json::json!([]));
    if mode == "forward_plus" {
        assert_eq!(report["reports"].as_array().unwrap().len(), 2);
        assert_eq!(report["reports"][0]["phase"], "auto_unavailable");
        assert_eq!(report["reports"][1]["phase"], "synchronous_recovery");
    }
    for phase in report["reports"].as_array().unwrap() {
        let clip = &phase["status"]["last_saved_clip"];
        let database = PathBuf::from(report["storage_path"].as_str().unwrap())
            .join(format!("{}.sqlite", clip["clip_id"].as_str().unwrap()));
        let db = rusqlite::Connection::open(database).unwrap();
        if mode == "forward_plus" {
            let spatial_frames: u32 = db
                .query_row("SELECT COUNT(*) FROM frames", [], |row| row.get(0))
                .unwrap();
            assert!(spatial_frames > 0, "Forward+ must retain spatial evidence");
            if phase["phase"] == "auto_unavailable" {
                let screenshots: u32 = db
                    .query_row("SELECT COUNT(*) FROM screenshots", [], |row| row.get(0))
                    .unwrap();
                assert_eq!(
                    screenshots, 0,
                    "Auto must not fall back to synchronous pixels"
                );
                continue;
            }
        }
        if mode == "auto" && phase["phase"] == "invalidated_pending" {
            let invalidated: u32 = db
                .query_row(
                    "SELECT COALESCE(SUM(dropped), 0) FROM screenshot_gaps WHERE reason='capture_generation_changed'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(invalidated, 1, "The one pending image must have one loss");
            let unfinished: u32 = db
                .query_row(
                    "SELECT COALESCE(SUM(dropped), 0) FROM screenshot_gaps WHERE reason='unavailable_at_save'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                unfinished, 0,
                "Save must not count the already-invalidated pending image again"
            );
        }
        if mode == "auto" && phase["phase"] == "initial" {
            let unfinished: u32 = db
                .query_row(
                    "SELECT COUNT(*) FROM screenshot_gaps WHERE reason='unavailable_at_save'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(
                unfinished > 0,
                "Saving while GPU pending must retain unavailable-at-save evidence"
            );
            let latest_frame: u64 = db
                .query_row("SELECT MAX(frame) FROM screenshots", [], |row| row.get(0))
                .unwrap();
            assert_eq!(
                latest_frame,
                phase["status"]["capture_probe"]["last_request_frame"]
                    .as_f64()
                    .unwrap() as u64,
                "JPEG metadata must use the request frame, not its later completion frame"
            );
            assert!(
                latest_frame
                    < phase["status"]["capture_probe"]["last_completion_frame"]
                        .as_f64()
                        .unwrap() as u64
            );
        }
        let (jpeg, width, height): (Vec<u8>, u32, u32) = db
            .query_row(
                "SELECT image_data,width,height FROM screenshots ORDER BY frame DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(width as u64, phase["width"].as_u64().unwrap());
        assert_eq!(height as u64, phase["height"].as_u64().unwrap());
        let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(jpeg));
        let pixels = decoder.decode().unwrap();
        let info = decoder.info().unwrap();
        assert_eq!((info.width as u32, info.height as u32), (width, height));
        let top = ((height / 4 * width + width / 2) * 3) as usize;
        let bottom = ((height * 3 / 4 * width + width / 2) * 3) as usize;
        assert!(
            pixels[top] > 220 && pixels[top + 2] < 30,
            "top should be red: {:?}",
            &pixels[top..top + 3]
        );
        assert!(
            pixels[bottom + 2] > 220 && pixels[bottom] < 30,
            "bottom should be blue: {:?}",
            &pixels[bottom..bottom + 3]
        );
    }
}
