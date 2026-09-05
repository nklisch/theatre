use serde::{Deserialize, Serialize};

use crate::runtime::RuntimeIdentity;

/// Internal engine query. Response shaping and token limits belong to the Stage server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeDiagnosticsQueryParams {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum RuntimeDiagnosticKind {
    Error,
    Warning,
    ScriptError,
    ShaderError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RuntimeDiagnosticOrigin {
    pub function: String,
    pub file: String,
    pub line: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RuntimeBacktraceFrame {
    pub file: String,
    pub function: String,
    pub line: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RuntimeDiagnostic {
    /// Monotonic within one game process; it is not a physics or render frame.
    pub sequence: u64,
    pub kind: RuntimeDiagnosticKind,
    pub message: String,
    pub origin: RuntimeDiagnosticOrigin,
    #[serde(default)]
    pub backtrace: Vec<RuntimeBacktraceFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RuntimeDiagnosticLimits {
    pub queue_capacity: u32,
    pub message_max_chars: u32,
    pub file_max_chars: u32,
    pub function_max_chars: u32,
    pub backtrace_max_frames: u32,
}

/// Data copied from the bounded GDScript logger under its mutex.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeDiagnosticCapture {
    pub entries: Vec<RuntimeDiagnostic>,
    pub retained_count: u32,
    /// Diagnostics evicted from the process-local queue since registration.
    pub omitted_count: u64,
    pub limits: RuntimeDiagnosticLimits,
}

/// Typed engine response before server-side pagination and response budgeting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeDiagnosticsEngineResponse {
    pub identity: RuntimeIdentity,
    pub available: bool,
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub retained_count: u32,
    pub omitted_count: u64,
    pub limits: RuntimeDiagnosticLimits,
    pub limitations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_kind_uses_wire_names() {
        assert_eq!(
            serde_json::to_string(&RuntimeDiagnosticKind::ScriptError).unwrap(),
            r#""script_error""#
        );
    }

    #[test]
    fn query_params_reject_unknown_fields() {
        assert!(
            serde_json::from_value::<RuntimeDiagnosticsQueryParams>(
                serde_json::json!({"limit": 1})
            )
            .is_err()
        );
    }
}
