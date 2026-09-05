use std::io::IsTerminal as _;
use std::sync::Arc;

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};

use director::backend::Backend;
use director::resolve::{resolve_godot_bin, validate_project_path};

#[tokio::main]
async fn main() -> Result<()> {
    let mut args_os = std::env::args_os();
    let _executable = args_os.next();
    if args_os.next().as_deref() == Some(std::ffi::OsStr::new("__process-supervisor")) {
        std::process::exit(director::process::run_supervisor(args_os)?);
    }

    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        Some("serve") => serve().await,
        Some("--help") | Some("-h") | None => {
            print_usage();
            Ok(())
        }
        Some("--version") | Some("-V") => {
            println!("{{\"version\": \"{}\"}}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some(operation) => {
            let json_arg = args.get(2).map(|s| s.as_str());
            cli(operation, json_arg).await
        }
    }
}

fn print_usage() {
    let version = env!("CARGO_PKG_VERSION");
    eprintln!("director v{version} — Scene and resource authoring for Godot");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  director serve                      — MCP server (stdio)");
    eprintln!("  director <tool> '<json-params>'      — CLI one-shot mode");
    eprintln!("  echo '<json>' | director <tool>      — read params from stdin");
    eprintln!();
    eprintln!("Scene Tools:");
    eprintln!("  scene_create       Create a new scene file");
    eprintln!(
        "  scene_save         Save selected scene only; retain undo and external resource edits"
    );
    eprintln!("  scene_read         Read scene node tree");
    eprintln!("  scene_list         List scene files in project");
    eprintln!("  scene_diff         Compare two scene files");
    eprintln!("  scene_add_instance Add a scene instance as child");
    eprintln!();
    eprintln!("Node Tools:");
    eprintln!("  node_add           Add a node to a scene");
    eprintln!("  node_remove        Remove a node from a scene");
    eprintln!("  node_set_properties Set node properties");
    eprintln!("  node_reparent      Move a node to a new parent");
    eprintln!("  node_find          Search for nodes");
    eprintln!("  node_set_groups    Add/remove node groups");
    eprintln!("  node_set_script    Attach/detach scripts");
    eprintln!("  node_set_meta      Set/remove node metadata");
    eprintln!();
    eprintln!("Resource Tools:");
    eprintln!("  resource_read      Read a resource file");
    eprintln!("  resource_duplicate Duplicate a resource");
    eprintln!("  material_create    Create a material resource");
    eprintln!("  shape_create       Create a collision shape");
    eprintln!("  style_box_create   Create a StyleBox resource");
    eprintln!();
    eprintln!("TileMap Tools:");
    eprintln!("  tilemap_set_cells  Set cells on a TileMapLayer");
    eprintln!("  tilemap_get_cells  Read cells from a TileMapLayer");
    eprintln!("  tilemap_clear      Clear TileMapLayer cells");
    eprintln!();
    eprintln!("GridMap Tools:");
    eprintln!("  gridmap_set_cells  Set cells in a GridMap");
    eprintln!("  gridmap_get_cells  Read cells from a GridMap");
    eprintln!("  gridmap_clear      Clear GridMap cells");
    eprintln!();
    eprintln!("Animation Tools:");
    eprintln!("  animation_create   Create an animation resource");
    eprintln!("  animation_add_track Add a track to an animation");
    eprintln!("  animation_read     Read an animation resource");
    eprintln!("  animation_remove_track Remove an animation track");
    eprintln!();
    eprintln!("Physics Tools:");
    eprintln!("  physics_set_layers Set collision layers/masks");
    eprintln!("  physics_set_layer_names Name physics layers");
    eprintln!();
    eprintln!("Signal Tools:");
    eprintln!("  signal_connect     Connect a signal");
    eprintln!("  signal_disconnect  Disconnect a signal");
    eprintln!("  signal_list        List signal connections");
    eprintln!();
    eprintln!("Engine API Tools:");
    eprintln!("  engine_api         Query installed Godot ClassDB metadata");
    eprintln!();
    eprintln!("Editor Tools:");
    eprintln!("  editor_run         Start, stop, restart, or inspect a saved-scene run");
    eprintln!();
    eprintln!("Other Tools:");
    eprintln!("  feedback           List, retrieve, handle or delete project-local human feedback");
    eprintln!("  visual_shader_create Create a visual shader");
    eprintln!("  export_mesh_library  Export MeshLibrary from scene");
    eprintln!("  uid_get            Resolve a file's Godot UID");
    eprintln!("  uid_update_project Scan and register missing UIDs");
    eprintln!("  batch              Execute multiple operations");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --help, -h         Print this help");
    eprintln!("  --version, -V      Print version");
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

async fn cli(operation: &str, json_arg: Option<&str>) -> Result<()> {
    // Parse params from argument or stdin
    let params: serde_json::Value = if let Some(json_str) = json_arg {
        serde_json::from_str(json_str).unwrap_or_else(|e| {
            let error = serde_json::json!({
                "error": "invalid_json",
                "message": format!("Invalid JSON: {e}"),
            });
            println!("{}", serde_json::to_string(&error).unwrap_or_default());
            std::process::exit(2);
        })
    } else if !std::io::stdin().is_terminal() {
        // Stdin is piped — read from it
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
            .expect("Failed to read stdin");
        let trimmed = buf.trim();
        if trimmed.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(trimmed).unwrap_or_else(|e| {
                let error = serde_json::json!({
                    "error": "invalid_json",
                    "message": format!("Invalid JSON from stdin: {e}"),
                });
                println!("{}", serde_json::to_string(&error).unwrap_or_default());
                std::process::exit(2);
            })
        }
    } else {
        // No JSON provided and stdin is a terminal
        let error = serde_json::json!({
            "error": "missing_params",
            "message": format!("No JSON params provided. Usage: director {operation} '<json>'"),
        });
        println!("{}", serde_json::to_string(&error)?);
        std::process::exit(2);
    };

    if operation == "feedback" {
        let project = params
            .get("project_path")
            .and_then(|p| p.as_str())
            .map(std::path::PathBuf::from);
        let parsed = serde_json::from_value::<director::mcp::feedback::FeedbackParams>(params);
        let result = parsed.map_err(|e| e.to_string()).and_then(|params| {
            theatre_feedback::Queue::open(std::path::Path::new(&params.project_path))
                .and_then(|queue| queue.execute(params.operation))
                .map_err(|e| e.to_string())
        });
        let (mut value, code) = match result {
            Ok(response) => (serde_json::to_value(response)?, 0),
            Err(message) => (
                serde_json::json!({"error": "feedback_error", "message": message}),
                1,
            ),
        };
        if let Some(project) = project {
            theatre_feedback::append_json_notice(&mut value, &project);
        }
        println!("{value}");
        std::process::exit(code);
    }

    let project_path = match params.get("project_path").and_then(|v| v.as_str()) {
        Some(p) => p.to_owned(),
        None => {
            let error = serde_json::json!({
                "error": "missing_project_path",
                "message": "params must include \"project_path\"",
            });
            println!("{}", serde_json::to_string(&error)?);
            std::process::exit(2);
        }
    };

    if operation == "editor_run" {
        let project = std::path::Path::new(&project_path);
        let typed: director::mcp::editor_run::EditorRunParams = match serde_json::from_value(params)
        {
            Ok(params) => params,
            Err(error) => {
                let mut response = serde_json::json!({
                    "error": "invalid_parameters",
                    "message": error.to_string(),
                });
                theatre_feedback::append_json_notice(&mut response, project);
                println!("{}", serde_json::to_string(&response)?);
                std::process::exit(2);
            }
        };
        let backend = Backend::new();
        match director::mcp::editor_run::run_editor(&backend, &typed).await {
            Ok(data) => {
                let mut response = serde_json::json!({
                    "success": true,
                    "data": data,
                });
                theatre_feedback::append_json_notice(&mut response, project);
                println!("{}", serde_json::to_string(&response)?);
                backend.shutdown().await;
                return Ok(());
            }
            Err(error) => {
                let mut response = serde_json::json!({
                    "error": "operation_failed",
                    "message": error.message,
                });
                theatre_feedback::append_json_notice(&mut response, project);
                println!("{}", serde_json::to_string(&response)?);
                backend.shutdown().await;
                std::process::exit(1);
            }
        }
    }

    let godot = match resolve_godot_bin() {
        Ok(g) => g,
        Err(e) => {
            let mut error = serde_json::json!({
                "error": "godot_not_found",
                "message": e.to_string(),
                "hint": "Ensure Godot is installed and in your PATH.",
            });
            theatre_feedback::append_json_notice(&mut error, std::path::Path::new(&project_path));
            println!("{}", serde_json::to_string(&error)?);
            std::process::exit(1);
        }
    };

    let project = std::path::Path::new(&project_path);
    if let Err(e) = validate_project_path(project) {
        let error = serde_json::json!({
            "error": "invalid_project",
            "message": e.to_string(),
        });
        println!("{}", serde_json::to_string(&error)?);
        std::process::exit(1);
    }

    let backend = Backend::new();
    let result = match backend
        .run_operation(&godot, project, operation, &params)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let mut error = serde_json::json!({
                "error": "operation_failed",
                "message": e.to_string(),
            });
            theatre_feedback::append_json_notice(&mut error, project);
            println!("{}", serde_json::to_string(&error)?);
            backend.shutdown().await;
            std::process::exit(1);
        }
    };

    backend.shutdown().await;

    // Keep operation data/envelope intact; queue availability never changes success.
    let mut value = serde_json::to_value(&result)?;
    theatre_feedback::append_json_notice(&mut value, project);
    println!("{value}");
    Ok(())
}
