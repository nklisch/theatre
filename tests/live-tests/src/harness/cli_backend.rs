#![allow(dead_code)]
use super::backend::{LiveBackend, ToolResult};
use super::godot_process::LiveGodotProcess;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;
use tokio::time::Duration;

pub struct CliBackend {
    godot: LiveGodotProcess,
    project_dir: PathBuf,
}

impl CliBackend {
    pub async fn start(scene: &str) -> anyhow::Result<Self> {
        let godot = LiveGodotProcess::start(scene).await?;
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project_dir = manifest_dir
            .join("../../tests/godot-project")
            .canonicalize()?;
        Ok(Self { godot, project_dir })
    }
}

impl LiveBackend for CliBackend {
    async fn stage(&self, tool: &str, params: Value) -> anyhow::Result<ToolResult> {
        // Find the stage binary in the workspace target directory.
        let bin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/stage");
        let params_str = params.to_string();
        let port = self.godot.port().to_string();

        let output = Command::new(&bin)
            .args([tool, &params_str])
            .env("THEATRE_PORT", &port)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to spawn stage binary: {e}"))?;

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout);

        if stdout.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "stage CLI returned empty stdout (exit {exit_code})\nstderr: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let json: Value = serde_json::from_str(stdout.trim()).map_err(|e| {
            anyhow::anyhow!(
                "stage output is not valid JSON (exit {exit_code}): {e}\nstdout: {stdout}"
            )
        })?;

        if exit_code == 0 {
            Ok(ToolResult::Ok(json))
        } else {
            Ok(ToolResult::Err {
                code: json["error"].as_str().unwrap_or("unknown").to_string(),
                message: json["message"]
                    .as_str()
                    .unwrap_or(&json.to_string())
                    .to_string(),
            })
        }
    }

    async fn director(&self, operation: &str, params: Value) -> anyhow::Result<ToolResult> {
        let bin =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/director");

        let mut params = params;
        if let Value::Object(ref mut map) = params {
            map.entry("project_path").or_insert_with(|| {
                Value::String(self.project_dir.to_string_lossy().into())
            });
        }

        let output = Command::new(&bin)
            .args([operation, &params.to_string()])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to spawn director binary: {e}"))?;

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout);

        if stdout.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "director CLI returned empty stdout (exit {exit_code})\nstderr: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let json: Value = serde_json::from_str(stdout.trim()).map_err(|e| {
            anyhow::anyhow!("director output is not valid JSON: {e}\nstdout: {stdout}")
        })?;

        // Director CLI exits 0 even on operation failure — check success field
        // Unwrap the "data" envelope so callers get the payload directly
        if exit_code == 0 && json["success"].as_bool() != Some(false) {
            let data = json.get("data").cloned().unwrap_or(json.clone());
            Ok(ToolResult::Ok(data))
        } else {
            Ok(ToolResult::Err {
                code: "director_error".to_string(),
                message: json["error"]
                    .as_str()
                    .unwrap_or(&json.to_string())
                    .to_string(),
            })
        }
    }

    async fn wait_frames(&self, n: u32) {
        let ms = (n as u64 * 1000) / 60 + 50;
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    fn is_stateful(&self) -> bool {
        false
    }
}
