use std::{fs, path::Path, process::Command};
use theatre_feedback::{Capture, Queue};

fn copy_shared(project: &Path) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/theatre_shared");
    let target = project.join("addons/theatre_shared");
    fs::create_dir_all(&target).unwrap();
    for name in ["feedback.gd", "feedback_composer.gd"] {
        fs::copy(source.join(name), target.join(name)).unwrap();
    }
}

fn copy(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == "bin" {
            continue;
        }
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn runtime(headless: bool) {
    let project = tempfile::tempdir().unwrap();
    copy_shared(project.path());
    fs::write(
        project.path().join("project.godot"),
        "config_version=5\n[rendering]\nrenderer/rendering_method=\"gl_compatibility\"\n",
    )
    .unwrap();
    fs::write(
        project.path().join("runtime.gd"),
        include_str!("fixtures/runtime.gd"),
    )
    .unwrap();
    let mut command = Command::new(std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into()));
    command.args(["--path"]).arg(project.path()).args([
        "--script",
        "res://runtime.gd",
        "--quit-after",
        "300",
        "--accessibility",
        "disabled",
    ]);
    if headless {
        command.arg("--headless");
    }
    let output = command.output().unwrap();
    let logs = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success() && logs.contains("FEEDBACK_RUNTIME_OK"),
        "{logs}"
    );
    assert!(!logs.contains("SCRIPT ERROR"), "{logs}");
    let queue = Queue::open(project.path()).unwrap();
    let status = queue.status().unwrap();
    assert_eq!(status.pending_count, 2, "{logs}\n{status:?}");
    assert_eq!(status.incomplete.len(), 64);
    let first = queue.item(&status.items[0].feedback_id).unwrap();
    assert_eq!(first.note, "Blue surface before composer");
    assert_eq!(first.run_id.as_deref(), Some("run_fixture"));
    let retrieval = theatre_feedback::mcp::execute(
        project.path(),
        theatre_feedback::Operation::Retrieve {
            feedback_id: first.feedback_id.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        retrieval.structured_content.as_ref().unwrap()["item"]["note"],
        first.note
    );
    if !headless {
        use base64::Engine;
        let rmcp::model::RawContent::Image(image) = &retrieval.content[1].raw else {
            panic!("missing MCP image")
        };
        assert_eq!(image.mime_type, "image/jpeg");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&image.data)
            .unwrap();
        assert!(
            !jpeg_decoder::Decoder::new(std::io::Cursor::new(bytes))
                .decode()
                .unwrap()
                .is_empty()
        );
    }
    assert_eq!(queue.status().unwrap().pending_count, 2);
    match first.capture {
        Capture::Unavailable { reason } => {
            assert!(headless);
            assert!(reason.contains("Headless"));
        }
        Capture::Available {
            source_dimensions,
            output_dimensions,
        } => {
            assert!(!headless);
            assert_eq!(
                (source_dimensions.width, source_dimensions.height),
                (1920, 1080)
            );
            assert_eq!(
                (output_dimensions.width, output_dimensions.height),
                (1280, 720)
            );
            let image = fs::File::open(
                project
                    .path()
                    .join(".theatre/feedback")
                    .join(first.feedback_id)
                    .join("image.jpg"),
            )
            .unwrap();
            let mut decoder = jpeg_decoder::Decoder::new(image);
            let pixels = decoder.decode().unwrap();
            assert_eq!(decoder.info().unwrap().width, 1280);
            assert!(
                pixels.chunks_exact(3).all(|pixel| pixel[2] > pixel[0]),
                "expected the blue surface, not the composer"
            );
        }
    }
}

#[test]
#[ignore = "requires Godot"]
fn headless_capture_context_composer_publication_and_capacity() {
    runtime(true);
}

#[test]
#[ignore = "requires graphical Godot"]
fn scaled_runtime_pixels_are_captured_before_composer() {
    runtime(false);
}

