use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use console::style;
use dialoguer::{Confirm, Input, MultiSelect};

use crate::paths::{TheatrePaths, copy_dir_recursive, gdext_filename, platform_dir};
use crate::project::{
    generate_mcp_json, remove_autoload, set_autoload, set_plugin_enabled, validate_project,
    write_mcp_json,
};

const SPECTATOR_PLUGIN_CFG: &str = "res://addons/spectator/plugin.cfg";
const DIRECTOR_PLUGIN_CFG: &str = "res://addons/director/plugin.cfg";
const SPECTATOR_RUNTIME_NAME: &str = "SpectatorRuntime";
const SPECTATOR_RUNTIME_SCRIPT: &str = "res://addons/spectator/runtime.gd";

#[derive(Args)]
pub struct InitArgs {
    /// Godot project path
    project: PathBuf,

    /// Skip interactive prompts, use defaults
    /// (both addons, both plugins, generate .mcp.json)
    #[arg(long, short = 'y')]
    yes: bool,
}

pub fn run(args: InitArgs) -> Result<()> {
    eprintln!("{}", style("Theatre — Project Setup").bold());
    eprintln!();

    // Step 1: Resolve and validate TheatrePaths
    let theatre = TheatrePaths::resolve()?;
    theatre.validate_installed().map_err(|e| {
        anyhow::anyhow!("Theatre is not installed. Run `theatre install` first.\n\nDetails: {e}")
    })?;

    // Step 2: Validate project path
    validate_project(&args.project)?;

    // Step 3: Check current state
    let spectator_exists = args.project.join("addons").join("spectator").exists();
    let director_exists = args.project.join("addons").join("director").exists();
    let mcp_json_exists = args.project.join(".mcp.json").exists();

    // Step 4 & 5: Determine selections
    let (do_spectator, do_director, do_mcp, port, enable_spectator, enable_director) = if args.yes {
        (true, true, true, 9077u16, true, true)
    } else {
        gather_interactive_selections(spectator_exists, director_exists, mcp_json_exists)?
    };

    // Check if nothing was selected
    if !do_spectator && !do_director && !do_mcp && !enable_spectator && !enable_director {
        eprintln!("Nothing selected. Exiting.");
        return Ok(());
    }

    eprintln!();

    // Step 6a: Copy addon directories
    if do_spectator {
        let src = theatre.addon_source().join("spectator");
        let dst = args.project.join("addons").join("spectator");

        if spectator_exists && !args.yes {
            // Was already prompted for overwrite in interactive mode
        }

        std::fs::create_dir_all(args.project.join("addons"))
            .context("Failed to create addons directory")?;
        copy_dir_recursive(&src, &dst, &|_| false).context("Failed to copy spectator addon")?;

        // Also copy GDExtension binary
        let gdext_src = theatre.gdext_binary();
        let gdext_dst_dir = dst.join("bin").join(platform_dir());
        std::fs::create_dir_all(&gdext_dst_dir)
            .context("Failed to create GDExtension bin dir in project")?;
        std::fs::copy(&gdext_src, gdext_dst_dir.join(gdext_filename()))
            .with_context(|| format!("Failed to copy GDExtension from {}", gdext_src.display()))?;

        eprintln!(
            "  {} Copied addons/spectator/ (with GDExtension)",
            style("✓").green()
        );
    }

    if do_director {
        let src = theatre.addon_source().join("director");
        let dst = args.project.join("addons").join("director");

        std::fs::create_dir_all(args.project.join("addons"))
            .context("Failed to create addons directory")?;
        copy_dir_recursive(&src, &dst, &|_| false).context("Failed to copy director addon")?;

        eprintln!("  {} Copied addons/director/", style("✓").green());
    }

    // Step 6b: Generate and write .mcp.json
    if do_mcp {
        let spectator_bin = theatre.bin_dir.join("spectator");
        let director_bin = theatre.bin_dir.join("director");

        if !spectator_bin.exists() {
            eprintln!(
                "  {} spectator not found at {} — generating .mcp.json anyway",
                style("⚠").yellow(),
                spectator_bin.display()
            );
        }
        if !director_bin.exists() {
            eprintln!(
                "  {} director not found at {} — generating .mcp.json anyway",
                style("⚠").yellow(),
                director_bin.display()
            );
        }

        let port_opt = if port == 9077 { Some(9077) } else { Some(port) };
        let mcp = generate_mcp_json(
            &spectator_bin,
            &director_bin,
            do_spectator,
            do_director,
            port_opt,
        );
        let overwrite = args.yes || !mcp_json_exists;
        let written = write_mcp_json(&args.project, &mcp, overwrite)?;
        if written {
            eprintln!("  {} Generated .mcp.json", style("✓").green());
        } else {
            eprintln!(
                "  {} .mcp.json already exists — skipped (use --yes to overwrite)",
                style("⚠").yellow()
            );
        }
    }

    // Step 6c: Enable plugins
    if enable_spectator {
        set_plugin_enabled(&args.project, SPECTATOR_PLUGIN_CFG, true)?;
        eprintln!(
            "  {} Enabled Spectator in project.godot",
            style("✓").green()
        );
        set_autoload(
            &args.project,
            SPECTATOR_RUNTIME_NAME,
            SPECTATOR_RUNTIME_SCRIPT,
        )?;
        eprintln!("  {} SpectatorRuntime autoload added", style("✓").green());
    } else {
        // If not enabling, ensure it's disabled
        remove_autoload(&args.project, SPECTATOR_RUNTIME_NAME)?;
    }

    if enable_director {
        set_plugin_enabled(&args.project, DIRECTOR_PLUGIN_CFG, true)?;
        eprintln!("  {} Enabled Director in project.godot", style("✓").green());
    }

    eprintln!();
    eprintln!("Done. Open your project in Godot — plugins are active.");

    Ok(())
}

