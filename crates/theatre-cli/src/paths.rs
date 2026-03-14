use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// Installed Theatre layout under ~/.local.
pub struct TheatrePaths {
    /// Where binaries live: ~/.local/bin (or override)
    pub bin_dir: PathBuf,
    /// Where addon templates + GDExtension live: ~/.local/share/theatre
    pub share_dir: PathBuf,
}

impl TheatrePaths {
    /// Resolve the installed Theatre paths.
    ///
    /// 1. `THEATRE_SHARE_DIR` env var (if set) → share_dir
    /// 2. Default: `~/.local/share/theatre`
    ///
    /// bin_dir resolved separately via `resolve_bin_dir()`.
    pub fn resolve() -> Result<Self> {
        Ok(Self {
            bin_dir: resolve_bin_dir()?,
            share_dir: resolve_share_dir()?,
        })
    }

    /// Path to the addon source directory within the share dir.
    /// `~/.local/share/theatre/addons`
    pub fn addon_source(&self) -> PathBuf {
        self.share_dir.join("addons")
    }

    /// Path to the GDExtension binary within the share dir.
    /// `~/.local/share/theatre/addons/spectator/bin/<platform>/<filename>`
    pub fn gdext_binary(&self) -> PathBuf {
        self.addon_source()
            .join("spectator")
            .join("bin")
            .join(platform_dir())
            .join(gdext_filename())
    }

    /// Verify the share dir has been populated (install was run).
    pub fn validate_installed(&self) -> Result<()> {
        let addon_dir = self.addon_source();
        if !addon_dir.exists() {
            bail!(
                "Theatre is not installed. The share directory does not exist: {}\n\
                Run `theatre install` first.",
                self.share_dir.display()
            );
        }
        let spectator_cfg = addon_dir.join("spectator").join("plugin.cfg");
        if !spectator_cfg.exists() {
            bail!(
                "Theatre is not installed. Expected plugin.cfg at: {}\n\
                Run `theatre install` first.",
                spectator_cfg.display()
            );
        }
        Ok(())
    }
}

/// Context needed only during `theatre install` — knows about the source repo.
pub struct SourcePaths {
    /// Root of the Theatre source tree (for cargo builds)
    pub repo_root: PathBuf,
}

impl SourcePaths {
    /// Discover the repo root for install/deploy-from-source.
    ///
    /// 1. `THEATRE_ROOT` env var (if set)
    /// 2. Walk up from current executable to find workspace Cargo.toml
    /// 3. Walk up from current working directory
    pub fn discover() -> Result<Self> {
        if let Ok(root) = std::env::var("THEATRE_ROOT") {
            let path = PathBuf::from(root);
            if is_workspace_root(&path) {
                return Ok(Self { repo_root: path });
            }
            bail!(
                "THEATRE_ROOT is set but does not contain a workspace Cargo.toml: {}",
                path.display()
            );
        }

        if let Ok(exe) = std::env::current_exe()
            && let Some(root) = walk_up_for_workspace(&exe)
        {
            return Ok(Self { repo_root: root });
        }

        if let Ok(cwd) = std::env::current_dir()
            && let Some(root) = walk_up_for_workspace(&cwd)
        {
            return Ok(Self { repo_root: root });
        }

        bail!(
            "Could not find the Theatre repo root (workspace Cargo.toml).\n\
            Set the THEATRE_ROOT environment variable to the repo root, or run from within the repo."
        )
    }

    /// Path to a built binary in the repo's target dir.
    pub fn built_binary(&self, name: &str, release: bool) -> PathBuf {
        let mode = if release { "release" } else { "debug" };
        self.repo_root.join("target").join(mode).join(name)
    }

    /// Path to the built GDExtension in the repo's target dir.
    pub fn built_gdext(&self, release: bool) -> PathBuf {
        self.built_binary(gdext_filename(), release)
    }

    /// Path to the addon source in the repo.
    pub fn addon_source(&self) -> PathBuf {
        self.repo_root.join("addons")
    }
}

fn is_workspace_root(path: &Path) -> bool {
    let cargo_toml = path.join("Cargo.toml");
    if !cargo_toml.exists() {
        return false;
    }
    let Ok(content) = std::fs::read_to_string(&cargo_toml) else {
        return false;
    };
    content.contains("[workspace]")
}

fn walk_up_for_workspace(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        if is_workspace_root(&current) {
            return Some(current);
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return None,
        }
    }
}

/// Platform-specific GDExtension library filename.
pub fn gdext_filename() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "libspectator_godot.so"
    }
    #[cfg(target_os = "macos")]
    {
        "libspectator_godot.dylib"
    }
    #[cfg(target_os = "windows")]
    {
        "spectator_godot.dll"
    }
}

/// Platform-specific subdirectory name under addons/spectator/bin/.
pub fn platform_dir() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
}

/// Default install directory for binaries: ~/.local/bin
pub fn default_bin_dir() -> Result<PathBuf> {
    let home = home_dir()?;
    Ok(home.join(".local").join("bin"))
}

/// Default share directory: ~/.local/share/theatre
pub fn default_share_dir() -> Result<PathBuf> {
    let home = home_dir()?;
    Ok(home.join(".local").join("share").join("theatre"))
}

/// Resolve bin_dir: THEATRE_BIN_DIR env → ~/.local/bin
pub fn resolve_bin_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("THEATRE_BIN_DIR") {
        return Ok(PathBuf::from(dir));
    }
    default_bin_dir()
}

