use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A single diagnostic parsed from Godot's stderr output.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GodotDiagnostic {
    pub file: String,
    pub line: u32,
    pub severity: String,
    pub message: String,
}

/// Parse Godot's stderr for error/warning lines.
///
/// Godot emits errors in the format:
///   `ERROR: res://path/file.gd:42 - Parse Error: Some message.`
///   `WARNING: res://path/file.gd:10 - Some warning message.`
///
/// Also handles the `modules/gdscript/gdscript.cpp` summary lines:
///   `ERROR: modules/gdscript/gdscript.cpp:2907 - Failed to load script "res://file.gd" ...`
///
/// Returns deduplicated diagnostics sorted by file then line (Godot often repeats errors).
pub fn parse_godot_stderr(stderr: &str) -> Vec<GodotDiagnostic> {
    // Matches: ERROR: res://path/file.gd:42 - message
    // or:      WARNING: res://path/file.gd:10 - message
    let script_re = regex::Regex::new(
        r"(?m)^(ERROR|WARNING): (res://[^\s:]+):(\d+) - (.+)$",
    )
    .expect("static regex is valid");

    // Matches the modules/gdscript summary line: extract res:// path from message body
    // ERROR: modules/gdscript/gdscript.cpp:2907 - Failed to load script "res://file.gd" ...
    let cpp_re = regex::Regex::new(
        r#"(?m)^ERROR: modules/gdscript/[^:]+:\d+ - Failed to load script "(res://[^"]+)""#,
    )
    .expect("static regex is valid");

    let mut seen: HashSet<(String, u32, String)> = HashSet::new();
    let mut diagnostics: Vec<GodotDiagnostic> = Vec::new();

    for cap in script_re.captures_iter(stderr) {
        let severity = cap[1].to_lowercase();
        let file = cap[2].trim_start_matches("res://").to_string();
        let line: u32 = cap[3].parse().unwrap_or(0);
        let message = cap[4].to_string();

        let key = (file.clone(), line, message.clone());
        if seen.insert(key) {
            diagnostics.push(GodotDiagnostic {
                file,
                line,
                severity,
                message,
            });
        }
    }

    // Handle modules/gdscript/gdscript.cpp summary lines — these reference the
    // failing script path inside the message body rather than in the source location.
    for cap in cpp_re.captures_iter(stderr) {
        let file = cap[1].trim_start_matches("res://").to_string();
        // Use line 0 as a sentinel — the summary line doesn't carry a script line number.
        let line: u32 = 0;
        let message = "Failed to load script".to_string();

        let key = (file.clone(), line, message.clone());
        if seen.insert(key) {
            diagnostics.push(GodotDiagnostic {
                file,
                line,
                severity: "error".to_string(),
                message,
            });
        }
    }

    // Sort by file path, then line number for deterministic output.
    diagnostics.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_script_error() {
        let stderr = r#"ERROR: res://scenes/game/grid/grid.gd:70 - Parse Error: Identifier "GameState" not declared in the current scope."#;
        let diags = parse_godot_stderr(stderr);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].file, "scenes/game/grid/grid.gd");
        assert_eq!(diags[0].line, 70);
        assert_eq!(diags[0].severity, "error");
        assert!(diags[0].message.contains("GameState"));
    }

    #[test]
    fn parse_warning() {
        let stderr = "WARNING: res://scripts/old.gd:5 - Unused variable 'x'.";
        let diags = parse_godot_stderr(stderr);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, "warning");
    }

    #[test]
    fn parse_gdscript_cpp_summary_line() {
        let stderr = r#"ERROR: modules/gdscript/gdscript.cpp:2907 - Failed to load script "res://debug/test_grid.gd" with error "Parse error"."#;
        let diags = parse_godot_stderr(stderr);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].file, "debug/test_grid.gd");
    }

    #[test]
    fn deduplicates_repeated_errors() {
        let stderr = "\
ERROR: res://foo.gd:10 - Parse Error: bad.\n\
ERROR: res://foo.gd:10 - Parse Error: bad.\n\
ERROR: res://foo.gd:10 - Parse Error: bad.";
        let diags = parse_godot_stderr(stderr);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn empty_stderr_returns_empty() {
        let diags = parse_godot_stderr("");
        assert!(diags.is_empty());
    }

    #[test]
    fn ignores_non_error_lines() {
        let stderr = "Godot Engine v4.6.1.stable.official\n\
[Stage] TCP server listening on 127.0.0.1:9077\n\
[Stage] TCP server stopped";
        let diags = parse_godot_stderr(stderr);
        assert!(diags.is_empty());
    }

    #[test]
    fn strips_res_prefix_from_file_path() {
        let stderr = "ERROR: res://path/to/script.gd:1 - Some error.";
        let diags = parse_godot_stderr(stderr);
        assert_eq!(diags[0].file, "path/to/script.gd");
    }

    #[test]
    fn sorts_by_file_then_line() {
        let stderr = "\
ERROR: res://b.gd:5 - err b5.\n\
ERROR: res://a.gd:10 - err a10.\n\
ERROR: res://a.gd:2 - err a2.";
        let diags = parse_godot_stderr(stderr);
        assert_eq!(diags[0].file, "a.gd");
        assert_eq!(diags[0].line, 2);
        assert_eq!(diags[1].file, "a.gd");
        assert_eq!(diags[1].line, 10);
        assert_eq!(diags[2].file, "b.gd");
    }
}
