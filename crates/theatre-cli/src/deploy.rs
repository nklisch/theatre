use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use console::style;

use crate::paths::{SourcePaths, TheatrePaths, copy_dir_recursive, gdext_filename, platform_dir};
use crate::project::validate_project;

#[derive(Args)]
pub struct DeployArgs {
    /// Godot project paths to deploy to (default: current directory)
    #[arg(default_value = ".")]
    projects: Vec<PathBuf>,

    /// Build in release mode (default: debug)
    #[arg(long)]
    release: bool,
}

pub fn run(args: DeployArgs) -> Result<()> {
    eprintln!("{}", style("Theatre Deploy").bold());
    eprintln!();

    // Step 1: Try to discover source repo (optional — not needed for installed mode)
    let source = SourcePaths::discover().ok();

    // Step 2: Resolve theatre paths (installed location)
    let theatre = TheatrePaths::resolve()?;

    // Step 3: Validate all project paths before building
    for project in &args.projects {
        validate_project(project)
            .with_context(|| format!("Invalid project path: {}", project.display()))?;
    }

    // Step 4: Build from source or use installed share dir
    if let Some(source) = &source {
        build_and_update_share(source, &theatre, args.release)?;
    } else {
        // No source repo — verify share dir is populated
        theatre.validate_installed().map_err(|e| {
            anyhow::anyhow!(
                "No source repo found and Theatre is not installed.\n\
                Either run from within the Theatre repo, set THEATRE_ROOT, \
                or run `theatre install` first.\n\nDetails: {e}"
            )
        })?;
        eprintln!(
            "  {} No source repo found — deploying from installed share dir",
            style("ℹ").blue()
        );
        eprintln!();
    }

    // Step 5: Deploy to each project
    let gdext_src = theatre.gdext_binary();
    for project in &args.projects {
        deploy_to_project(&theatre, project, &gdext_src)?;
    }

    eprintln!("Deploy complete.");
    Ok(())
}

/// Build from source and update the share dir.
fn build_and_update_share(
    source: &SourcePaths,
    theatre: &TheatrePaths,
    release: bool,
) -> Result<()> {
    eprintln!(
        "  Building {} binaries...",
        if release { "release" } else { "debug" }
    );

    let mut cmd = std::process::Command::new("cargo");
    cmd.current_dir(&source.repo_root)
        .args([
            "build",
            "-p",
            "stage-godot",
            "-p",
            "stage-server",
            "-p",
            "director",
        ])
        .stderr(std::process::Stdio::inherit());

    if release {
        cmd.arg("--release");
    }

    let status = cmd.status().context("Failed to run cargo build")?;
    if !status.success() {
        anyhow::bail!("cargo build failed with exit code: {:?}", status.code());
    }

    eprintln!("  {} stage-godot", style("✓").green());
    eprintln!("  {} stage", style("✓").green());
    eprintln!("  {} director", style("✓").green());
    eprintln!();

    // Update share dir
    eprintln!("  Updating share dir...");

    // Copy fresh GDExtension to share dir
    let gdext_src = source.built_gdext(release);
    let gdext_platform_dir = theatre
        .addon_source()
        .join("stage")
        .join("bin")
        .join(platform_dir());
    std::fs::create_dir_all(&gdext_platform_dir).with_context(|| {
        format!(
            "Failed to create GDExtension bin dir: {}",
            gdext_platform_dir.display()
        )
    })?;
    let gdext_dst = gdext_platform_dir.join(gdext_filename());
    std::fs::copy(&gdext_src, &gdext_dst).with_context(|| {
        format!(
            "Failed to copy GDExtension from {} to {}",
            gdext_src.display(),
            gdext_dst.display()
        )
    })?;
    eprintln!("  {} Updated GDExtension in share dir", style("✓").green());

    // Sync addon GDScript from repo to share dir
    let stage_src = source.addon_source().join("stage");
    let stage_share_dst = theatre.addon_source().join("stage");
    copy_dir_recursive(&stage_src, &stage_share_dst, &|p| {
        p.file_name().map(|n| n == "bin").unwrap_or(false)
    })
    .context("Failed to sync stage addon to share dir")?;

    let director_src = source.addon_source().join("director");
    let director_share_dst = theatre.addon_source().join("director");
    copy_dir_recursive(&director_src, &director_share_dst, &|_| false)
        .context("Failed to sync director addon to share dir")?;

    eprintln!("  {} Synced addon scripts to share dir", style("✓").green());

    // Copy fresh server binaries to bin_dir
    for bin_name in &["stage", "director"] {
        let src = source.built_binary(bin_name, release);
        let dst = theatre.bin_dir.join(bin_name);
        if theatre.bin_dir.exists() {
            std::fs::copy(&src, &dst).with_context(|| {
                format!("Failed to copy {} to {}", src.display(), dst.display())
            })?;
            eprintln!("  {} Updated {bin_name} in bin dir", style("✓").green());
        }
    }
    eprintln!();

    Ok(())
}

/// Deploy from the share dir to a single project.
fn deploy_to_project(theatre: &TheatrePaths, project: &Path, gdext_src: &Path) -> Result<()> {
    eprintln!("  Deploying to {}...", project.display());

    // Deploy stage addon
    let stage_project_dst = project.join("addons").join("stage");
    let is_symlink = std::fs::symlink_metadata(&stage_project_dst)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);

    if is_symlink {
        eprintln!(
            "  {} addons/stage/ is a symlink — skipping copy (dev setup)",
            style("⚠").yellow()
        );
    } else {
        copy_dir_recursive(
            &theatre.addon_source().join("stage"),
            &stage_project_dst,
            &|_| false,
        )
        .with_context(|| {
            format!(
                "Failed to copy stage addon to {}",
                stage_project_dst.display()
            )
        })?;

        // Also copy the GDExtension binary
        let gdext_proj_dir = stage_project_dst.join("bin").join(platform_dir());
        std::fs::create_dir_all(&gdext_proj_dir).with_context(|| {
            format!(
                "Failed to create GDExtension dir in project: {}",
                gdext_proj_dir.display()
            )
        })?;
        std::fs::copy(gdext_src, gdext_proj_dir.join(gdext_filename())).with_context(|| {
            format!(
                "Failed to copy GDExtension to project: {}",
                project.display()
            )
        })?;

        eprintln!("  {} addons/stage/ (with GDExtension)", style("✓").green());
    }

    // Deploy director addon
    let director_project_dst = project.join("addons").join("director");
    let is_symlink = std::fs::symlink_metadata(&director_project_dst)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);

    if is_symlink {
        eprintln!(
            "  {} addons/director/ is a symlink — skipping copy (dev setup)",
            style("⚠").yellow()
        );
    } else {
        copy_dir_recursive(
            &theatre.addon_source().join("director"),
            &director_project_dst,
            &|_| false,
        )
        .with_context(|| {
            format!(
                "Failed to copy director addon to {}",
                director_project_dst.display()
            )
        })?;
        eprintln!("  {} addons/director/", style("✓").green());
    }

    eprintln!();
    Ok(())
}
