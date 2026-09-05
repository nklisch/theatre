mod support;

use rmcp::{handler::server::wrapper::Parameters, model::RawContent};
use serde_json::json;
use stage_protocol::viewport::ViewportParams;
use stage_server::mcp::viewport::handle_viewport_cli;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use support::harness::TestHarness;

#[tokio::test]
async fn viewport_boundary_validates_size_and_preserves_image_and_provenance() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let harness = TestHarness::new(Arc::new(move |method, params| {
        assert_eq!(method, "get_viewport");
        assert_eq!(params["max_dimension"], 1280);
        seen.fetch_add(1, Ordering::SeqCst);
        Ok(json!({"metadata": {
            "identity": {"project_path":"/game", "process_id":42, "run_id":"run_42"},
            "readback_physics_frame":100, "frames_drawn":95, "timestamp_ms":2000,
            "status":"available", "width":1280,"height":720,"source_width":1920,"source_height":1080
        }, "image_base64":"jpeg-data"}))
    }))
    .await;
    for max_dimension in [0, 2049] {
        assert!(
            harness
                .server
                .viewport(Parameters(ViewportParams { max_dimension }))
                .await
                .is_err()
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let params = || serde_json::from_value(json!({})).unwrap();
    let result = harness.server.viewport(Parameters(params())).await.unwrap();
    let metadata = result.structured_content.unwrap();
    assert_eq!(metadata["readback_physics_frame"], 100);
    assert_eq!(metadata["frames_drawn"], 95);
    assert_eq!(metadata["identity"]["run_id"], "run_42");
    assert!(
        matches!(&result.content[1].raw, RawContent::Image(image) if image.mime_type == "image/jpeg" && image.data == "jpeg-data")
    );
    let cli: serde_json::Value =
        serde_json::from_str(&handle_viewport_cli(params(), &harness.state).await.unwrap())
            .unwrap();
    assert_eq!(cli["image_base64"], "jpeg-data");
    assert_eq!(cli["mime_type"], "image/jpeg");
    assert_eq!(cli["identity"], metadata["identity"]);
    let router = stage_server::server::StageServer::router_with_schemas();
    let tool = &router.map["viewport"].attr;
    assert!(tool.output_schema.is_some());
    let schema = serde_json::to_value(&tool.input_schema).unwrap();
    assert_eq!(schema["properties"]["max_dimension"]["maximum"], 2048);
    assert!(serde_json::from_value::<ViewportParams>(json!({"quality":99})).is_err());
}
