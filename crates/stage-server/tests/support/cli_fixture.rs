#![allow(dead_code)]
/// CLI test fixture: shells out to the `stage` binary against a live Godot process.
use serde_json::Value;
use std::process::Command;
use tokio::time::Duration;

use super::godot_process::GodotProcess;

/// Result of a single CLI invocation.
pub enum CliResult {
    Ok(Value),
    Err { exit_code: i32, error: Value },
}

impl CliResult {
    /// Returns true if the invocation succeeded (exit code 0).
    pub fn is_ok(&self) -> bool {
        matches!(self, CliResult::Ok(_))
    }

    /// Returns true if the invocation failed (non-zero exit code).
    pub fn is_err(&self) -> bool {
        !self.is_ok()
    }

    /// Unwrap the success value, panicking with details on failure.
    pub fn unwrap_data(self) -> Value {
        match self {
            CliResult::Ok(v) => v,
            CliResult::Err { exit_code, error } => {
                panic!("Expected CLI Ok but got Err (exit {exit_code}): {error}")
            }
        }
    }

    /// Unwrap the error, panicking with details on success.
    pub fn unwrap_err(self) -> (i32, Value) {
        match self {
            CliResult::Err { exit_code, error } => (exit_code, error),
            CliResult::Ok(v) => panic!("Expected CLI Err but got Ok: {v}"),
        }
    }
}

/// Fixture that manages a live Godot process and shells out to the stage binary.
pub struct StageCliFixture {
    godot: GodotProcess,
}

impl StageCliFixture {
    /// Launch Godot with the 3D test scene.
    pub async fn start_3d() -> anyhow::Result<Self> {
        let godot = GodotProcess::start_3d().await?;
        Ok(Self { godot })
    }

    /// Launch Godot with the 2D test scene.
    pub async fn start_2d() -> anyhow::Result<Self> {
        let godot = GodotProcess::start_2d().await?;
        Ok(Self { godot })
    }

    /// The port the Godot addon is listening on.
    pub fn port(&self) -> u16 {
        self.godot.port()
    }

    /// Captured Godot stderr output (for debugging on test failure).
    pub fn godot_stderr(&self) -> String {
        self.godot.stderr_output()
    }

    /// Invoke `stage <tool> '<json>'` as a subprocess.
    ///
    /// Sets `THEATRE_PORT` so the CLI connects to this fixture's Godot instance.
    /// Parses stdout as JSON and returns a `CliResult` based on exit code.
    pub fn run(&self, tool: &str, params: Value) -> anyhow::Result<CliResult> {
        let bin = env!("CARGO_BIN_EXE_stage");
        let params_str = params.to_string();
        let port = self.godot.port().to_string();

        let output = Command::new(bin)
            .args([tool, &params_str])
            .env("THEATRE_PORT", &port)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to spawn stage binary: {e}"))?;

        let exit_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&output.stdout);

        let json: Value = serde_json::from_str(stdout.trim()).map_err(|e| {
            anyhow::anyhow!(
                "stage output is not valid JSON (exit {exit_code}): {e}\nstdout: {stdout}"
            )
        })?;

        if exit_code == 0 {
            Ok(CliResult::Ok(json))
        } else {
            Ok(CliResult::Err {
                exit_code,
                error: json,
            })
        }
    }

    /// Wait for N physics frames to elapse.
    /// Formula: ms = (n * 1000) / 60 + 50
    pub async fn wait_frames(&self, n: u32) {
        let ms = (n as u64 * 1000) / 60 + 50;
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }
}
