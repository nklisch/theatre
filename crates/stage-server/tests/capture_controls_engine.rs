use std::{
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn copy_scripts(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == "bin" {
            continue;
        }
        let dest = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_scripts(&entry.path(), &dest);
        } else if matches!(
            entry.path().extension().and_then(|value| value.to_str()),
            Some("gd" | "gdextension")
        ) {
            std::fs::copy(entry.path(), dest).unwrap();
        }
    }
}

fn run_journey(script: &str, headless: bool, environment: &[(&str, &str)]) -> String {
    let dir = tempfile::tempdir().unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for addon in ["stage", "theatre_shared"] {
        copy_scripts(
            &repo.join("addons").join(addon),
            &dir.path().join("addons").join(addon),
        );
    }
    let (platform, library) = if cfg!(target_os = "windows") {
        ("windows", "stage_godot.dll")
    } else if cfg!(target_os = "macos") {
        ("macos", "libstage_godot.dylib")
    } else {
        ("linux", "libstage_godot.so")
    };
    let binary = dir.path().join("addons/stage/bin").join(platform);
    std::fs::create_dir_all(&binary).unwrap();
    let executable = std::env::current_exe().unwrap();
    let build = executable.parent().unwrap().parent().unwrap();
    std::fs::copy(build.join(library), binary.join(library)).unwrap();
    std::fs::copy(
        repo.join("tests/godot-project/tests").join(script),
        dir.path().join("journey.gd"),
    )
    .unwrap();
    std::fs::write(dir.path().join("project.godot"), "config_version=5\n[application]\nconfig/name=\"Capture journey\"\n[rendering]\nrenderer/rendering_method=\"gl_compatibility\"\n").unwrap();
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let log = dir.path().join("godot.log");
    let output = std::fs::File::create(&log).unwrap();
    let mut command = Command::new(std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into()));
    if headless {
        command.arg("--headless");
    }
    let mut child = command
        .args(["--path"])
        .arg(dir.path())
        .args(["--script", "res://journey.gd"])
        .env("THEATRE_PORT", port.to_string())
        .env("XDG_DATA_HOME", dir.path().join("user-data"))
        .env("APPDATA", dir.path().join("user-data"))
        .envs(environment.iter().copied())
        .stdout(Stdio::from(output.try_clone().unwrap()))
        .stderr(Stdio::from(output))
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "Capture journey timed out: {}",
                std::fs::read_to_string(&log).unwrap()
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let output = std::fs::read_to_string(log).unwrap();
    assert!(status.success(), "Capture journey failed: {output}");
    assert!(!output.contains("SCRIPT ERROR"), "Script errors: {output}");
    output
}

#[test]
#[ignore = "requires graphical Godot and built GDExtension"]
fn human_capture_controls_preserve_intent_and_fit_small_viewports() {
    let output = run_journey("capture_controls_journey.gd", false, &[]);
    assert!(
        output.contains("CAPTURE_CONTROL_REPORT:{\"failures\":[]}"),
        "{output}"
    );
}

#[test]
#[ignore = "requires graphical Godot and built GDExtension; measures representative capture cost"]
fn measure_recording_presets_on_a_moving_scene() {
    for profile in ["disabled", "lightweight", "detailed"] {
        let output = run_journey(
            "capture_overhead_journey.gd",
            false,
            &[("CAPTURE_PROFILE", profile)],
        );
        let report = output
            .lines()
            .find_map(|line| line.strip_prefix("CAPTURE_BENCHMARK:"))
            .expect("benchmark report");
        let report: serde_json::Value = serde_json::from_str(report).unwrap();
        assert!(report["physics_ticks"].as_u64().unwrap() >= 60, "{report}");
        assert!(report["physics_ms_p95"].as_f64().unwrap().is_finite());
        if profile == "disabled" {
            assert_eq!(report["status"]["buffer_frames"].as_f64(), Some(0.0));
        } else {
            assert!(report["status"]["buffer_frames"].as_f64().unwrap() > 0.0);
            assert!(
                report["status"]["screenshot_buffer_count"]
                    .as_f64()
                    .unwrap()
                    > 0.0
            );
        }
        eprintln!("CAPTURE_BENCHMARK:{report}");
    }
}
