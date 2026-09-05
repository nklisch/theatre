use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use console::style;
use dialoguer::{Confirm, Select};

use crate::project::validate_project;

/// Guidance distributed to project-local AI agents.
/// Single source of truth: `rules-template.md` at the repo root.
const RULES_CONTENT: &str = include_str!("../../../rules-template.md");
const RULES_MARKER: &str = "# Godot Project File Guidance";
const LEGACY_RULES_MARKER: &str = "## Never hand-edit Godot files";

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
        .with_prompt("Generate AI agent guidance for Godot project files?")
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

    // The legacy generated section contradicts the current guidance. Preserve the
    // surrounding user-owned file and require an intentional reconciliation instead
    // of silently appending a second, conflicting policy.
    if existing.contains(LEGACY_RULES_MARKER) {
        eprintln!(
            "  {} {filename} contains legacy generated Godot rules that conflict with current guidance — left unchanged. Remove or reconcile the 'Never hand-edit Godot files' section, then rerun `theatre rules`.",
            style("⚠").yellow()
        );
        return Ok(());
    }

    // Avoid appending a second copy of the current generated guidance.
    if existing.contains(RULES_MARKER) {
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

#[cfg(test)]
mod tests {
    use super::{RULES_CONTENT, RULES_MARKER, append_to_file};

    #[test]
    fn generated_rules_allow_inspection_and_keep_source_code_code_first() {
        assert!(RULES_CONTENT.contains("Read Godot project files"));
        assert!(RULES_CONTENT.contains("inspect their diffs"));
        assert!(RULES_CONTENT.contains("GDScript (`.gd`)"));
        assert!(RULES_CONTENT.contains("shader source (`.gdshader`)"));
    }

    #[test]
    fn generated_rules_prefer_native_structural_edits_without_blanket_claims() {
        assert!(RULES_CONTENT.starts_with(RULES_MARKER));
        assert!(RULES_CONTENT.contains("prefer **Director**"));
        assert!(RULES_CONTENT.contains("Do not automatically fall back"));
        assert!(RULES_CONTENT.contains("live root and native undo history"));
        assert!(RULES_CONTENT.contains("remain unsaved until `scene_save`"));
        assert!(RULES_CONTENT.contains("saved content after partial failures"));
        assert!(RULES_CONTENT.contains("Detached headless scene and resource operations persist"));
        assert!(!RULES_CONTENT.contains("Do NOT directly read"));
        assert!(!RULES_CONTENT.contains("will produce corrupt"));
    }

    #[test]
    fn append_preserves_user_content_and_adds_current_guidance() {
        let project = tempfile::tempdir().unwrap();
        let original = "# My project\n\nKeep this user-authored section.\n";
        std::fs::write(project.path().join("CLAUDE.md"), original).unwrap();

        append_to_file(project.path(), "CLAUDE.md").unwrap();

        let content = std::fs::read_to_string(project.path().join("CLAUDE.md")).unwrap();
        assert!(content.starts_with(original));
        assert_eq!(content.matches(RULES_MARKER).count(), 1);
    }

    #[test]
    fn append_refuses_legacy_generated_rules_without_changing_user_content() {
        let project = tempfile::tempdir().unwrap();
        for filename in ["CLAUDE.md", "AGENTS.md"] {
            let original = String::from(
                "# My project\n\nUser-authored content.\n\n## Never hand-edit Godot files\n\nDo NOT directly read Godot files.\n",
            );
            let path = project.path().join(filename);
            std::fs::write(&path, &original).unwrap();

            append_to_file(project.path(), filename).unwrap();

            assert_eq!(std::fs::read_to_string(path).unwrap(), original);
        }
    }
}
