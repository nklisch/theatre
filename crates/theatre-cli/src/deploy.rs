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
    let source_deployment = if let Some(source) = &source {
        let gdext_artifact = build_and_update_share(source, &theatre, args.release)?;
        Some(SourceDeployment {
            stage_addon: source.addon_source().join("stage"),
            gdext_artifact,
        })
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
        None
    };

    // Step 5: Deploy to each project
    let gdext_src = theatre.gdext_binary();
    for project in &args.projects {
        deploy_to_project(&theatre, project, &gdext_src, source_deployment.as_ref())?;
    }

    eprintln!("Deploy complete.");
    Ok(())
}

struct SourceDeployment {
    stage_addon: PathBuf,
    gdext_artifact: PathBuf,
}

/// Build from source and update the share dir.
fn build_and_update_share(
    source: &SourcePaths,
    theatre: &TheatrePaths,
    release: bool,
) -> Result<PathBuf> {
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
        let src = source.built_executable(bin_name, release);
        let dst = theatre.executable(bin_name);
        if theatre.bin_dir.exists() {
            std::fs::copy(&src, &dst).with_context(|| {
                format!("Failed to copy {} to {}", src.display(), dst.display())
            })?;
            eprintln!("  {} Updated {bin_name} in bin dir", style("✓").green());
        }
    }
    eprintln!();

    Ok(gdext_src)
}

/// Deploy from the share dir to a single project.
fn deploy_to_project(
    theatre: &TheatrePaths,
    project: &Path,
    gdext_src: &Path,
    source_deployment: Option<&SourceDeployment>,
) -> Result<()> {
    eprintln!("  Deploying to {}...", project.display());

    // Deploy stage addon
    let stage_project_dst = project.join("addons").join("stage");
    let is_symlink = std::fs::symlink_metadata(&stage_project_dst)
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false);

    if is_symlink {
        eprintln!(
            "  {} addons/stage/ is a symlink — skipping script copy (dev setup)",
            style("⚠").yellow()
        );

        let verified_source_target = source_deployment.and_then(|deployment| {
            verified_symlink_target(&stage_project_dst, &deployment.stage_addon)
        });
        if let (Some(target), Some(deployment)) = (verified_source_target, source_deployment) {
            let copied = copy_gdextension(&deployment.gdext_artifact, &target, project)?;
            if copied {
                eprintln!(
                    "  {} Updated GDExtension in verified source addon",
                    style("✓").green()
                );
            } else {
                eprintln!(
                    "  {} GDExtension already uses the freshly built artifact — skipping self-copy",
                    style("ℹ").blue()
                );
            }
        } else {
            eprintln!(
                "  {} Skipping GDExtension copy through unrelated addons/stage/ symlink",
                style("⚠").yellow()
            );
        }
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

        let copied = copy_gdextension(gdext_src, &stage_project_dst, project)?;
        if copied {
            eprintln!("  {} addons/stage/ (with GDExtension)", style("✓").green());
        } else {
            eprintln!(
                "  {} addons/stage/ (GDExtension already at destination)",
                style("✓").green()
            );
        }
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

/// Resolve both sides before trusting a project symlink, then return the
/// canonical source path so subsequent writes cannot be redirected by swapping
/// the project link after verification.
fn verified_symlink_target(link: &Path, expected_target: &Path) -> Option<PathBuf> {
    let actual_target = std::fs::canonicalize(link).ok()?;
    let expected_target = std::fs::canonicalize(expected_target).ok()?;
    (actual_target == expected_target).then_some(expected_target)
}

fn copy_gdextension(gdext_src: &Path, stage_dst: &Path, project: &Path) -> Result<bool> {
    let gdext_dir = stage_dst.join("bin").join(platform_dir());
    std::fs::create_dir_all(&gdext_dir).with_context(|| {
        format!(
            "Failed to create GDExtension dir in project: {}",
            gdext_dir.display()
        )
    })?;

    let gdext_dst = gdext_dir.join(gdext_filename());
    if paths_refer_to_same_file(gdext_src, &gdext_dst) {
        return Ok(false);
    }

    std::fs::copy(gdext_src, &gdext_dst).with_context(|| {
        format!(
            "Failed to copy GDExtension to project: {}",
            project.display()
        )
    })?;
    Ok(true)
}

fn paths_refer_to_same_file(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }

    match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}
