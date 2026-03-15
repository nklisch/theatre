use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use console::style;
use dialoguer::{Confirm, Input};

use crate::paths::TheatrePaths;
use crate::project::{generate_mcp_json, validate_project, write_mcp_json};

#[derive(Args)]
pub struct McpArgs {
    /// Godot project path (default: current directory)
    #[arg(default_value = ".")]
    project: PathBuf,

    /// Skip interactive prompts, use defaults (port 9077, overwrite existing)
    #[arg(long, short = 'y')]
    yes: bool,

    /// Stage/Director port (default: 9077)
    #[arg(long, default_value = "9077")]
    port: u16,
}

pub fn run(args: McpArgs) -> Result<()> {
    eprintln!("{}", style("Theatre — Generate MCP Config").bold());
    eprintln!();

    let theatre = TheatrePaths::resolve()?;
    theatre.validate_installed().map_err(|e| {
        anyhow::anyhow!("Theatre is not installed. Run `theatre install` first.\n\nDetails: {e}")
    })?;

    validate_project(&args.project)?;

    let stage_exists = args.project.join("addons").join("stage").exists();
    let director_exists = args.project.join("addons").join("director").exists();
    let mcp_json_exists = args.project.join(".mcp.json").exists();

    if !stage_exists && !director_exists {
        eprintln!(
            "  {} Neither addons/stage/ nor addons/director/ found in project.",
            style("⚠").yellow()
        );
        eprintln!(
            "       Run `theatre init {}` to install addons first.",
            args.project.display()
        );
        eprintln!("       Generating .mcp.json anyway.");
        eprintln!();
    }

    let (port, overwrite) = if args.yes {
        (args.port, true)
    } else {
        gather_options(mcp_json_exists, args.port)?
    };

    let stage_bin = theatre.bin_dir.join("stage");
    let director_bin = theatre.bin_dir.join("director");

    let port_opt = Some(port);
    let mcp = generate_mcp_json(
        &stage_bin,
        &director_bin,
        stage_exists || !director_exists, // include stage if installed, or if neither (generate both as fallback)
        director_exists || !stage_exists,
        port_opt,
    );

    let written =
        write_mcp_json(&args.project, &mcp, overwrite).context("Failed to write .mcp.json")?;

    if written {
        eprintln!(
            "  {} Generated .mcp.json at {}",
            style("✓").green(),
            args.project.join(".mcp.json").display()
        );
    } else {
        eprintln!(
            "  {} .mcp.json already exists — skipped (use --yes to overwrite)",
            style("⚠").yellow()
        );
    }

    eprintln!();
    eprintln!("Restart your AI agent to pick up the new MCP servers.");

    Ok(())
}

fn gather_options(mcp_json_exists: bool, default_port: u16) -> Result<(u16, bool)> {
    let port_str: String = Input::new()
        .with_prompt("Port")
        .default(default_port.to_string())
        .interact_text()
        .context("Port input cancelled")?;
    let port: u16 = port_str
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid port number: {port_str}"))?;
    if port < 1024 {
        anyhow::bail!("Port {port} is a privileged port (< 1024). Choose a port >= 1024.");
    }

    let overwrite = if mcp_json_exists {
        Confirm::new()
            .with_prompt(".mcp.json already exists. Overwrite?")
            .default(true)
            .interact()
            .context("Overwrite prompt cancelled")?
    } else {
        true
    };

    Ok((port, overwrite))
}
