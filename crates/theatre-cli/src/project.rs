use std::path::Path;

use anyhow::{Context, Result, bail};

/// Validate that a path is a Godot project (contains project.godot).
pub fn validate_project(path: &Path) -> Result<()> {
    if !path.exists() {
        bail!("Project path does not exist: {}", path.display());
    }
    if !path.is_dir() {
        bail!("Project path is not a directory: {}", path.display());
    }
    let godot_file = path.join("project.godot");
    if !godot_file.exists() {
        bail!(
            "Not a Godot project (project.godot not found): {}",
            path.display()
        );
    }
    Ok(())
}

/// Read project.godot and return its contents as a String.
pub fn read_project_godot(project: &Path) -> Result<String> {
    let path = project.join("project.godot");
    std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read project.godot at: {}", path.display()))
}

fn write_project_godot(project: &Path, content: &str) -> Result<()> {
    let path = project.join("project.godot");
    std::fs::write(&path, content)
        .with_context(|| format!("Failed to write project.godot at: {}", path.display()))
}

/// Check which Theatre plugins are currently enabled in project.godot.
/// Returns (spectator_enabled, director_enabled).
#[allow(dead_code)]
pub fn check_plugins_enabled(project: &Path) -> Result<(bool, bool)> {
    let content = read_project_godot(project)?;
    let array = find_enabled_array(&content);
    let spectator = array.contains("addons/spectator/plugin.cfg");
    let director = array.contains("addons/director/plugin.cfg");
    Ok((spectator, director))
}

/// Extract the PackedStringArray content from [editor_plugins] section.
#[allow(dead_code)]
fn find_enabled_array(content: &str) -> String {
    let in_section = find_in_section(content, "editor_plugins", "enabled");
    in_section.unwrap_or_default()
}

/// Find the value of `key` inside `[section_name]` block.
fn find_in_section(content: &str, section_name: &str, key: &str) -> Option<String> {
    let section_header = format!("[{section_name}]");
    let mut in_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == section_header {
            in_section = true;
            continue;
        }
        if in_section {
            if trimmed.starts_with('[') {
                // Entered a new section
                break;
            }
            let prefix = format!("{key}=");
            if trimmed.starts_with(&prefix) {
                return Some(trimmed[prefix.len()..].to_string());
            }
        }
    }
    None
}

/// Enable or disable a plugin in project.godot.
///
/// Parses the `[editor_plugins]` section and modifies the
/// `enabled=PackedStringArray(...)` value. Creates the section if missing.
///
/// `plugin_cfg_path` is the res:// path, e.g.
/// `"res://addons/spectator/plugin.cfg"`.
pub fn set_plugin_enabled(
    project: &Path,
    plugin_cfg_path: &str,
    enabled: bool,
) -> Result<()> {
    let content = read_project_godot(project)?;
    let new_content = modify_plugin_enabled(&content, plugin_cfg_path, enabled);
    write_project_godot(project, &new_content)
}

