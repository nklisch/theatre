/// Generate identical test functions for both CLI and MCP backends.
///
/// The test body must be an `async fn` that takes `&impl LiveBackend`.
///
/// Usage:
///   async fn my_test_body(b: &impl LiveBackend) { ... }
///   dual_test!(my_test_name, "res://scene.tscn", my_test_body);
#[macro_export]
macro_rules! dual_test {
    ($name:ident, $scene:expr, $body:ident) => {
        mod $name {
            use super::*;

            #[tokio::test]
            #[ignore = "requires display and Godot binary"]
            async fn cli() {
                let backend = $crate::harness::CliBackend::start($scene)
                    .await
                    .expect("Failed to start live Godot (CLI backend)");
                $body(&backend).await;
            }

            #[tokio::test]
            #[ignore = "requires display and Godot binary"]
            async fn mcp() {
                let backend = $crate::harness::McpBackend::start($scene)
                    .await
                    .expect("Failed to start live Godot (MCP backend)");
                $body(&backend).await;
            }
        }
    };
}

/// Generate test functions for stateful (MCP) backend only.
/// The CLI variant emits a skip message via eprintln.
///
/// The test body must be an `async fn` that takes `&impl LiveBackend`.
///
/// Usage:
///   async fn my_test_body(b: &impl LiveBackend) { ... }
///   stateful_test!(my_test_name, "res://scene.tscn", my_test_body);
#[macro_export]
macro_rules! stateful_test {
    ($name:ident, $scene:expr, $body:ident) => {
        mod $name {
            use super::*;

            #[tokio::test]
            #[ignore = "requires display and Godot binary"]
            async fn cli() {
                eprintln!(
                    "SKIPPED: {} requires stateful (MCP) backend",
                    stringify!($name)
                );
            }

            #[tokio::test]
            #[ignore = "requires display and Godot binary"]
            async fn mcp() {
                let backend = $crate::harness::McpBackend::start($scene)
                    .await
                    .expect("Failed to start live Godot (MCP backend)");
                $body(&backend).await;
            }
        }
    };
}
