mod cli;

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use stage_server::{config, server::StageServer, tcp};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Default TCP port for connecting to the Godot addon.
const DEFAULT_PORT: u16 = 9077;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // `stage serve` — MCP server on stdio
    if args.len() >= 2 && args[1] == "serve" {
        return serve().await;
    }

    // `stage --version` / `-V` — print version JSON to stdout
    if args.len() >= 2 && (args[1] == "--version" || args[1] == "-V") {
        println!("{{\"version\": \"{}\"}}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // `stage <tool> ['<json>']` — CLI one-shot mode
    if args.len() >= 2 && !args[1].starts_with('-') {
        let tool = &args[1];
        let json_arg = args.get(2).map(|s| s.as_str());
        return cli::run(tool, json_arg).await;
    }

    // No recognized command — print usage to stderr
    eprintln!("Usage:");
    eprintln!("  stage serve                      — MCP server (stdio)");
    eprintln!("  stage <tool> '<json>'             — one-shot CLI invocation");
    eprintln!("  stage --version                  — print version JSON");
    eprintln!();
    eprintln!("Tools:");
    for tool in cli::TOOLS {
        eprintln!("  {tool}");
    }
    std::process::exit(1);
}

async fn serve() -> Result<()> {
    // Initialize tracing to stderr (stdout is MCP protocol only)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive("stage=info".parse()?),
        )
        .init();

    tracing::info!("stage v{}", env!("CARGO_PKG_VERSION"));

    // Parse port from env or use default
    let env_port: u16 = std::env::var("THEATRE_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    // Determine project directory for stage.toml lookup
    let project_dir = std::env::var("THEATRE_PROJECT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

    // Load TOML config (port override + session config defaults)
    let toml_port = config::load_toml_port(&project_dir);
    let port = toml_port.unwrap_or(env_port);
    let base_config = config::load_toml_config(&project_dir);

    // Shared state between MCP handlers and TCP client
    let state = Arc::new(Mutex::new(tcp::SessionState {
        config: base_config,
        ..Default::default()
    }));

    // Spawn TCP client background task (reconnects automatically)
    let tcp_state = state.clone();
    tokio::spawn(async move {
        tcp::tcp_client_loop(tcp_state, port).await;
    });

    // Start MCP server on stdio — blocks until AI client disconnects
    let server = StageServer::new(state);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;

    tracing::info!("MCP session ended, shutting down");
    Ok(())
}
