//! Exercise MCP envelopes through the SDK transport, not direct tool methods.
use rmcp::{ServiceExt, model::CallToolRequestParams};
use serde_json::{Value, json};
use stage_server::{server::StageServer, tcp::SessionState};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn disconnected_status_and_watch_variants_keep_structured_content() {
    let project = tempfile::tempdir().unwrap();
    let state = Arc::new(Mutex::new(SessionState {
        project_dir: project.path().into(),
        ..Default::default()
    }));
    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let server_task = tokio::spawn(StageServer::new(state).serve(server_io));
    let client = ().serve(client_io).await.unwrap();
    let server = server_task.await.unwrap().unwrap();
    let tools = client.list_all_tools().await.unwrap();
    // Check generated schema positions, not JSON examples containing literal
    // fields named nullable. The real Pi client rejects OpenAPI nullability.
    fn standard_schema(schema: &mut schemars::Schema) {
        if let Some(object) = schema.as_object() {
            assert!(
                !object.contains_key("nullable"),
                "OpenAPI keyword in MCP schema: {schema:?}"
            );
        }
        schemars::transform::transform_subschemas(&mut standard_schema, schema);
    }
    for tool in &tools {
        let mut input: schemars::Schema = Value::Object(tool.input_schema.as_ref().clone())
            .try_into()
            .unwrap();
        standard_schema(&mut input);
        if let Some(output) = &tool.output_schema {
            let mut output: schemars::Schema =
                Value::Object(output.as_ref().clone()).try_into().unwrap();
            standard_schema(&mut output);
        }
    }
    for name in ["spatial_snapshot", "spatial_watch"] {
        let tool = tools.iter().find(|t| t.name == name).unwrap();
        let schema = tool.output_schema.as_ref().unwrap();
        assert_eq!(schema["type"], "object");
        assert!(schema["anyOf"].as_array().unwrap().len() >= 3);
    }
    for (name, arguments, expected_field) in [
        ("runtime_status", json!({}), "connected"),
        (
            "spatial_watch",
            json!({"action":"add", "watch":{"node":"player"}}),
            "watch_id",
        ),
        ("spatial_watch", json!({"action":"list"}), "watches"),
        (
            "spatial_watch",
            json!({"action":"remove", "watch_id":"missing"}),
            "result",
        ),
        ("spatial_watch", json!({"action":"clear"}), "removed"),
    ] {
        let result = client
            .call_tool(CallToolRequestParams {
                name: name.into(),
                arguments: arguments.as_object().cloned(),
                meta: None,
                task: None,
            })
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true));
        let structured = result
            .structured_content
            .expect("advertised output requires data");
        assert!(
            structured.get(expected_field).is_some(),
            "{name}: {structured}"
        );
        let text: Value = serde_json::from_str(&result.content[0].as_text().unwrap().text).unwrap();
        assert_eq!(text, structured);
        if name == "runtime_status" {
            assert_eq!(structured["connected"], false);
            assert_eq!(structured["ready"], false);
        }
    }
    client.cancel().await.unwrap();
    server.cancel().await.unwrap();
}