fn modify_plugin_enabled(content: &str, plugin_cfg_path: &str, enabled: bool) -> String {
    // Find the [editor_plugins] section
    let section_header = "[editor_plugins]";
    let enabled_prefix = "enabled=PackedStringArray(";

    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    // Find the section line index
    let section_idx = lines.iter().position(|l| l.trim() == section_header);

    match section_idx {
        Some(sec_idx) => {
            // Find the enabled= line within this section
            let mut enabled_line_idx = None;
            for (i, line) in lines.iter().enumerate().skip(sec_idx + 1) {
                let trimmed = line.trim();
                if trimmed.starts_with('[') {
                    break;
                }
                if trimmed.starts_with(enabled_prefix) {
                    enabled_line_idx = Some(i);
                    break;
                }
            }

            match enabled_line_idx {
                Some(idx) => {
                    let new_line = toggle_plugin_in_array(&lines[idx], plugin_cfg_path, enabled);
                    lines[idx] = new_line;
                }
                None => {
                    // Insert an enabled= line after the section header
                    let new_line = if enabled {
                        format!("enabled=PackedStringArray(\"{plugin_cfg_path}\")")
                    } else {
                        "enabled=PackedStringArray()".to_string()
                    };
                    lines.insert(sec_idx + 1, new_line);
                }
            }
        }
        None => {
            // Create the [editor_plugins] section at end of file
            let new_line = if enabled {
                format!("enabled=PackedStringArray(\"{plugin_cfg_path}\")")
            } else {
                "enabled=PackedStringArray()".to_string()
            };
            // Add a blank line before the new section if content doesn't end with one
            if !lines.last().map(|l| l.is_empty()).unwrap_or(true) {
                lines.push(String::new());
            }
            lines.push(section_header.to_string());
            lines.push(new_line);
        }
    }

    let mut result = lines.join("\n");
    // Preserve trailing newline if original had one
    if content.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Modify a `enabled=PackedStringArray(...)` line to add or remove a plugin.
fn toggle_plugin_in_array(line: &str, plugin_cfg_path: &str, enabled: bool) -> String {
    let trimmed = line.trim();
    let prefix = "enabled=PackedStringArray(";
    let suffix = ")";

    if !trimmed.starts_with(prefix) || !trimmed.ends_with(suffix) {
        return line.to_string();
    }

    let inner = &trimmed[prefix.len()..trimmed.len() - suffix.len()];
    let quoted_path = format!("\"{plugin_cfg_path}\"");

    let mut entries: Vec<String> = if inner.trim().is_empty() {
        vec![]
    } else {
        inner
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    let already_present = entries.contains(&quoted_path);

    if enabled && !already_present {
        entries.push(quoted_path);
    } else if !enabled && already_present {
        entries.retain(|e| e != &quoted_path);
    }

    if entries.is_empty() {
        "enabled=PackedStringArray()".to_string()
    } else {
        format!("enabled=PackedStringArray({})", entries.join(", "))
    }
}

/// Add an autoload entry to project.godot if not already present.
///
/// Entries follow Godot format: `Name="*res://path/to/script.gd"`
/// The `*` prefix means "enabled".
pub fn set_autoload(project: &Path, name: &str, script_path: &str) -> Result<()> {
    let content = read_project_godot(project)?;
    let new_content = modify_autoload(&content, name, script_path, true);
    write_project_godot(project, &new_content)
}

/// Remove an autoload entry from project.godot.
pub fn remove_autoload(project: &Path, name: &str) -> Result<()> {
    let content = read_project_godot(project)?;
    let new_content = modify_autoload(&content, name, "", false);
    write_project_godot(project, &new_content)
}

fn modify_autoload(content: &str, name: &str, script_path: &str, add: bool) -> String {
    let section_header = "[autoload]";
    let entry_prefix = format!("{name}=");

    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    let section_idx = lines.iter().position(|l| l.trim() == section_header);

    match section_idx {
        Some(sec_idx) => {
            // Look for existing entry in this section
            let mut entry_idx = None;
            for (i, line) in lines.iter().enumerate().skip(sec_idx + 1) {
                let trimmed = line.trim();
                if trimmed.starts_with('[') {
                    break;
                }
                if trimmed.starts_with(&entry_prefix) {
                    entry_idx = Some(i);
                    break;
                }
            }

            if add {
                if entry_idx.is_none() {
                    // Insert after section header
                    let entry = format!("{name}=\"*{script_path}\"");
                    lines.insert(sec_idx + 1, entry);
                }
                // If already present, no-op
            } else if let Some(idx) = entry_idx {
                lines.remove(idx);
            }
            // If removing and not present, no-op
        }
        None => {
            if add {
                // Create [autoload] section
                if !lines.last().map(|l| l.is_empty()).unwrap_or(true) {
                    lines.push(String::new());
                }
                lines.push(section_header.to_string());
                lines.push(format!("{name}=\"*{script_path}\""));
            }
            // If removing and section doesn't exist, no-op
        }
    }

    let mut result = lines.join("\n");
    if content.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Generate .mcp.json content for a project.
///
/// `spectator_bin` and `director_bin` are absolute paths to the installed
/// binaries.
pub fn generate_mcp_json(
    spectator_bin: &Path,
    director_bin: &Path,
    include_spectator: bool,
    include_director: bool,
    port: Option<u16>,
) -> serde_json::Value {
    let mut servers = serde_json::Map::new();

    if include_spectator {
        let mut spectator = serde_json::json!({
            "type": "stdio",
            "command": spectator_bin.to_string_lossy()
        });
        if let Some(p) = port
            && p != 9077
        {
            spectator["env"] = serde_json::json!({
                "THEATRE_PORT": p.to_string()
            });
        }
        servers.insert("spectator".to_string(), spectator);
    }

    if include_director {
        let director = serde_json::json!({
            "type": "stdio",
            "command": director_bin.to_string_lossy(),
            "args": ["serve"]
        });
        servers.insert("director".to_string(), director);
    }

    serde_json::json!({
        "mcpServers": servers
    })
}

/// Write .mcp.json to project root. Returns false without writing if file
/// exists and overwrite is false.
pub fn write_mcp_json(
    project: &Path,
    content: &serde_json::Value,
    overwrite: bool,
) -> Result<bool> {
    let path = project.join(".mcp.json");
    if path.exists() && !overwrite {
        return Ok(false);
    }
    let json = serde_json::to_string_pretty(content)
        .context("Failed to serialize .mcp.json")?;
    std::fs::write(&path, json + "\n")
        .with_context(|| format!("Failed to write .mcp.json at: {}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    fn make_project(content: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("project.godot"), content).unwrap();
        dir
    }

    #[test]
    fn test_validate_project_ok() {
        let dir = make_project("[gd_resource type=\"ProjectSettings\"]\n");
        assert!(validate_project(dir.path()).is_ok());
    }

    #[test]
    fn test_validate_project_missing() {
        let dir = TempDir::new().unwrap();
        let err = validate_project(dir.path()).unwrap_err();
        assert!(err.to_string().contains("project.godot"));
    }

    #[test]
    fn test_check_plugins_empty() {
        let dir = make_project("[editor_plugins]\nenabled=PackedStringArray()\n");
        let (s, d) = check_plugins_enabled(dir.path()).unwrap();
        assert!(!s);
        assert!(!d);
    }

    #[test]
    fn test_check_plugins_one_enabled() {
        let dir = make_project(
            "[editor_plugins]\nenabled=PackedStringArray(\"res://addons/spectator/plugin.cfg\")\n",
        );
        let (s, d) = check_plugins_enabled(dir.path()).unwrap();
        assert!(s);
        assert!(!d);
    }

    #[test]
    fn test_check_plugins_both_enabled() {
        let dir = make_project(
            "[editor_plugins]\nenabled=PackedStringArray(\"res://addons/spectator/plugin.cfg\", \"res://addons/director/plugin.cfg\")\n",
        );
        let (s, d) = check_plugins_enabled(dir.path()).unwrap();
        assert!(s);
        assert!(d);
    }

    #[test]
    fn test_set_plugin_enabled_add_to_empty() {
        let dir = make_project("[editor_plugins]\nenabled=PackedStringArray()\n");
        set_plugin_enabled(
            dir.path(),
            "res://addons/spectator/plugin.cfg",
            true,
        )
        .unwrap();
        let content = read_project_godot(dir.path()).unwrap();
        assert!(content.contains("res://addons/spectator/plugin.cfg"));
    }

    #[test]
    fn test_set_plugin_enabled_add_second() {
        let dir = make_project(
            "[editor_plugins]\nenabled=PackedStringArray(\"res://addons/spectator/plugin.cfg\")\n",
        );
        set_plugin_enabled(
            dir.path(),
            "res://addons/director/plugin.cfg",
            true,
        )
        .unwrap();
        let content = read_project_godot(dir.path()).unwrap();
        assert!(content.contains("res://addons/spectator/plugin.cfg"));
        assert!(content.contains("res://addons/director/plugin.cfg"));
    }

    #[test]
    fn test_set_plugin_enabled_remove() {
        let dir = make_project(
            "[editor_plugins]\nenabled=PackedStringArray(\"res://addons/spectator/plugin.cfg\", \"res://addons/director/plugin.cfg\")\n",
        );
        set_plugin_enabled(
            dir.path(),
            "res://addons/spectator/plugin.cfg",
            false,
        )
        .unwrap();
        let content = read_project_godot(dir.path()).unwrap();
        assert!(!content.contains("res://addons/spectator/plugin.cfg"));
        assert!(content.contains("res://addons/director/plugin.cfg"));
    }

    #[test]
    fn test_set_plugin_enabled_idempotent() {
        let dir = make_project("[editor_plugins]\nenabled=PackedStringArray()\n");
        set_plugin_enabled(
            dir.path(),
            "res://addons/spectator/plugin.cfg",
            true,
        )
        .unwrap();
        set_plugin_enabled(
            dir.path(),
            "res://addons/spectator/plugin.cfg",
            true,
        )
        .unwrap();
        let content = read_project_godot(dir.path()).unwrap();
        let count = content
            .matches("res://addons/spectator/plugin.cfg")
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_set_plugin_enabled_creates_section() {
        let dir = make_project("[application]\nconfig/name=\"MyGame\"\n");
        set_plugin_enabled(
            dir.path(),
            "res://addons/spectator/plugin.cfg",
            true,
        )
        .unwrap();
        let content = read_project_godot(dir.path()).unwrap();
        assert!(content.contains("[editor_plugins]"));
        assert!(content.contains("res://addons/spectator/plugin.cfg"));
    }

    #[test]
    fn test_set_autoload_add() {
        let dir = make_project("[autoload]\n");
        set_autoload(
            dir.path(),
            "SpectatorRuntime",
            "res://addons/spectator/runtime.gd",
        )
        .unwrap();
        let content = read_project_godot(dir.path()).unwrap();
        assert!(content.contains("SpectatorRuntime"));
        assert!(content.contains("*res://addons/spectator/runtime.gd"));
    }

    #[test]
    fn test_set_autoload_idempotent() {
        let dir = make_project("[autoload]\n");
        set_autoload(
            dir.path(),
            "SpectatorRuntime",
            "res://addons/spectator/runtime.gd",
        )
        .unwrap();
        set_autoload(
            dir.path(),
            "SpectatorRuntime",
            "res://addons/spectator/runtime.gd",
        )
        .unwrap();
        let content = read_project_godot(dir.path()).unwrap();
        let count = content.matches("SpectatorRuntime").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_set_autoload_creates_section() {
        let dir = make_project("[application]\nconfig/name=\"MyGame\"\n");
        set_autoload(
            dir.path(),
            "SpectatorRuntime",
            "res://addons/spectator/runtime.gd",
        )
        .unwrap();
        let content = read_project_godot(dir.path()).unwrap();
        assert!(content.contains("[autoload]"));
        assert!(content.contains("SpectatorRuntime"));
    }

    #[test]
    fn test_remove_autoload() {
        let dir = make_project(
            "[autoload]\nSpectatorRuntime=\"*res://addons/spectator/runtime.gd\"\n",
        );
        remove_autoload(dir.path(), "SpectatorRuntime").unwrap();
        let content = read_project_godot(dir.path()).unwrap();
        assert!(!content.contains("SpectatorRuntime"));
    }

    #[test]
    fn test_remove_autoload_noop() {
        let dir = make_project("[autoload]\n");
        // Should not error
        remove_autoload(dir.path(), "SpectatorRuntime").unwrap();
        let content = read_project_godot(dir.path()).unwrap();
        assert!(!content.contains("SpectatorRuntime"));
    }

    #[test]
    fn test_generate_mcp_json_default_port() {
        let spec_bin = Path::new("/home/user/.local/bin/spectator-server");
        let dir_bin = Path::new("/home/user/.local/bin/director");
        let json = generate_mcp_json(spec_bin, dir_bin, true, true, Some(9077));
        let servers = json["mcpServers"].as_object().unwrap();
        assert!(servers.contains_key("spectator"));
        assert!(servers.contains_key("director"));
        // No env for default port
        assert!(servers["spectator"].get("env").is_none());
    }

    #[test]
    fn test_generate_mcp_json_custom_port() {
        let spec_bin = Path::new("/home/user/.local/bin/spectator-server");
        let dir_bin = Path::new("/home/user/.local/bin/director");
        let json = generate_mcp_json(spec_bin, dir_bin, true, true, Some(9999));
        let spectator = &json["mcpServers"]["spectator"];
        assert!(spectator.get("env").is_some());
        assert_eq!(spectator["env"]["THEATRE_PORT"], "9999");
    }

    #[test]
    fn test_generate_mcp_json_spectator_only() {
        let spec_bin = Path::new("/home/user/.local/bin/spectator-server");
        let dir_bin = Path::new("/home/user/.local/bin/director");
        let json = generate_mcp_json(spec_bin, dir_bin, true, false, None);
        let servers = json["mcpServers"].as_object().unwrap();
        assert!(servers.contains_key("spectator"));
        assert!(!servers.contains_key("director"));
    }

    #[test]
    fn test_write_mcp_json_no_overwrite() {
        let dir = TempDir::new().unwrap();
        let existing = serde_json::json!({"old": true});
        std::fs::write(dir.path().join(".mcp.json"), "{\"old\": true}\n").unwrap();

        let new_content = serde_json::json!({"new": true});
        let written = write_mcp_json(dir.path(), &new_content, false).unwrap();
        assert!(!written);

        // File should still contain old content
        let content = std::fs::read_to_string(dir.path().join(".mcp.json")).unwrap();
        assert!(content.contains("old"));
        assert!(!content.contains("new"));

        drop(existing); // suppress unused warning
    }
}