/// Run the interactive TUI and return selections.
/// Returns (do_spectator, do_director, do_mcp, port, enable_spectator, enable_director).
fn gather_interactive_selections(
    spectator_exists: bool,
    director_exists: bool,
    mcp_json_exists: bool,
) -> Result<(bool, bool, bool, u16, bool, bool)> {
    // Addon selection
    let addon_items = vec![
        "Spectator — spatial awareness for AI agents",
        "Director — scene and resource authoring",
    ];
    let addon_defaults = vec![true, true];
    let addon_selections = MultiSelect::new()
        .with_prompt("Which addons to install?")
        .items(&addon_items)
        .defaults(&addon_defaults)
        .interact()
        .context("Addon selection cancelled")?;

    let do_spectator = addon_selections.contains(&0);
    let do_director = addon_selections.contains(&1);

    // Check overwrite if addons already exist
    if do_spectator && spectator_exists {
        let overwrite = Confirm::new()
            .with_prompt("addons/spectator/ already exists. Overwrite?")
            .default(true)
            .interact()
            .context("Overwrite prompt cancelled")?;
        if !overwrite {
            // User declined — treat as not installing spectator
            return gather_interactive_selections(false, director_exists, mcp_json_exists)
                .map(|(_, d, m, p, _, ed)| (false, d, m, p, false, ed));
        }
    }

    if do_director && director_exists {
        let overwrite = Confirm::new()
            .with_prompt("addons/director/ already exists. Overwrite?")
            .default(true)
            .interact()
            .context("Overwrite prompt cancelled")?;
        if !overwrite {
            return gather_interactive_selections(spectator_exists, false, mcp_json_exists)
                .map(|(s, _, m, p, es, _)| (s, false, m, p, es, false));
        }
    }

    // MCP config
    let do_mcp = Confirm::new()
        .with_prompt("Generate .mcp.json for AI agent configuration?")
        .default(true)
        .interact()
        .context("MCP config prompt cancelled")?;

    let port = if do_mcp {
        if mcp_json_exists {
            let overwrite = Confirm::new()
                .with_prompt(".mcp.json already exists. Overwrite?")
                .default(true)
                .interact()
                .context("Overwrite prompt cancelled")?;
            if !overwrite {
                // Skip MCP generation
                return gather_interactive_selections_no_mcp(do_spectator, do_director);
            }
        }

        let port_str: String = Input::new()
            .with_prompt("Port")
            .default("9077".to_string())
            .interact_text()
            .context("Port input cancelled")?;
        let port: u16 = port_str
            .trim()
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid port number: {port_str}"))?;
        if port < 1024 {
            anyhow::bail!("Port {port} is a privileged port (< 1024). Choose a port >= 1024.");
        }
        port
    } else {
        9077
    };

    // Plugin enable selection (only for selected addons)
    let mut plugin_items = vec![];
    let mut plugin_defaults = vec![];
    if do_spectator {
        plugin_items.push("Spectator");
        plugin_defaults.push(true);
    }
    if do_director {
        plugin_items.push("Director");
        plugin_defaults.push(true);
    }

    let (enable_spectator, enable_director) = if !plugin_items.is_empty() {
        let plugin_selections = MultiSelect::new()
            .with_prompt("Enable plugins in project.godot?")
            .items(&plugin_items)
            .defaults(&plugin_defaults)
            .interact()
            .context("Plugin selection cancelled")?;

        let enable_spectator = do_spectator && plugin_selections.contains(&0);
        let enable_director = if do_spectator {
            do_director && plugin_selections.contains(&1)
        } else {
            do_director && plugin_selections.contains(&0)
        };
        (enable_spectator, enable_director)
    } else {
        (false, false)
    };

    // Final confirm
    let proceed = Confirm::new()
        .with_prompt("Proceed with setup?")
        .default(true)
        .interact()
        .context("Confirmation cancelled")?;

    if !proceed {
        eprintln!("Aborted.");
        std::process::exit(0);
    }

    Ok((
        do_spectator,
        do_director,
        do_mcp,
        port,
        enable_spectator,
        enable_director,
    ))
}

fn gather_interactive_selections_no_mcp(
    do_spectator: bool,
    do_director: bool,
) -> Result<(bool, bool, bool, u16, bool, bool)> {
    // Plugin enable selection
    let mut plugin_items = vec![];
    let mut plugin_defaults = vec![];
    if do_spectator {
        plugin_items.push("Spectator");
        plugin_defaults.push(true);
    }
    if do_director {
        plugin_items.push("Director");
        plugin_defaults.push(true);
    }

    let (enable_spectator, enable_director) = if !plugin_items.is_empty() {
        let plugin_selections = MultiSelect::new()
            .with_prompt("Enable plugins in project.godot?")
            .items(&plugin_items)
            .defaults(&plugin_defaults)
            .interact()
            .context("Plugin selection cancelled")?;

        let enable_spectator = do_spectator && plugin_selections.contains(&0);
        let enable_director = if do_spectator {
            do_director && plugin_selections.contains(&1)
        } else {
            do_director && plugin_selections.contains(&0)
        };
        (enable_spectator, enable_director)
    } else {
        (false, false)
    };

    Ok((
        do_spectator,
        do_director,
        false,
        9077,
        enable_spectator,
        enable_director,
    ))
}
