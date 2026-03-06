mod mcp;
mod server;
mod tcp;

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use std::sync::Arc;
use tokio::sync::Mutex;

use server::SpectatorServer;
use tcp::SessionState;

/// Default TCP port for connecting to the Godot addon.
const DEFAULT_PORT: u16 = 9077;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing to stderr (stdout is MCP protocol only)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("spectator=info".parse()?),
        )
        .init();

    tracing::info!("spectator-server v{}", env!("CARGO_PKG_VERSION"));

    // Parse port from env or use default
    let port: u16 = std::env::var("SPECTATOR_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    // Shared state between MCP handlers and TCP client
    let state = Arc::new(Mutex::new(SessionState::default()));

    // Spawn TCP client background task (reconnects automatically)
    let tcp_state = state.clone();
    tokio::spawn(async move {
        tcp::tcp_client_loop(tcp_state, port).await;
    });

    // Start MCP server on stdio — blocks until AI client disconnects
    let server = SpectatorServer::new(state);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    tracing::info!("MCP session ended, shutting down");
    Ok(())
}
