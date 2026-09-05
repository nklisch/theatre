use anyhow::Result;
use clap::Args;
use std::{io::Write, path::PathBuf};

#[derive(Args)]
pub struct FeedbackArgs {
    /// Selected Godot project; retained evidence works without an engine.
    #[arg(long, default_value = ".")]
    project: PathBuf,
    /// Operation JSON, e.g. '{"action":"status"}'.
    #[arg(default_value = "{\"action\":\"status\"}")]
    params: String,
}

pub fn run(args: FeedbackArgs) -> Result<()> {
    let result = serde_json::from_str::<theatre_feedback::Operation>(&args.params)
        .map_err(|e| e.to_string())
        .and_then(|operation| {
            theatre_feedback::Queue::open(&args.project)
                .and_then(|queue| queue.execute(operation))
                .map_err(|e| e.to_string())
        });
    let (mut value, code) = match result {
        Ok(response) => (serde_json::to_value(response)?, 0),
        Err(message) => (
            serde_json::json!({"error": "feedback_error", "message": message}),
            1,
        ),
    };
    theatre_feedback::append_json_notice(&mut value, &args.project);
    writeln!(std::io::stdout(), "{value}")?;
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

/// Both native plugins document this synchronous PostToolUse text envelope.
/// A hook never connects to Godot, changes handling, or pretends text is an image.
pub fn hook() -> Result<()> {
    // Deserialize only routing fields; large tool results are skipped rather than
    // retained or truncated into invalid JSON that would lose the pending notice.
    let event = serde_json::from_reader(std::io::stdin().lock()).ok();
    // An explicit selection is authoritative. Falling back after a bad selection
    // could surface feedback from a different project than the one the user chose.
    let selected_project = std::env::var_os("THEATRE_PROJECT_DIR").map(PathBuf::from);
    let output = hook_output(event, selected_project.as_deref());
    writeln!(std::io::stdout(), "{output}")?;
    Ok(())
}

#[derive(serde::Deserialize)]
struct HookEvent {
    hook_event_name: String,
    cwd: PathBuf,
}

fn hook_output(
    event: Option<HookEvent>,
    selected_project: Option<&std::path::Path>,
) -> serde_json::Value {
    let Some(event) = event else {
        return serde_json::json!({});
    };
    if event.hook_event_name != "PostToolUse" {
        return serde_json::json!({});
    }
    let project = if let Some(project) = selected_project {
        if project.as_os_str().is_empty() || !project.join("project.godot").is_file() {
            return serde_json::json!({});
        }
        project
    } else {
        let Some(project) = event
            .cwd
            .ancestors()
            .find(|p| p.join("project.godot").is_file())
        else {
            return serde_json::json!({});
        };
        project
    };
    let Some(notice) = theatre_feedback::pending_notice(project) else {
        return serde_json::json!({});
    };
    serde_json::json!({"hookSpecificOutput": {"hookEventName": "PostToolUse", "additionalContext": notice}})
}

#[cfg(test)]
mod tests {
    #[test]
    fn unrelated_or_malformed_events_are_quiet() {
        for input in ["", "{}", "{\"hook_event_name\":\"Stop\",\"cwd\":\".\"}"] {
            assert_eq!(
                super::hook_output(serde_json::from_str(input).ok(), None),
                serde_json::json!({})
            );
        }
    }
}
