use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};
use clap::Args;
use serde_json::{Map, Value, json};

#[derive(Args)]
pub struct RunArgs {
    /// Saved scene relative to the Godot project
    scene: String,
    #[arg(long, default_value = ".")]
    project: PathBuf,
    /// Restart instead of start (stops the editor's current game)
    #[arg(long)]
    restart: bool,
    /// Window presentation on Windows; omission preserves legacy editor behavior
    #[arg(long, value_parser = ["automated", "deferred", "interactive"])]
    presentation: Option<String>,
    #[arg(long, value_parser = ["on", "off"])]
    observe: Option<String>,
    #[arg(long, value_parser = ["human", "agent"])]
    operator: Option<String>,
    #[arg(long, value_parser = ["project", "scene"])]
    readiness: Option<String>,
    #[arg(long, value_parser = ["manual", "ready"])]
    play_start: Option<String>,
    #[arg(long, value_parser = ["off", "minimal", "light", "standard", "heavy"])]
    startup: Option<String>,
    #[arg(long, value_parser = ["off", "minimal", "light", "standard", "heavy"])]
    play: Option<String>,
    #[arg(long, value_parser = ["on", "off"])]
    startup_images: Option<String>,
    #[arg(long, value_parser = ["on", "off"])]
    play_images: Option<String>,
    #[arg(long)]
    startup_scope: Vec<String>,
    #[arg(long)]
    play_scope: Vec<String>,
    #[arg(long, value_parser = ["session", "rolling"])]
    play_retention: Option<String>,
    #[arg(long, value_parser = ["manual", "on_stop", "on_trigger"])]
    play_save: Option<String>,
    #[arg(long, value_parser = ["manual", "on_stop"])]
    startup_save: Option<String>,
    /// Additional typed LaunchOptions JSON; named flags override its fields
    #[arg(long)]
    options: Option<String>,
}

fn options(args: &RunArgs) -> Result<Value> {
    let typed: stage_protocol::capture::LaunchOptions =
        serde_json::from_str(args.options.as_deref().unwrap_or("{}"))
            .context("Invalid --options LaunchOptions JSON")?;
    let mut result = serde_json::to_value(typed)?;
    for (name, value) in [
        ("operator", &args.operator),
        ("readiness", &args.readiness),
        ("play_start", &args.play_start),
    ] {
        if let Some(value) = value {
            result[name] = json!(value);
        }
    }
    if let Some(observe) = &args.observe {
        result["observe"] = json!(observe == "on");
    }
    for (name, preset, images, scope, save) in [
        (
            "startup",
            &args.startup,
            &args.startup_images,
            &args.startup_scope,
            &args.startup_save,
        ),
        (
            "play",
            &args.play,
            &args.play_images,
            &args.play_scope,
            &args.play_save,
        ),
    ] {
        if result.get(name).is_none() {
            result[name] = Value::Object(Map::new());
        }
        if let Some(preset) = preset {
            result[name]["preset"] = json!(preset);
        }
        if let Some(images) = images {
            result[name]["images"] = json!(images == "on");
        }
        if !scope.is_empty() {
            result[name]["scope"] = json!(scope);
        }
        if let Some(save) = save {
            result[name]["save"] = json!(save);
        }
    }
    if let Some(retention) = &args.play_retention {
        result["play"]["retention"] = json!(retention);
    }
    for phase in ["startup", "play"] {
        if result[phase]
            .as_object()
            .is_some_and(|fields| fields.is_empty())
        {
            result
                .as_object_mut()
                .expect("serialized options object")
                .remove(phase);
        }
    }
    Ok(result)
}

fn editor_run_params(args: &RunArgs, project: &std::path::Path, launch: Value) -> Value {
    let mut params = json!({
        "project_path": project,
        "scene_path": args.scene,
        "action": if args.restart { "restart" } else { "start" },
        "launch": launch,
    });
    if let Some(presentation) = &args.presentation {
        params["presentation"] = json!(presentation);
    }
    params
}

pub fn run(args: RunArgs) -> Result<()> {
    crate::project::validate_project(&args.project)?;
    let launch = options(&args)?;
    let project = args
        .project
        .canonicalize()
        .context("Cannot resolve project directory")?;
    // Director owns the existing editor lifecycle. This command does not install,
    // open an editor, change preferences, or create another process supervisor.
    let executable = which::which("director").context("director is not on PATH. Ask the user before installing Theatre; otherwise add its existing installation to PATH.")?;
    let params = editor_run_params(&args, &project, launch);
    let mut child = Command::new(executable)
        .arg("editor_run")
        .arg(params.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("Cannot invoke Director")?;
    // Drain while waiting: a valid resolved policy can exceed a Windows pipe's
    // capacity. Bound retained output but continue draining to avoid deadlock.
    let mut stdout = child
        .stdout
        .take()
        .context("Director stdout was not piped")?;
    let reader = std::thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut retained = Vec::new();
        let mut buffer = [0; 8192];
        let mut oversized = false;
        loop {
            let count = stdout.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            if retained.len() + count <= 1024 * 1024 {
                retained.extend_from_slice(&buffer[..count]);
            } else {
                oversized = true;
            }
        }
        if oversized {
            return Err(std::io::Error::other("Director response exceeds 1 MiB"));
        }
        Ok(retained)
    });
    use wait_timeout::ChildExt;
    let status = child.wait_timeout(std::time::Duration::from_secs(60))?;
    let Some(status) = status else {
        let _ = child.kill();
        let _ = child.wait();
        bail!("Director launch request timed out; inspect the editor before retrying");
    };
    let output = reader
        .join()
        .map_err(|_| anyhow::anyhow!("Director output reader failed"))??;
    std::io::stdout().write_all(&output)?;
    if !status.success() {
        bail!("Director could not launch the scene; no fallback process was started");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn ordinary_run_does_not_request_stage_installation() {
        let crate::Command::Run(args) =
            crate::Cli::parse_from(["theatre", "run", "review.tscn"]).command
        else {
            panic!("run");
        };
        assert_eq!(options(&args).unwrap(), json!({}));
        let params = editor_run_params(&args, std::path::Path::new("/project"), json!({}));
        assert!(params.get("presentation").is_none());
    }

    #[test]
    fn launch_flags_are_independent_and_do_not_imply_observation() {
        let crate::Command::Run(args) = crate::Cli::parse_from([
            "theatre",
            "run",
            "review.tscn",
            "--startup",
            "minimal",
            "--play",
            "heavy",
            "--play-images",
            "off",
            "--operator",
            "human",
        ])
        .command
        else {
            panic!("run");
        };
        let result = options(&args).unwrap();
        assert_eq!(result["startup"]["preset"], "minimal");
        assert_eq!(result["play"]["images"], false);
        assert!(result.get("observe").is_none());
    }

    #[test]
    fn presentation_is_forwarded_only_when_explicit() {
        let crate::Command::Run(args) = crate::Cli::parse_from([
            "theatre",
            "run",
            "review.tscn",
            "--presentation",
            "deferred",
        ])
        .command
        else {
            panic!("run");
        };
        let params = editor_run_params(&args, std::path::Path::new("/project"), json!({}));
        assert_eq!(params["presentation"], "deferred");
    }
}
