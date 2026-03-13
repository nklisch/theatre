use crate::harness::{DirectorFixture, OperationResultExt, project_dir_path};
use serde_json::json;
use std::io::Write as IoWrite;

#[test]
#[ignore = "requires Godot binary"]
fn journey_wire_main_menu() {
    let f = DirectorFixture::new();
    let scene = DirectorFixture::journey_scene_path("main_menu");

    // 1. Create Control root "MainMenu"
    f.run(
        "scene_create",
        json!({"scene_path": scene, "root_type": "Control", "root_name": "MainMenu"}),
    )
    .unwrap()
    .unwrap_data();

    // 2. Add VBoxContainer "ButtonContainer"
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "VBoxContainer",
            "node_name": "ButtonContainer"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 3. Add Button "PlayButton" under ButtonContainer with text "Play"
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "parent_path": "ButtonContainer",
            "node_type": "Button",
            "node_name": "PlayButton",
            "properties": {"text": "Play"}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 4. Add Button "OptionsButton" with text "Options"
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "parent_path": "ButtonContainer",
            "node_type": "Button",
            "node_name": "OptionsButton",
            "properties": {"text": "Options"}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 5. Add Button "QuitButton" with text "Quit"
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "parent_path": "ButtonContainer",
            "node_type": "Button",
            "node_name": "QuitButton",
            "properties": {"text": "Quit"}
        }),
    )
    .unwrap()
    .unwrap_data();

    // 6. Add Node "MenuLogic" under root (signal handler)
    f.run(
        "node_add",
        json!({
            "scene_path": scene,
            "node_type": "Node",
            "node_name": "MenuLogic"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 7. Create a minimal .gd script file on disk
    let project_dir = project_dir_path();
    let script_dir = project_dir.join("scripts");
    std::fs::create_dir_all(&script_dir).unwrap();
    let script_path = script_dir.join("journey_menu_logic.gd");
    let mut file = std::fs::File::create(&script_path).unwrap();
    IoWrite::write_all(
        &mut file,
        b"extends Node\n\nfunc on_play_pressed():\n\tpass\n\nfunc on_options_pressed():\n\tpass\n\nfunc on_quit_pressed():\n\tpass\n",
    )
    .unwrap();

    // 8. Attach script to MenuLogic
    let script_data = f
        .run(
            "node_set_script",
            json!({
                "scene_path": scene,
                "node_path": "MenuLogic",
                "script_path": "scripts/journey_menu_logic.gd"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert!(
        script_data["script_path"]
            .as_str()
            .unwrap()
            .contains("journey_menu_logic.gd"),
        "Script path not set correctly"
    );

    // 9. signal_connect — PlayButton.pressed → MenuLogic.on_play_pressed
    f.run(
        "signal_connect",
        json!({
            "scene_path": scene,
            "source_path": "ButtonContainer/PlayButton",
            "signal_name": "pressed",
            "target_path": "MenuLogic",
            "method_name": "on_play_pressed"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 10. signal_connect — OptionsButton.pressed → MenuLogic.on_options_pressed
    f.run(
        "signal_connect",
        json!({
            "scene_path": scene,
            "source_path": "ButtonContainer/OptionsButton",
            "signal_name": "pressed",
            "target_path": "MenuLogic",
            "method_name": "on_options_pressed"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 11. signal_connect — QuitButton.pressed → MenuLogic.on_quit_pressed
    f.run(
        "signal_connect",
        json!({
            "scene_path": scene,
            "source_path": "ButtonContainer/QuitButton",
            "signal_name": "pressed",
            "target_path": "MenuLogic",
            "method_name": "on_quit_pressed"
        }),
    )
    .unwrap()
    .unwrap_data();

    // 12. signal_list — verify 3 connections total
    let all_signals = f
        .run("signal_list", json!({"scene_path": scene}))
        .unwrap()
        .unwrap_data();
    let connections = all_signals["connections"].as_array().unwrap();
    assert_eq!(connections.len(), 3, "Expected 3 signal connections");

    // 13. signal_list — filter by PlayButton → 1 connection
    let play_signals = f
        .run(
            "signal_list",
            json!({
                "scene_path": scene,
                "node_path": "ButtonContainer/PlayButton"
            }),
        )
        .unwrap()
        .unwrap_data();
    let play_connections = play_signals["connections"].as_array().unwrap();
    assert_eq!(play_connections.len(), 1);
    assert_eq!(play_connections[0]["signal_name"], "pressed");
    assert_eq!(play_connections[0]["method_name"], "on_play_pressed");

    // 14. node_set_meta — set meta on root
    let meta_data = f
        .run(
            "node_set_meta",
            json!({
                "scene_path": scene,
                "node_path": ".",
                "meta": {"version": 1, "author": "agent"}
            }),
        )
        .unwrap()
        .unwrap_data();
    let meta_keys = meta_data["meta_keys"].as_array().unwrap();
    assert!(meta_keys.iter().any(|k| k == "version"));
    assert!(meta_keys.iter().any(|k| k == "author"));

    // 15. scene_read — verify all buttons have text property, signal handler exists
    let play_btn = f.read_node(&scene, "ButtonContainer/PlayButton");
    assert_eq!(play_btn["type"], "Button");
    assert_eq!(play_btn["properties"]["text"], "Play");

    let options_btn = f.read_node(&scene, "ButtonContainer/OptionsButton");
    assert_eq!(options_btn["properties"]["text"], "Options");

    let quit_btn = f.read_node(&scene, "ButtonContainer/QuitButton");
    assert_eq!(quit_btn["properties"]["text"], "Quit");

    let menu_logic = f.read_node(&scene, "MenuLogic");
    assert_eq!(menu_logic["type"], "Node");
    // Note: scene_read filters out the "script" property, so we verified
    // script attachment via the node_set_script response above (step 8).
}
