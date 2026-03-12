/// Wire tests for the handshake protocol.
use crate::harness::GodotFixture;

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn handshake_reports_3d_scene() {
    let fixture = GodotFixture::start("test_scene_3d.tscn").unwrap();
    assert_eq!(fixture.handshake.scene_dimensions, 3);
    assert!(!fixture.handshake.project_name.is_empty());
    assert!(
        fixture.handshake.godot_version.starts_with("4."),
        "expected Godot 4.x, got: {}",
        fixture.handshake.godot_version
    );
    assert_eq!(fixture.handshake.physics_ticks_per_sec, 60);
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn handshake_reports_2d_scene() {
    let fixture = GodotFixture::start("test_scene_2d.tscn").unwrap();
    assert_eq!(fixture.handshake.scene_dimensions, 2);
}

#[test]
#[ignore = "requires Godot binary and built GDExtension"]
fn handshake_has_non_empty_session_info() {
    let fixture = GodotFixture::start("test_scene_3d.tscn").unwrap();
    assert!(
        !fixture.handshake.project_name.is_empty(),
        "project_name empty"
    );
    assert!(
        !fixture.handshake.godot_version.is_empty(),
        "godot_version empty"
    );
}
