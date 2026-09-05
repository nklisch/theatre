use std::sync::Arc;

use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Serialize;
use stage_protocol::mcp_helpers::{deserialize_response, serialize_response};
use stage_protocol::runtime::{RuntimeIdentity, RuntimeStatus};
use tokio::sync::Mutex;

use crate::mcp::responses::BudgetBlock;
use crate::tcp::{SessionState, query_addon};

pub use stage_protocol::runtime::RuntimeStatusParams;

#[derive(Debug, Serialize, JsonSchema)]
pub struct RuntimeStatusResponse {
    pub connected: bool,
    /// True only when a fresh engine query confirms a ready current scene.
    pub ready: bool,
    /// Current verified connection identity; absent when disconnected.
    pub identity: Option<RuntimeIdentity>,
    /// Client connection session, distinct from the engine-owned run_id.
    pub session_id: Option<String>,
    pub current_scene: Option<String>,
    /// Actionable connection failure or reason readiness could not be confirmed.
    pub diagnostic: Option<String>,
    pub budget: BudgetBlock,
}

pub async fn handle_runtime_status(
    _params: RuntimeStatusParams,
    state: &Arc<Mutex<SessionState>>,
) -> Result<String, McpError> {
    let (session_id, connected) = {
        let s = state.lock().await;
        (s.session_id.clone(), s.connected)
    };
    let fresh: Option<Result<RuntimeStatus, McpError>> = if connected {
        Some(
            query_addon(state, "runtime_status", serde_json::json!({}))
                .await
                .and_then(deserialize_response),
        )
    } else {
        None
    };
    let s = state.lock().await;
    let same_connection = s.connected && s.session_id == session_id;
    let identity = if same_connection {
        s.handshake_info.as_ref().map(|h| h.identity.clone())
    } else {
        None
    };
    let mut response = RuntimeStatusResponse {
        connected: same_connection,
        ready: false,
        identity,
        session_id: if same_connection {
            s.session_id.clone()
        } else {
            None
        },
        current_scene: None,
        diagnostic: s.connection_error.clone(),
        budget: BudgetBlock {
            used: 0,
            limit: 500.min(s.config.token_hard_cap),
            hard_cap: s.config.token_hard_cap,
        },
    };
    drop(s);
    if same_connection {
        match fresh {
            Some(Ok(status)) if response.identity.as_ref() == Some(&status.identity) => {
                response.ready = status.ready;
                response.current_scene = status.current_scene;
            }
            Some(Ok(_)) => {
                return Err(McpError::internal_error(
                    "Runtime status identity differs from the verified handshake; reconnect to the intended game.",
                    None,
                ));
            }
            Some(Err(error)) => response.diagnostic = Some(error.message.to_string()),
            None => {}
        }
    }
    response.budget.used =
        stage_core::budget::estimate_tokens(serialize_response(&response)?.len());
    serialize_response(&response)
}
