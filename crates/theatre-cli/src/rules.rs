use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use console::style;
use dialoguer::{Confirm, Select};

use crate::project::validate_project;

/// The rules content that tells AI agents not to hand-edit Godot files.
/// Single source of truth: `rules-template.md` at the repo root.
const RULES_CONTENT: &str = include_str!("../../../rules-template.md");

#[derive(Args)]
pub struct RulesArgs {
    /// Godot project path (default: current directory)
    #[arg(default_value = ".")]
    project: PathBuf,

    /// Skip interactive prompts (defaults to Claude Code rules file)
    #[arg(long, short = 'y')]
    yes: bool,
}

/// Target format for the rules output.
enum RulesTarget {
    /// Write to .claude/rules/godot.md (Claude Code auto-loads this)
    ClaudeRules,
    /// Append to CLAUDE.md in project root
    ClaudeMd,
    /// Append to AGENTS.md in project root
    AgentsMd,
}

pub fn run(args: RulesArgs) -> Result<()> {
    validate_project(&args.project)?;

    let target = if args.yes {
        RulesTarget::ClaudeRules
    } else {
        pick_target()?
    };

    match target {
        RulesTarget::ClaudeRules => write_claude_rules(&args.project)?,
        RulesTarget::ClaudeMd => append_to_file(&args.project, "CLAUDE.md")?,
        RulesTarget::AgentsMd => append_to_file(&args.project, "AGENTS.md")?,
    }

    Ok(())
}

/// Called from `theatre init` to optionally generate rules.
pub fn run_from_init(project: &Path, yes: bool) -> Result<()> {
    if yes {
        write_claude_rules(project)?;
        return Ok(());
    }

    let generate = Confirm::new()
        .with_prompt("Generate AI agent rules file? (prevents hand-editing .tscn/.tres)")
        .default(true)
        .interact()
        .context("Rules prompt cancelled")?;

    if !generate {
        return Ok(());
    }

    let target = pick_target()?;
    match target {
        RulesTarget::ClaudeRules => write_claude_rules(project)?,
        RulesTarget::ClaudeMd => append_to_file(project, "CLAUDE.md")?,
        RulesTarget::AgentsMd => append_to_file(project, "AGENTS.md")?,
    }

    Ok(())
}

fn pick_target() -> Result<RulesTarget> {
    let items = vec![
        ".claude/rules/godot.md  (Claude Code — auto-loaded)",
        "CLAUDE.md               (Claude Code — append to file)",
        "AGENTS.md               (other agents — append to file)",
    ];

    let selection = Select::new()
        .with_prompt("Where to write agent rules?")
        .items(&items)
        .default(0)
        .interact()
        .context("Rules target selection cancelled")?;

    Ok(match selection {
        0 => RulesTarget::ClaudeRules,
        1 => RulesTarget::ClaudeMd,
        _ => RulesTarget::AgentsMd,
    })
}

fn write_claude_rules(project: &Path) -> Result<()> {
    let rules_dir = project.join(".claude").join("rules");
    let rules_file = rules_dir.join("godot.md");

    if rules_file.exists() {
        eprintln!(
            "  {} .claude/rules/godot.md already exists — skipped",
            style("⚠").yellow()
        );
        return Ok(());
    }

    std::fs::create_dir_all(&rules_dir).context("Failed to create .claude/rules/ directory")?;
    std::fs::write(&rules_file, RULES_CONTENT).context("Failed to write .claude/rules/godot.md")?;

    eprintln!("  {} Generated .claude/rules/godot.md", style("✓").green());
    Ok(())
}

fn append_to_file(project: &Path, filename: &str) -> Result<()> {
    let file_path = project.join(filename);

    let existing = if file_path.exists() {
        std::fs::read_to_string(&file_path).with_context(|| format!("Failed to read {filename}"))?
    } else {
        String::new()
    };

    // Check if rules are already present
    if existing.contains("Never hand-edit Godot files") {
        eprintln!(
            "  {} {filename} already contains Godot rules — skipped",
            style("⚠").yellow()
        );
        return Ok(());
    }

    let separator = if existing.is_empty() || existing.ends_with('\n') {
        ""
    } else {
        "\n"
    };

    let content = format!("{existing}{separator}\n{RULES_CONTENT}");
    std::fs::write(&file_path, content).with_context(|| format!("Failed to write {filename}"))?;

    if existing.is_empty() {
        eprintln!("  {} Created {filename}", style("✓").green());
    } else {
        eprintln!(
            "  {} Appended Godot rules to {filename}",
            style("✓").green()
        );
    }
    Ok(())
}