#[test]
#[ignore = "requires Godot and built Stage GDExtension"]
fn stage_only_runtime_entrypoint_preserves_recorder_and_gameplay() {
    let project = tempfile::tempdir().unwrap();
    copy_shared(project.path());
    let addon = project.path().join("addons/stage");
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/stage"),
        &addon,
    );
    #[cfg(target_os = "linux")]
    let (platform, library) = ("linux", "libstage_godot.so");
    #[cfg(target_os = "macos")]
    let (platform, library) = ("macos", "libstage_godot.dylib");
    #[cfg(target_os = "windows")]
    let (platform, library) = ("windows", "stage_godot.dll");
    let bin = addon.join("bin").join(platform);
    fs::create_dir_all(&bin).unwrap();
    let executable = std::env::current_exe().unwrap();
    let artifact = executable.parent().unwrap().parent().unwrap().join(library);
    fs::copy(&artifact, bin.join(library)).unwrap_or_else(|error| {
        panic!("Build stage-godot first ({}): {error}", artifact.display())
    });
    fs::write(project.path().join("project.godot"), "config_version=5\n").unwrap();
    fs::write(
        project.path().join("runtime.gd"),
        include_str!("fixtures/stage_runtime.gd"),
    )
    .unwrap();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let output = Command::new(std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into()))
        .args(["--headless", "--path"])
        .arg(project.path())
        .args(["--script", "res://runtime.gd", "--quit-after", "200"])
        .env("THEATRE_PORT", port.to_string())
        .output()
        .unwrap();
    let logs = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success()
            && logs.contains("FEEDBACK_STAGE_OK")
            && !logs.contains("SCRIPT ERROR"),
        "{logs}"
    );
    let queue = Queue::open(project.path()).unwrap();
    let status = queue.status().unwrap();
    assert_eq!(status.pending_count, 2);
    let item = queue.item(&status.items[0].feedback_id).unwrap();
    assert_eq!(item.note, "Runtime entrypoint without agent connection");
    assert!(item.run_id.as_ref().unwrap().starts_with("run_"));
    let active = queue.item(&status.items[1].feedback_id).unwrap();
    assert_eq!(active.note, "Sharing while recorder stays active");
    assert_eq!(active.run_id, item.run_id);
}

#[test]
#[ignore = "requires graphical Godot editor"]
fn editor_2d_viewport_selection_and_dirty_state_survive_sharing() {
    editor_viewport("2D");
}

#[test]
#[ignore = "requires graphical Godot editor"]
fn editor_3d_viewport_selection_and_dirty_state_survive_sharing() {
    editor_viewport("3D");
}

fn editor_viewport(mode: &str) {
    let project = tempfile::tempdir().unwrap();
    copy_shared(project.path());
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/director"),
        &project.path().join("addons/director"),
    );
    fs::create_dir_all(project.path().join("addons/driver")).unwrap();
    fs::write(
        project.path().join("addons/driver/driver.gd"),
        include_str!("fixtures/editor.gd"),
    )
    .unwrap();
    fs::write(project.path().join("addons/driver/plugin.cfg"), "[plugin]\nname=\"Feedback test\"\ndescription=\"Test only\"\nauthor=\"Theatre\"\nversion=\"1\"\nscript=\"driver.gd\"\n").unwrap();
    fs::write(project.path().join("project.godot"), "config_version=5\n[rendering]\nrenderer/rendering_method=\"gl_compatibility\"\n[editor_plugins]\nenabled=PackedStringArray(\"res://addons/director/plugin.cfg\",\"res://addons/driver/plugin.cfg\")\n").unwrap();
    let output = Command::new(std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into()))
        .args(["--editor", "--path"])
        .arg(project.path())
        .args(["--quit-after", "400", "--accessibility", "disabled"])
        .env("DIRECTOR_EDITOR_PORT", "0")
        .env("FEEDBACK_EDITOR_MODE", mode)
        .output()
        .unwrap();
    let logs = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success()
            && logs.contains("FEEDBACK_EDITOR_OK")
            && !logs.contains("SCRIPT ERROR"),
        "{logs}"
    );
    let queue = Queue::open(project.path()).unwrap();
    let status = queue.status().unwrap();
    assert_eq!(status.pending_count, 1, "{logs}");
    let item = queue.item(&status.items[0].feedback_id).unwrap();
    assert_eq!(item.note, "Editor selection and viewport");
    assert_eq!(item.selection.len(), 1);
    assert!(item.selection[0].path.ends_with("SelectedForFeedback"));
    assert!(item.surface.starts_with(if mode == "2D" {
        "editor_2d"
    } else {
        "editor_3d"
    }));
    let image = fs::File::open(
        project
            .path()
            .join(".theatre/feedback")
            .join(item.feedback_id)
            .join("image.jpg"),
    )
    .unwrap();
    let mut decoder = jpeg_decoder::Decoder::new(image);
    assert!(!decoder.decode().unwrap().is_empty());
}
