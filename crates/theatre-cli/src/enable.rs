use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use console::style;

use crate::project::{remove_autoload, set_autoload, set_plugin_enabled, validate_project};

const SPECTATOR_PLUGIN_CFG: &str = "res://addons/spectator/plugin.cfg";
const DIRECTOR_PLUGIN_CFG: &str = "res://addons/director/plugin.cfg";
const SPECTATOR_RUNTIME_NAME: &str = "SpectatorRuntime";
const SPECTATOR_RUNTIME_SCRIPT: &str = "res://addons/spectator/runtime.gd";

#[derive(Args)]
pub struct EnableArgs {
    /// Godot project path
    project: PathBuf,

    /// Enable only Spectator (default: both)
    #[arg(long)]
    spectator: bool,

    /// Enable only Director (default: both)
    #[arg(long)]
    director: bool,

    /// Disable instead of enable
    #[arg(long)]
    disable: bool,
}

pub fn run(args: EnableArgs) -> Result<()> {
    eprintln!("{}", style("Theatre Enable").bold());
    eprintln!();

    // Step 1: Validate project
    validate_project(&args.project)?;

    // Step 2: Determine which plugins to act on
    let act_on_both = !args.spectator && !args.director;
    let do_spectator = act_on_both || args.spectator;
    let do_director = act_on_both || args.director;
    let enabling = !args.disable;

    // Step 3: Act on each plugin
    if do_spectator {
        // Check if addon files exist
        let plugin_cfg = args
            .project
            .join("addons")
            .join("spectator")
            .join("plugin.cfg");
        if enabling && !plugin_cfg.exists() {
            eprintln!(
                "  {} addons/spectator/plugin.cfg not found — plugin enabled in project.godot \
                but won't load until files are copied. Run `theatre init` or `theatre deploy`.",
                style("⚠").yellow()
            );
        }

        set_plugin_enabled(&args.project, SPECTATOR_PLUGIN_CFG, enabling)?;
        if enabling {
            eprintln!(
                "  {} Spectator enabled in project.godot",
                style("✓").green()
            );
            set_autoload(
                &args.project,
                SPECTATOR_RUNTIME_NAME,
                SPECTATOR_RUNTIME_SCRIPT,
            )?;
            eprintln!("  {} SpectatorRuntime autoload added", style("✓").green());
        } else {
            eprintln!(
                "  {} Spectator disabled in project.godot",
                style("✓").green()
            );
            remove_autoload(&args.project, SPECTATOR_RUNTIME_NAME)?;
            eprintln!("  {} SpectatorRuntime autoload removed", style("✓").green());
        }
    }

    if do_director {
        let plugin_cfg = args
            .project
            .join("addons")
            .join("director")
            .join("plugin.cfg");
        if enabling && !plugin_cfg.exists() {
            eprintln!(
                "  {} addons/director/plugin.cfg not found — plugin enabled in project.godot \
                but won't load until files are copied. Run `theatre init` or `theatre deploy`.",
                style("⚠").yellow()
            );
        }

        set_plugin_enabled(&args.project, DIRECTOR_PLUGIN_CFG, enabling)?;
        if enabling {
            eprintln!("  {} Director enabled in project.godot", style("✓").green());
        } else {
            eprintln!(
                "  {} Director disabled in project.godot",
                style("✓").green()
            );
        }
    }

    Ok(())
}
