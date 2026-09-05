use std::sync::Arc;

use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use stage_protocol::runtime::RuntimeIdentity;
use stage_protocol::runtime_diagnostics::{
    RuntimeDiagnostic, RuntimeDiagnosticLimits, RuntimeDiagnosticsEngineResponse,
    RuntimeDiagnosticsQueryParams,
};
use tokio::sync::Mutex;

use crate::mcp::responses::BudgetBlock;
use crate::mcp::{budget_context, finalize_response, query_and_deserialize};
use crate::tcp::SessionState;

const DEFAULT_MAX_ENTRIES: u32 = 20;
const ENGINE_QUEUE_CAPACITY: u32 = 128;
const DEFAULT_TOKEN_BUDGET: u32 = 1500;

fn default_max_entries() -> u32 {
    DEFAULT_MAX_ENTRIES
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeDiagnosticsParams {
    /// Maximum diagnostics to return, newest first. Range: 1-128. Default: 20.
    #[serde(default = "default_max_entries")]
    pub max_entries: u32,
    /// Return diagnostics older than this process-local sequence number.
    pub before_sequence: Option<u64>,
    /// Approximate response token budget, capped by the session hard cap.
    pub token_budget: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RuntimeDiagnosticsResponse {
    pub identity: RuntimeIdentity,
    pub available: bool,
    pub diagnostics: Vec<RuntimeDiagnostic>,
    /// Diagnostics currently retained by the process-local logger.
    pub retained_count: u32,
    /// Diagnostics evicted from the bounded logger queue since registration.
    pub omitted_count: u64,
    /// Retained diagnostics eligible after applying before_sequence.
    pub eligible_count: u32,
    pub returned_count: u32,
    /// Eligible diagnostics omitted by max_entries or the response budget.
    pub response_omitted_count: u32,
    /// Pass as before_sequence to continue toward older retained diagnostics.
    pub next_before_sequence: Option<u64>,
    pub limits: RuntimeDiagnosticLimits,
    pub limitations: Vec<String>,
    pub budget: BudgetBlock,
}

pub async fn handle_runtime_diagnostics(
    params: RuntimeDiagnosticsParams,
    state: &Arc<Mutex<SessionState>>,
) -> Result<String, McpError> {
    if !(1..=ENGINE_QUEUE_CAPACITY).contains(&params.max_entries) {
        return Err(McpError::invalid_params(
            format!("max_entries must be between 1 and {ENGINE_QUEUE_CAPACITY}"),
            None,
        ));
    }

    let session_id = {
        let session = state.lock().await;
        if !session.connected {
            let detail = session
                .connection_error
                .as_deref()
                .unwrap_or("No running game has connected.");
            return Err(McpError::internal_error(
                format!(
                    "Runtime diagnostics are unavailable because Stage is not connected to a running game: {detail}"
                ),
                None,
            ));
        }
        session.session_id.clone()
    };

    let engine: RuntimeDiagnosticsEngineResponse = query_and_deserialize(
        state,
        "runtime_diagnostics",
        &RuntimeDiagnosticsQueryParams {},
    )
    .await?;

    {
        let session = state.lock().await;
        let current_identity = session.handshake_info.as_ref().map(|info| &info.identity);
        if !session.connected
            || session.session_id != session_id
            || current_identity != Some(&engine.identity)
        {
            return Err(McpError::internal_error(
                "The running game changed while diagnostics were being read; retry against the current runtime.",
                None,
            ));
        }
    }

    let bctx = budget_context(state).await;
    let budget_limit = bctx.resolve(params.token_budget, DEFAULT_TOKEN_BUDGET);
    let mut eligible: Vec<RuntimeDiagnostic> = engine
        .diagnostics
        .into_iter()
        .rev()
        .filter(|diagnostic| {
            params
                .before_sequence
                .is_none_or(|before| diagnostic.sequence < before)
        })
        .collect();
    let eligible_count = eligible.len() as u32;
    eligible.truncate(params.max_entries as usize);

    let mut response = RuntimeDiagnosticsResponse {
        identity: engine.identity,
        available: engine.available,
        diagnostics: Vec::new(),
        retained_count: engine.retained_count,
        omitted_count: engine.omitted_count,
        eligible_count,
        returned_count: 0,
        response_omitted_count: eligible_count,
        next_before_sequence: None,
        limits: engine.limits,
        limitations: engine.limitations,
        budget: BudgetBlock {
            used: 0,
            limit: budget_limit,
            hard_cap: bctx.hard_cap,
        },
    };

    // The queue is already bounded at 128 entries, so measuring each candidate
    // against the actual response shape is simpler and more truthful than a
    // second diagnostic-specific budgeting abstraction.
    for diagnostic in eligible {
        response.diagnostics.push(diagnostic);
        response.returned_count = response.diagnostics.len() as u32;
        response.response_omitted_count = eligible_count - response.returned_count;
        let encoded_len = serde_json::to_vec(&response)
            .map_err(|error| {
                McpError::internal_error(format!("Serialization error: {error}"), None)
            })?
            .len();
        let required_tokens = stage_core::budget::estimate_tokens(encoded_len);
        if required_tokens > budget_limit {
            let oversized_sequence = response.diagnostics.last().map(|entry| entry.sequence);
            response.diagnostics.pop();
            response.returned_count = response.diagnostics.len() as u32;
            response.response_omitted_count = eligible_count - response.returned_count;
            if response.diagnostics.is_empty() {
                // Returning an empty page with no cursor strands the caller at
                // the newest retained entry. Refuse with a concrete recovery
                // instead; the engine-owned queue remains untouched.
                let required_budget = required_tokens.saturating_add(32);
                return Err(McpError::invalid_params(
                    format!(
                        "The newest eligible diagnostic (sequence {}) requires a token_budget of at least {required_budget}, but the effective budget is {budget_limit}. Retry with token_budget >= {required_budget} and ensure the session token_hard_cap is also >= {required_budget}; retained diagnostics were not modified.",
                        oversized_sequence.unwrap_or_default()
                    ),
                    None,
                ));
            }
            break;
        }
    }
    if response.response_omitted_count > 0 {
        response.next_before_sequence = response.diagnostics.last().map(|entry| entry.sequence);
    }

    let mut value = serde_json::to_value(response)
        .map_err(|error| McpError::internal_error(format!("Serialization error: {error}"), None))?;
    let encoded = finalize_response(&mut value, budget_limit, bctx.hard_cap)?;
    let required_tokens = stage_core::budget::estimate_tokens(encoded.len());
    if required_tokens > budget_limit {
        // Even an empty page has identity, retention and capture-limit metadata.
        // Check the final envelope, not only candidates added inside the loop.
        let required_budget = required_tokens.saturating_add(32);
        return Err(McpError::invalid_params(
            format!(
                "This diagnostic page requires a token_budget of at least {required_budget}, but the effective budget is {budget_limit}. Retry with token_budget >= {required_budget} and ensure the session token_hard_cap is also >= {required_budget}; retained diagnostics were not modified."
            ),
            None,
        ));
    }
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_default_to_small_recent_page() {
        let params: RuntimeDiagnosticsParams =
            serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(params.max_entries, 20);
        assert!(params.before_sequence.is_none());
    }
}
