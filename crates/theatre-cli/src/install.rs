use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use console::style;

use crate::paths::{
    SourcePaths, copy_dir_recursive, executable_filename, gdext_filename, platform_dir,
    resolve_bin_dir, resolve_share_dir,
};

#[derive(Args)]
pub struct InstallArgs {
    /// Installation directory for binaries (default: ~/.local/bin)
    #[arg(long)]
    bin_dir: Option<PathBuf>,

    /// Installation directory for shared data (default: ~/.local/share/theatre)
    #[arg(long)]
    share_dir: Option<PathBuf>,
}

pub fn run(args: InstallArgs) -> Result<()> {
    eprintln!("{}", style("Theatre Install").bold());
    eprintln!();

    // Step 1: Resolve source paths
    let source = SourcePaths::discover()?;

    // Step 2 & 3: Resolve directories
    let bin_dir = match args.bin_dir {
        Some(d) => d,
        None => resolve_bin_dir()?,
    };
    let share_dir = match args.share_dir {
        Some(d) => d,
        None => resolve_share_dir()?,
    };

    // Step 4: Create directories
    std::fs::create_dir_all(&bin_dir)
        .with_context(|| format!("Failed to create bin dir: {}", bin_dir.display()))?;
    std::fs::create_dir_all(&share_dir)
        .with_context(|| format!("Failed to create share dir: {}", share_dir.display()))?;

    // Step 5: Build release binaries
    eprintln!("  Building release binaries...");
    let status = std::process::Command::new("cargo")
        .current_dir(&source.repo_root)
        .args([
            "build",
            "--release",
            "-p",
            "stage-server",
            "-p",
            "stage-godot",
            "-p",
            "director",
            "-p",
            "theatre-cli",
        ])
        .stderr(std::process::Stdio::inherit())
        .status()
        .context("Failed to run cargo build")?;

    if !status.success() {
        anyhow::bail!("cargo build failed with exit code: {:?}", status.code());
    }

    eprintln!("  {} stage", style("✓").green());
    eprintln!("  {} director", style("✓").green());
    eprintln!("  {} stage-godot", style("✓").green());
    eprintln!("  {} theatre", style("✓").green());
    eprintln!();

    // Step 6: Copy binaries to bin_dir
    eprintln!("  Installing to {}/:", bin_dir.display());

    for bin_name in &["stage", "director", "theatre"] {
        let src = source.built_executable(bin_name, true);
        let filename = executable_filename(bin_name);
        let dst = bin_dir.join(&filename);
        std::fs::copy(&src, &dst)
            .with_context(|| format!("Failed to copy {} to {}", src.display(), dst.display()))?;
        eprintln!("  {} {filename}", style("✓").green());
    }
    eprintln!();

    // Step 7: Copy addon templates
    eprintln!("  Installing to {}/:", share_dir.display());

    let share_addons = share_dir.join("addons");

    // Copy stage addon, skipping bin/ subdir
    let stage_src = source.addon_source().join("stage");
    let stage_dst = share_addons.join("stage");
    let stage_count = copy_dir_recursive(&stage_src, &stage_dst, &|p| {
        p.file_name().map(|n| n == "bin").unwrap_or(false)
    })
    .with_context(|| format!("Failed to copy stage addon from {}", stage_src.display()))?;
    eprintln!(
        "  {} addons/stage/ ({stage_count} files)",
        style("✓").green()
    );

    // Step 8: Copy GDExtension binary
    let gdext_src = source.built_gdext(true);
    let gdext_platform_dir = stage_dst.join("bin").join(platform_dir());
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
    eprintln!(
        "  {} addons/stage/bin/{}/{gdext_filename}",
        style("✓").green(),
        platform_dir(),
        gdext_filename = gdext_filename()
    );

    // Copy director addon
    let director_src = source.addon_source().join("director");
    let director_dst = share_addons.join("director");
    let director_count = copy_dir_recursive(&director_src, &director_dst, &|_| false)
        .with_context(|| {
            format!(
                "Failed to copy director addon from {}",
                director_src.display()
            )
        })?;
    eprintln!(
        "  {} addons/director/ ({director_count} files)",
        style("✓").green()
    );
    eprintln!();

    // Step 9: Check if bin_dir is in PATH
    if !path_contains(&bin_dir) {
        eprintln!(
            "  {} {} is not in your PATH. Add it:",
            style("⚠").yellow(),
            bin_dir.display()
        );
        print_path_instruction(&bin_dir);
        eprintln!();
    }

    // Step 10: Anonymous install telemetry (respects DO_NOT_TRACK, CI, etc.)
    crate::telemetry::record_install();

    // Step 11: Summary
    eprintln!("Install complete. Run `theatre init <project>` to set up a Godot project.");

    Ok(())
}

fn path_contains(bin_dir: &std::path::Path) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|entry| paths_equivalent(&entry, bin_dir))
}

fn paths_equivalent(left: &std::path::Path, right: &std::path::Path) -> bool {
    if let (Ok(left), Ok(right)) = (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
        return left == right;
    }

    #[cfg(windows)]
    {
        let normalize = |path: &std::path::Path| {
            path.to_string_lossy()
                .replace('/', "\\")
                .trim_end_matches('\\')
                .to_lowercase()
        };
        normalize(left) == normalize(right)
    }

    #[cfg(not(windows))]
    {
        left == right
    }
}

#[cfg(windows)]
fn print_path_instruction(bin_dir: &std::path::Path) {
    let resolved = std::fs::canonicalize(bin_dir).unwrap_or_else(|_| bin_dir.to_path_buf());
    let escaped = resolved.to_string_lossy().replace('\'', "''");
    eprintln!("    $theatreBin = '{escaped}'");
    eprintln!(
        "    [Environment]::SetEnvironmentVariable('Path', [Environment]::GetEnvironmentVariable('Path', 'User') + [IO.Path]::PathSeparator + $theatreBin, 'User')"
    );
}

#[cfg(not(windows))]
fn print_path_instruction(bin_dir: &std::path::Path) {
    let resolved = std::fs::canonicalize(bin_dir).unwrap_or_else(|_| bin_dir.to_path_buf());
    eprintln!("    export PATH=\"{}:$PATH\"", resolved.display());
}
