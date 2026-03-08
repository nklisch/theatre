use std::sync::Arc;

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("director=info".parse()?),
        )
        .init();

    tracing::info!("director v{}", env!("CARGO_PKG_VERSION"));

    let server = director::server::DirectorServer::new();
    let backend = Arc::clone(&server.backend);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    backend.shutdown().await;
    tracing::info!("MCP session ended, shutting down");
    Ok(())
}