/// Resolve share_dir: THEATRE_SHARE_DIR env → ~/.local/share/theatre
pub fn resolve_share_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("THEATRE_SHARE_DIR") {
        return Ok(PathBuf::from(dir));
    }
    default_share_dir()
}

fn home_dir() -> Result<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .context("Neither HOME nor USERPROFILE is set")
}

/// Recursively copy a directory tree, skipping entries matched by `skip`.
/// Used by install (repo addons → share dir) and init (share dir → project).
/// Returns the number of files copied.
pub fn copy_dir_recursive(src: &Path, dst: &Path, skip: &dyn Fn(&Path) -> bool) -> Result<u64> {
    if skip(src) {
        return Ok(0);
    }

    std::fs::create_dir_all(dst)
        .with_context(|| format!("Failed to create directory: {}", dst.display()))?;

    let mut count = 0u64;

    for entry in std::fs::read_dir(src)
        .with_context(|| format!("Failed to read directory: {}", src.display()))?
    {
        let entry = entry.with_context(|| format!("Failed to read entry in: {}", src.display()))?;
        let src_path = entry.path();
        let file_name = entry.file_name();

        // Skip .git and .godot directories
        let name = file_name.to_string_lossy();
        if name == ".git" || name == ".godot" {
            continue;
        }

        let dst_path = dst.join(&file_name);

        if skip(&src_path) {
            continue;
        }

        let file_type = entry
            .file_type()
            .with_context(|| format!("Failed to get file type: {}", src_path.display()))?;

        if file_type.is_dir() {
            count += copy_dir_recursive(&src_path, &dst_path, skip)?;
        } else if file_type.is_file() {
            std::fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "Failed to copy {} to {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
            count += 1;
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_gdext_filename() {
        let name = gdext_filename();
        #[cfg(target_os = "linux")]
        assert_eq!(name, "libspectator_godot.so");
        #[cfg(target_os = "macos")]
        assert_eq!(name, "libspectator_godot.dylib");
        #[cfg(target_os = "windows")]
        assert_eq!(name, "spectator_godot.dll");
    }

    #[test]
    fn test_platform_dir() {
        let dir = platform_dir();
        #[cfg(target_os = "linux")]
        assert_eq!(dir, "linux");
        #[cfg(target_os = "macos")]
        assert_eq!(dir, "macos");
        #[cfg(target_os = "windows")]
        assert_eq!(dir, "windows");
    }

    #[test]
    fn test_default_bin_dir() {
        // Set HOME to a known value so we can assert the result
        unsafe { std::env::set_var("HOME", "/home/testuser") };
        let bin_dir = default_bin_dir().unwrap();
        assert_eq!(bin_dir, PathBuf::from("/home/testuser/.local/bin"));
    }

    #[test]
    fn test_default_share_dir() {
        unsafe { std::env::set_var("HOME", "/home/testuser") };
        let share_dir = default_share_dir().unwrap();
        assert_eq!(
            share_dir,
            PathBuf::from("/home/testuser/.local/share/theatre")
        );
    }

    #[test]
    fn test_resolve_share_dir_from_env() {
        unsafe { std::env::set_var("THEATRE_SHARE_DIR", "/tmp/custom-share") };
        let share_dir = resolve_share_dir().unwrap();
        assert_eq!(share_dir, PathBuf::from("/tmp/custom-share"));
        unsafe { std::env::remove_var("THEATRE_SHARE_DIR") };
    }

    #[test]
    fn test_source_discover_from_env() {
        let tmp = TempDir::new().unwrap();
        // Create a fake workspace Cargo.toml
        std::fs::write(tmp.path().join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();

        unsafe { std::env::set_var("THEATRE_ROOT", tmp.path().to_str().unwrap()) };
        let source = SourcePaths::discover().unwrap();
        assert_eq!(source.repo_root, tmp.path());
        unsafe { std::env::remove_var("THEATRE_ROOT") };
    }

    #[test]
    fn test_copy_dir_recursive() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();

        // Create a tree: src/a.txt, src/sub/b.txt
        std::fs::write(src.path().join("a.txt"), "hello").unwrap();
        std::fs::create_dir(src.path().join("sub")).unwrap();
        std::fs::write(src.path().join("sub").join("b.txt"), "world").unwrap();

        let count = copy_dir_recursive(src.path(), dst.path(), &|_| false).unwrap();
        assert_eq!(count, 2);

        assert!(dst.path().join("a.txt").exists());
        assert!(dst.path().join("sub").join("b.txt").exists());
        assert_eq!(
            std::fs::read_to_string(dst.path().join("a.txt")).unwrap(),
            "hello"
        );
    }

    #[test]
    fn test_copy_dir_recursive_with_skip() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();

        std::fs::write(src.path().join("keep.txt"), "keep").unwrap();
        std::fs::create_dir(src.path().join("bin")).unwrap();
        std::fs::write(src.path().join("bin").join("lib.so"), "binary").unwrap();

        let count = copy_dir_recursive(src.path(), dst.path(), &|p| {
            p.file_name().map(|n| n == "bin").unwrap_or(false)
        })
        .unwrap();

        assert_eq!(count, 1);
        assert!(dst.path().join("keep.txt").exists());
        assert!(!dst.path().join("bin").exists());
    }
}
