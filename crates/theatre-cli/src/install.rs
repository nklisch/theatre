use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use console::style;

use crate::paths::{
    SourcePaths, copy_dir_recursive, gdext_filename, platform_dir, resolve_bin_dir,
    resolve_share_dir,
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
            "spectator-server",
            "-p",
            "spectator-godot",
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

    eprintln!("  {} spectator", style("✓").green());
    eprintln!("  {} director", style("✓").green());
    eprintln!("  {} spectator-godot", style("✓").green());
    eprintln!("  {} theatre", style("✓").green());
    eprintln!();

    // Step 6: Copy binaries to bin_dir
    eprintln!("  Installing to {}/:", bin_dir.display());

    for bin_name in &["spectator", "director", "theatre"] {
        let src = source.built_binary(bin_name, true);
        let dst = bin_dir.join(bin_name);
        std::fs::copy(&src, &dst)
            .with_context(|| format!("Failed to copy {} to {}", src.display(), dst.display()))?;
        eprintln!("  {} {bin_name}", style("✓").green());
    }
    eprintln!();

    // Step 7: Copy addon templates
    eprintln!("  Installing to {}/:", share_dir.display());

    let share_addons = share_dir.join("addons");

    // Copy spectator addon, skipping bin/ subdir
    let spectator_src = source.addon_source().join("spectator");
    let spectator_dst = share_addons.join("spectator");
    let spectator_count = copy_dir_recursive(&spectator_src, &spectator_dst, &|p| {
        p.file_name().map(|n| n == "bin").unwrap_or(false)
    })
    .with_context(|| {
        format!(
            "Failed to copy spectator addon from {}",
            spectator_src.display()
        )
    })?;
    eprintln!(
        "  {} addons/spectator/ ({spectator_count} files)",
        style("✓").green()
    );

    // Step 8: Copy GDExtension binary
    let gdext_src = source.built_gdext(true);
    let gdext_platform_dir = spectator_dst.join("bin").join(platform_dir());
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
        "  {} addons/spectator/bin/{}/{gdext_filename}",
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
    let path_env = std::env::var("PATH").unwrap_or_default();
    let bin_dir_str = bin_dir.to_string_lossy();
    if !path_env.split(':').any(|p| p == bin_dir_str.as_ref()) {
        eprintln!(
            "  {} {} is not in your PATH. Add it:",
            style("⚠").yellow(),
            bin_dir.display()
        );
        eprintln!("    export PATH=\"$HOME/.local/bin:$PATH\"");
        eprintln!();
    }

    // Step 10: Summary
    eprintln!("Install complete. Run `theatre init <project>` to set up a Godot project.");

    Ok(())
}
