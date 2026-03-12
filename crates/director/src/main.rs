use std::sync::Arc;

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};

use director::backend::Backend;
use director::resolve::{resolve_godot_bin, validate_project_path};

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // `director serve` — MCP server on stdio
    // `director <operation> '<json>'` — direct CLI invocation
    if args.len() >= 2 && args[1] == "serve" {
        return serve().await;
    }

    if args.len() >= 3 {
        return cli(&args[1], &args[2]).await;
    }

    eprintln!("Usage:");
    eprintln!("  director serve                    — MCP server (stdio)");
    eprintln!("  director <operation> '<json>'      — direct CLI invocation");
    std::process::exit(1);
}

async fn serve() -> Result<()> {
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

async fn cli(operation: &str, json_str: &str) -> Result<()> {
    let params: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| anyhow::anyhow!("Invalid JSON: {e}"))?;

    let project_path = params
        .get("project_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("params must include \"project_path\""))?;

    let godot = resolve_godot_bin()?;
    let project = std::path::Path::new(project_path);
    validate_project_path(project)?;

    let backend = Backend::new();
    let result = backend
        .run_operation(&godot, project, operation, &params)
        .await?;

    backend.shutdown().await;

    // Print result JSON to stdout (not MCP — direct output).
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
