use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FeedbackParams {
    /// Selected Godot project. No engine connection or backend launch is needed.
    pub project_path: String,
    #[serde(flatten)]
    pub operation: theatre_feedback::Operation,
}
