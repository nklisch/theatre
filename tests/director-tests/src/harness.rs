use std::path::{Path, PathBuf};
use std::process::Command;

/// A Director operation runner for E2E tests.
///
/// Spawns `godot --headless --path <project> --script addons/director/operations.gd
/// -- <op> '<json>'` and parses the JSON result from stdout.
pub struct DirectorFixture {
    godot_bin: String,
    project_dir: PathBuf,
}

/// Parsed operation result from GDScript stdout.
#[derive(Debug, serde::Deserialize)]
pub struct OperationResult {
    pub success: bool,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub operation: Option<String>,
    #[serde(default)]
    pub context: Option<serde_json::Value>,
}

impl OperationResult {
    pub fn unwrap_data(self) -> serde_json::Value {
        if !self.success {
            panic!(
                "Expected success, got error: {}",
                self.error.unwrap_or_else(|| "unknown".into())
            );
        }
        self.data
    }

    pub fn unwrap_err(self) -> String {
        if self.success {
            panic!("Expected error, got success: {:?}", self.data);
        }
        self.error.unwrap_or_else(|| "unknown error".into())
    }
}

impl DirectorFixture {
    pub fn new() -> Self {
        let godot_bin = std::env::var("GODOT_BIN").unwrap_or_else(|_| "godot".into());
        Self {
            godot_bin,
            project_dir: Self::project_dir(),
        }
    }

    /// Run a Director operation and return the parsed result.
    pub fn run(&self, operation: &str, params: serde_json::Value) -> anyhow::Result<OperationResult> {
        let output = Command::new(&self.godot_bin)
            .args([
                "--headless",
                "--path",
                &self.project_dir.to_string_lossy(),
                "--script",
                "addons/director/operations.gd",
                "--",
                operation,
                &params.to_string(),
            ])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to launch Godot ({}): {e}", self.godot_bin))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Parse the last non-empty line of stdout as JSON
        let json_line = stdout
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("No output from Godot.\nstderr: {stderr}")
            })?;

        serde_json::from_str(json_line).map_err(|e| {
            anyhow::anyhow!("Failed to parse JSON: {e}\nline: {json_line}\nfull stdout: {stdout}\nstderr: {stderr}")
        })
    }

    /// Create a temporary scene file path that won't conflict between tests.
    pub fn temp_scene_path(name: &str) -> String {
        format!("tmp/test_{name}.tscn")
    }

    fn project_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../godot-project")
            .canonicalize()
            .expect("tests/godot-project dir must exist")
    }
}

/// Assert two f64 values are approximately equal (within 0.01).
pub fn assert_approx(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 0.01,
        "expected ~{expected}, got {actual}"
    );
}
