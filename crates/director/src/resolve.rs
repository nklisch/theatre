use std::path::{Path, PathBuf};

/// Resolve the Godot binary path.
///
/// Priority: `GODOT_PATH` env var → `which godot`.
pub fn resolve_godot_bin() -> Result<PathBuf, ResolveError> {
    if let Ok(path) = std::env::var("GODOT_PATH") {
        return Ok(PathBuf::from(path));
    }
    which::which("godot").map_err(|_| ResolveError::GodotNotFound)
}

/// Validate that `project_path` contains a `project.godot` file.
pub fn validate_project_path(project_path: &Path) -> Result<(), ResolveError> {
    if project_path.join("project.godot").exists() {
        Ok(())
    } else {
        Err(ResolveError::NotAProject(project_path.to_path_buf()))
    }
}

/// Resolve a scene/resource path relative to the project root.
/// Returns the absolute path. Validates the parent directory exists.
pub fn resolve_scene_path(project_path: &Path, scene_path: &str) -> Result<PathBuf, ResolveError> {
    let full = project_path.join(scene_path);
    let parent = full
        .parent()
        .ok_or_else(|| ResolveError::ParentMissing(full.clone()))?;
    if !parent.exists() {
        return Err(ResolveError::ParentMissing(parent.to_path_buf()));
    }
    Ok(full)
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("Godot binary not found. Set GODOT_PATH or add `godot` to PATH")]
    GodotNotFound,

    #[error("project_path '{0}' does not contain a project.godot file")]
    NotAProject(PathBuf),

    #[error("scene parent directory does not exist: {0}")]
    ParentMissing(PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolve_godot_bin_uses_env_var() {
        // SAFETY: single-threaded test environment
        unsafe {
            std::env::set_var("GODOT_PATH", "/usr/bin/godot-fake");
        }
        let result = resolve_godot_bin();
        // SAFETY: single-threaded test environment
        unsafe {
            std::env::remove_var("GODOT_PATH");
        }
        // Should return whatever was set, no existence check
        assert_eq!(result.unwrap(), PathBuf::from("/usr/bin/godot-fake"));
    }

    #[test]
    fn validate_project_path_succeeds_for_valid_project() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("project.godot"), "").unwrap();
        assert!(validate_project_path(tmp.path()).is_ok());
    }

    #[test]
    fn validate_project_path_fails_for_non_project() {
        let tmp = tempfile::tempdir().unwrap();
        let err = validate_project_path(tmp.path()).unwrap_err();
        assert!(matches!(err, ResolveError::NotAProject(_)));
    }

    #[test]
    fn resolve_scene_path_returns_full_path() {
        let tmp = tempfile::tempdir().unwrap();
        let result = resolve_scene_path(tmp.path(), "test.tscn").unwrap();
        assert_eq!(result, tmp.path().join("test.tscn"));
    }

    #[test]
    fn resolve_scene_path_fails_for_missing_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let err = resolve_scene_path(tmp.path(), "missing_dir/test.tscn").unwrap_err();
        assert!(matches!(err, ResolveError::ParentMissing(_)));
    }
}
