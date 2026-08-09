pub mod assertions;
pub mod backend;
pub mod cli_backend;
pub mod dispatch;
pub mod godot_process;
pub mod macros;
pub mod mcp_backend;

pub use backend::{LiveBackend, ToolResult};
pub use cli_backend::CliBackend;
pub use godot_process::LiveGodotProcess;
pub use mcp_backend::McpBackend;

/// Resolve a binary in the workspace target dir, honoring a redirected
/// CARGO_TARGET_DIR (env var or cargo config `build.target-dir`, discovered
/// via `cargo metadata`). Falls back to `<repo>/target/debug/<name>`.
pub fn workspace_binary(name: &str) -> std::path::PathBuf {
    use std::path::{Path, PathBuf};
    let filename = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(dir).join("debug").join(&filename);
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    if let Ok(output) = std::process::Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(&root)
        .output()
        && output.status.success()
        && let Ok(json) = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        && let Some(dir) = json.get("target_directory").and_then(|v| v.as_str())
    {
        return PathBuf::from(dir).join("debug").join(&filename);
    }
    root.join("target").join("debug").join(filename)
}
