use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use wait_timeout::ChildExt;

const IMPORT_TIMEOUT: Duration = Duration::from_secs(90);
const STDERR_LIMIT: usize = 32 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GodotSource {
    Explicit,
    GodotBin,
    GodotPath,
    Path,
}

#[derive(Debug)]
pub struct GodotResolution {
    pub path: Option<PathBuf>,
    pub source: Option<GodotSource>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum ImportOutcome {
    Success,
    Failed { status: i32, stderr: String },
    TimedOut { stderr: String },
}

pub fn resolve_godot(explicit: Option<&Path>) -> Result<GodotResolution> {
    if let Some(path) = explicit {
        validate_file(path, "--godot-bin")?;
        return Ok(GodotResolution {
            path: Some(path.to_path_buf()),
            source: Some(GodotSource::Explicit),
            warnings: Vec::new(),
        });
    }

    let mut warnings = Vec::new();
    for (name, source) in [
        ("GODOT_BIN", GodotSource::GodotBin),
        ("GODOT_PATH", GodotSource::GodotPath),
    ] {
        if let Some(value) = std::env::var_os(name) {
            let path = PathBuf::from(value);
            if path.is_file() {
                return Ok(GodotResolution {
                    path: Some(path),
                    source: Some(source),
                    warnings,
                });
            }
            warnings.push(format!(
                "{name} does not point to a file: {}",
                path.display()
            ));
        }
    }

    if let Ok(path) = which::which("godot") {
        return Ok(GodotResolution {
            path: Some(path),
            source: Some(GodotSource::Path),
            warnings,
        });
    }

    Ok(GodotResolution {
        path: None,
        source: None,
        warnings,
    })
}

fn validate_file(path: &Path, source: &str) -> Result<()> {
    if !path.is_file() {
        bail!(
            "{source} does not point to a Godot executable file: {}",
            path.display()
        );
    }
    Ok(())
}

pub fn import_project(godot: &Path, project: &Path) -> Result<ImportOutcome> {
    let mut child = Command::new(godot)
        .args(["--headless", "--editor", "--path"])
        .arg(project)
        .args(["--quit", "--verbose"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to start Godot at {}", godot.display()))?;

    let stderr = child
        .stderr
        .take()
        .context("Godot stderr pipe was unavailable")?;
    let stderr_reader = std::thread::spawn(move || read_tail(stderr, STDERR_LIMIT));

    let status = match child.wait_timeout(IMPORT_TIMEOUT) {
        Ok(Some(status)) => status,
        Ok(None) => {
            terminate_process_tree(&mut child);
            let stderr = stderr_reader.join().unwrap_or_default();
            return Ok(ImportOutcome::TimedOut { stderr });
        }
        Err(error) => {
            terminate_process_tree(&mut child);
            let _ = stderr_reader.join();
            return Err(error).context("Failed while waiting for the Godot import");
        }
    };

    let stderr = stderr_reader.join().unwrap_or_default();
    if status.success() {
        Ok(ImportOutcome::Success)
    } else {
        Ok(ImportOutcome::Failed {
            status: status.code().unwrap_or(-1),
            stderr,
        })
    }
}

fn terminate_process_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn read_tail(mut reader: impl Read, limit: usize) -> String {
    let mut retained = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => {
                retained.extend_from_slice(&buffer[..read]);
                if retained.len() > limit {
                    retained.drain(..retained.len() - limit);
                }
            }
        }
    }
    String::from_utf8_lossy(&retained).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_godot_path_wins() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let result = resolve_godot(Some(file.path())).unwrap();
        assert_eq!(result.path.as_deref(), Some(file.path()));
        assert_eq!(result.source, Some(GodotSource::Explicit));
    }

    #[test]
    fn explicit_godot_path_must_exist() {
        let missing = std::env::temp_dir().join("theatre-missing-godot-778899");
        assert!(resolve_godot(Some(&missing)).is_err());
    }

    #[test]
    fn read_tail_is_bounded() {
        let input = b"0123456789";
        assert_eq!(read_tail(&input[..], 4), "6789");
    }
}
