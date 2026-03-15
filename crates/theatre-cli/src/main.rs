use anyhow::Result;
use clap::{Parser, Subcommand};

mod deploy;
mod enable;
mod init;
mod install;
mod paths;
mod project;
mod rules;
mod telemetry;

#[derive(Parser)]
#[command(name = "theatre", version, about = "Theatre — Godot AI agent toolkit")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build and install Theatre to ~/.local
    Install(install::InstallArgs),
    /// Set up a Godot project with Theatre addons and MCP config
    Init(init::InitArgs),
    /// Rebuild and redeploy Theatre to Godot projects
    Deploy(deploy::DeployArgs),
    /// Enable or disable Theatre plugins in a Godot project
    Enable(enable::EnableArgs),
    /// Generate AI agent rules to prevent hand-editing Godot files
    Rules(rules::RulesArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Install(args) => install::run(args),
        Command::Init(args) => init::run(args),
        Command::Deploy(args) => deploy::run(args),
        Command::Enable(args) => enable::run(args),
        Command::Rules(args) => rules::run(args),
    }
}
