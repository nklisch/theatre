use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use console::style;

use crate::paths::{SourcePaths, TheatrePaths, copy_dir_recursive, gdext_filename, platform_dir};
use crate::project::validate_project;

#[derive(Args)]
pub struct DeployArgs {
    /// Godot project paths to deploy to
    #[arg(required = true)]
    projects: Vec<PathBuf>,

    /// Build in release mode (default: debug)
    #[arg(long)]
    release: bool,
}

pub fn run(args: DeployArgs) -> Result<()> {
    eprintln!("{}", style("Theatre Deploy").bold());
    eprintln!();

    // Step 1: Resolve source paths
    let source = SourcePaths::discover()?;

    // Step 2: Resolve theatre paths
    let theatre = TheatrePaths::resolve()?;

    // Step 3: Validate all project paths before building
    for project in &args.projects {
        validate_project(project)
            .with_context(|| format!("Invalid project path: {}", project.display()))?;
    }

    // Step 4: Build
    let mode_flag = if args.release { "--release" } else { "" };
    eprintln!(
        "  Building {} binaries...",
        if args.release { "release" } else { "debug" }
    );

    let mut cmd = std::process::Command::new("cargo");
    cmd.current_dir(&source.repo_root)
        .args([
            "build",
            "-p",
            "spectator-godot",
            "-p",
            "spectator-server",
            "-p",
            "director",
        ])
        .stderr(std::process::Stdio::inherit());

    if args.release {
        cmd.arg("--release");
    }
    let _ = mode_flag; // already used above

    let status = cmd.status().context("Failed to run cargo build")?;
    if !status.success() {
        anyhow::bail!("cargo build failed with exit code: {:?}", status.code());
    }

    eprintln!("  {} spectator-godot", style("✓").green());
    eprintln!("  {} spectator", style("✓").green());
    eprintln!("  {} director", style("✓").green());
    eprintln!();

    // Step 5: Update share dir
    eprintln!("  Updating share dir...");

    // 5a: Copy fresh GDExtension to share dir
    let gdext_src = source.built_gdext(args.release);
    let gdext_platform_dir = theatre
        .addon_source()
        .join("spectator")
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

    // 5b: Sync addon GDScript from repo to share dir
    let spectator_src = source.addon_source().join("spectator");
    let spectator_share_dst = theatre.addon_source().join("spectator");
    copy_dir_recursive(&spectator_src, &spectator_share_dst, &|p| {
        p.file_name().map(|n| n == "bin").unwrap_or(false)
    })
    .context("Failed to sync spectator addon to share dir")?;

    let director_src = source.addon_source().join("director");
    let director_share_dst = theatre.addon_source().join("director");
    copy_dir_recursive(&director_src, &director_share_dst, &|_| false)
        .context("Failed to sync director addon to share dir")?;

    eprintln!("  {} Synced addon scripts to share dir", style("✓").green());

    // 5c: Copy fresh server binaries to bin_dir
    for bin_name in &["spectator", "director"] {
        let src = source.built_binary(bin_name, args.release);
        let dst = theatre.bin_dir.join(bin_name);
        if theatre.bin_dir.exists() {
            std::fs::copy(&src, &dst).with_context(|| {
                format!("Failed to copy {} to {}", src.display(), dst.display())
            })?;
            eprintln!("  {} Updated {bin_name} in bin dir", style("✓").green());
        }
    }
    eprintln!();

    // Step 6: Deploy to each project
    for project in &args.projects {
        eprintln!("  Deploying to {}...", project.display());

        // Deploy spectator addon
        let spectator_project_dst = project.join("addons").join("spectator");
        let is_symlink = std::fs::symlink_metadata(&spectator_project_dst)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);

        if is_symlink {
            eprintln!(
                "  {} addons/spectator/ is a symlink — skipping copy (dev setup)",
                style("⚠").yellow()
            );
        } else {
            // Copy everything from share dir including bin/
            copy_dir_recursive(
                &theatre.addon_source().join("spectator"),
                &spectator_project_dst,
                &|_| false,
            )
            .with_context(|| {
                format!(
                    "Failed to copy spectator addon to {}",
                    spectator_project_dst.display()
                )
            })?;

            // Also copy the GDExtension binary
            let gdext_proj_dir = spectator_project_dst.join("bin").join(platform_dir());
            std::fs::create_dir_all(&gdext_proj_dir).with_context(|| {
                format!(
                    "Failed to create GDExtension dir in project: {}",
                    gdext_proj_dir.display()
                )
            })?;
            std::fs::copy(&gdext_dst, gdext_proj_dir.join(gdext_filename())).with_context(
                || {
                    format!(
                        "Failed to copy GDExtension to project: {}",
                        project.display()
                    )
                },
            )?;

            eprintln!(
                "  {} addons/spectator/ (with GDExtension)",
                style("✓").green()
            );
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
    }

    eprintln!("Deploy complete.");
    Ok(())
}
