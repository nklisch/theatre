use crate::harness::{DaemonFixture, DirectorFixture, OperationResultExt, assert_approx};
use director::mcp::engine_api::EngineApiResponse;
use serde_json::json;

#[test]
#[ignore = "requires Godot binary"]
fn engine_api_metadata_feeds_scene_and_resource_authoring() {
    let fixture = DirectorFixture::new();

    let position = fixture
        .run(
            "engine_api",
            json!({
                "class_name": "Node2D",
                "category": "properties",
                "member": "position"
            }),
        )
        .unwrap()
        .unwrap_data();
    let _: EngineApiResponse = serde_json::from_value(position.clone()).unwrap();
    let position_property = &position["members"][0];
    assert_eq!(position_property["kind"], "property");
    assert_eq!(position_property["type_name"], "Vector2");
    assert_eq!(
        position_property["default_value"]["representation"],
        "serialized"
    );
    assert!(position["engine_version"]["major"].as_u64().unwrap() >= 4);

    let scene_path = DirectorFixture::temp_scene_path("engine_api_scene");
    fixture
        .run(
            "scene_create",
            json!({"scene_path": scene_path, "root_type": "Node2D"}),
        )
        .unwrap()
        .unwrap_data();
    fixture
        .run(
            "node_add",
            json!({
                "scene_path": scene_path,
                "node_type": "Node2D",
                "node_name": "DiscoveredNode",
                "properties": {"position": {"x": 12.0, "y": 34.0}}
            }),
        )
        .unwrap()
        .unwrap_data();
    let node = fixture.read_node(&scene_path, "DiscoveredNode");
    assert_approx(node["properties"]["position"]["x"].as_f64().unwrap(), 12.0);

    let roughness = fixture
        .run(
            "engine_api",
            json!({
                "class_name": "StandardMaterial3D",
                "category": "properties",
                "member": "roughness"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(roughness["members"][0]["type_name"], "float");

    let resource_path = DirectorFixture::temp_resource_path("engine_api_material");
    fixture
        .run(
            "material_create",
            json!({
                "resource_path": resource_path,
                "material_type": "StandardMaterial3D",
                "properties": {"roughness": 0.35}
            }),
        )
        .unwrap()
        .unwrap_data();
    let material = fixture
        .run("resource_read", json!({"resource_path": resource_path}))
        .unwrap()
        .unwrap_data();
    assert_approx(material["properties"]["roughness"].as_f64().unwrap(), 0.35);
}

#[test]
#[ignore = "requires Godot binary"]
fn engine_api_reports_inherited_members_and_focused_categories() {
    let fixture = DirectorFixture::new();

    let visible = fixture
        .run(
            "engine_api",
            json!({
                "class_name": "Button",
                "category": "properties",
                "member": "visible"
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(visible["page"]["total"], 1);
    assert_eq!(visible["members"][0]["declared_by"], "CanvasItem");

    let signal = fixture
        .run(
            "engine_api",
            json!({"class_name": "Button", "category": "signals", "member": "pressed"}),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(signal["members"][0]["kind"], "signal");

    let process_mode = fixture
        .run(
            "engine_api",
            json!({"class_name": "Node", "category": "enums", "member": "ProcessMode"}),
        )
        .unwrap()
        .unwrap_data();
    assert!(
        process_mode["members"][0]["values"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["name"] == "PROCESS_MODE_ALWAYS")
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn engine_api_paginates_deterministically_and_rejects_invalid_pages() {
    let fixture = DirectorFixture::new();

    let properties = fixture
        .run(
            "engine_api",
            json!({"class_name": "Node2D", "category": "properties", "limit": 100}),
        )
        .unwrap()
        .unwrap_data();
    let property_members = properties["members"].as_array().unwrap();
    const PROPERTY_HEADING_USAGE: u64 = 64 | 128 | 256;
    assert_eq!(
        properties["counts"]["properties"],
        properties["page"]["total"]
    );
    assert!(
        property_members
            .iter()
            .all(|property| { property["usage"].as_u64().unwrap() & PROPERTY_HEADING_USAGE == 0 })
    );
    assert!(
        property_members
            .iter()
            .any(|property| { property["name"] == "name" && property["usage"] == 0 })
    );
    assert!(!property_members.iter().any(|property| {
        matches!(
            property["name"].as_str(),
            Some("Auto Translate" | "Transform" | "Visibility" | "Thread Group")
        )
    }));

    let integral_float = fixture
        .run(
            "engine_api",
            json!({"class_name": "Node", "category": "methods", "limit": 2.0}),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(integral_float["members"].as_array().unwrap().len(), 2);

    let first = fixture
        .run(
            "engine_api",
            json!({"class_name": "Node", "category": "methods", "limit": 2}),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(first["members"].as_array().unwrap().len(), 2);
    assert_eq!(first["page"]["next_offset"], 2);

    let second = fixture
        .run(
            "engine_api",
            json!({"class_name": "Node", "category": "methods", "offset": 2.0, "limit": 2}),
        )
        .unwrap()
        .unwrap_data();
    assert_ne!(first["members"][0]["name"], second["members"][0]["name"]);

    let error = fixture
        .run(
            "engine_api",
            json!({"class_name": "Node", "category": "methods", "offset": 100000}),
        )
        .unwrap()
        .unwrap_err();
    assert!(error.contains("outside methods results"));

    let fractional_limit = fixture
        .run(
            "engine_api",
            json!({"class_name": "Node", "category": "methods", "limit": 1.5}),
        )
        .unwrap()
        .unwrap_err();
    assert!(fractional_limit.contains("limit must be an integer"));

    let nonnumeric_limit = fixture
        .run(
            "engine_api",
            json!({"class_name": "Node", "category": "methods", "limit": "2"}),
        )
        .unwrap()
        .unwrap_err();
    assert!(nonnumeric_limit.contains("limit must be an integer"));

    let fractional_offset = fixture
        .run(
            "engine_api",
            json!({"class_name": "Node", "category": "methods", "offset": 1.5}),
        )
        .unwrap()
        .unwrap_err();
    assert!(fractional_offset.contains("offset must be an integer"));

    let nonnumeric_offset = fixture
        .run(
            "engine_api",
            json!({"class_name": "Node", "category": "methods", "offset": "2"}),
        )
        .unwrap()
        .unwrap_err();
    assert!(nonnumeric_offset.contains("offset must be an integer"));
}

#[test]
#[ignore = "requires Godot binary"]
fn engine_api_rejects_unknown_classes_and_members() {
    let fixture = DirectorFixture::new();
    let class_error = fixture
        .run(
            "engine_api",
            json!({"class_name": "DefinitelyNotAGodotClass"}),
        )
        .unwrap()
        .unwrap_err();
    assert!(class_error.contains("Unknown ClassDB class"));

    let member_error = fixture
        .run(
            "engine_api",
            json!({
                "class_name": "Node2D",
                "category": "properties",
                "member": "definitely_not_a_property"
            }),
        )
        .unwrap()
        .unwrap_err();
    assert!(member_error.contains("Unknown property member"));
}

#[test]
#[ignore = "requires Godot binary"]
fn engine_api_marks_unsupported_non_scalar_defaults_as_text_only() {
    let fixture = DirectorFixture::new();
    let polygon = fixture
        .run(
            "engine_api",
            json!({
                "class_name": "Polygon2D",
                "category": "properties",
                "member": "polygon"
            }),
        )
        .unwrap()
        .unwrap_data();
    let default_value = &polygon["members"][0]["default_value"];
    assert_eq!(default_value["native_type"], "PackedVector2Array");
    assert_eq!(default_value["representation"], "text");
    assert!(default_value["value"].is_null());
    assert!(
        default_value["text"]
            .as_str()
            .is_some_and(|text| !text.is_empty())
    );
}

#[test]
#[ignore = "requires Godot binary"]
fn engine_api_dispatches_through_daemon() {
    let mut daemon = DaemonFixture::start_with_port(16650);
    let data = daemon
        .run("engine_api", json!({"class_name": "Node2D"}))
        .unwrap()
        .unwrap_data();
    assert_eq!(data["category"], "summary");
    assert_eq!(data["class"]["class_name"], "Node2D");
}

#[test]
#[ignore = "requires Godot binary"]
fn engine_api_is_available_inside_existing_batches() {
    let fixture = DirectorFixture::new();
    let batch = fixture
        .run(
            "batch",
            json!({
                "operations": [{
                    "operation": "engine_api",
                    "params": {"class_name": "Node2D"}
                }]
            }),
        )
        .unwrap()
        .unwrap_data();
    assert_eq!(batch["completed"], 1);
    assert_eq!(batch["results"][0]["data"]["category"], "summary");
}
