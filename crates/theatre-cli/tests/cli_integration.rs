use std::process::Command;

#[test]
fn help_prints_subcommands() {
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .arg("--help")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("install"));
    assert!(stdout.contains("init"));
    assert!(stdout.contains("deploy"));
    assert!(stdout.contains("enable"));
}

#[test]
fn version_prints_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .arg("--version")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn enable_on_valid_project() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("project.godot"),
        "[editor_plugins]\nenabled=PackedStringArray()\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["enable", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(dir.path().join("project.godot")).unwrap();
    assert!(content.contains("spectator/plugin.cfg"));
    assert!(content.contains("director/plugin.cfg"));
}

#[test]
fn enable_on_missing_project() {
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["enable", "/tmp/nonexistent-project-12345"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn init_fails_without_install() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("project.godot"), "").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["init", dir.path().to_str().unwrap(), "--yes"])
        .env("THEATRE_SHARE_DIR", "/tmp/nonexistent-share-12345")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("install") || stderr.contains("not installed"));
}
