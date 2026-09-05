use std::process::Command;

use director::mcp::engine_api::{EngineApiCategory, EngineApiParams};
use director::server::DirectorServer;

#[test]
fn engine_api_router_has_typed_bounded_contract() {
    let server = DirectorServer::new();
    let route = server
        .tool_router
        .map
        .get("engine_api")
        .expect("engine_api must be registered");

    assert!(route.attr.output_schema.is_some());
    assert!(
        route
            .attr
            .description
            .as_deref()
            .is_some_and(|description| description.contains("bounded pagination"))
    );

    let params: EngineApiParams = serde_json::from_value(serde_json::json!({
        "project_path": "/project",
        "class_name": "Node2D"
    }))
    .unwrap();
    assert!(matches!(params.category, EngineApiCategory::Summary));
    assert_eq!(params.offset, 0);
    assert_eq!(params.limit, 25);
}

#[test]
fn director_help_lists_engine_api() {
    let output = Command::new(env!("CARGO_BIN_EXE_director"))
        .arg("--help")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("engine_api"));
}
