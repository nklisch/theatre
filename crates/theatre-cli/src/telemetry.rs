//! Minimal, privacy-respecting install telemetry.
//!
//! Sends a single anonymous event ("install") to Google Analytics 4
//! via the Measurement Protocol. No user-identifiable data is collected —
//! only the event name, Theatre version, and OS/arch.
//!
//! Opt-out: Set any of these environment variables to disable telemetry:
//!   - DO_NOT_TRACK=1        (https://consoledonottrack.com/)
//!   - THEATRE_NO_TELEMETRY=1
//!   - CI=true               (most CI systems set this)

use std::thread;
use std::time::Duration;

const GA_MEASUREMENT_ID: &str = "G-QDTG6Z9L05";
/// Set THEATRE_GA_API_SECRET at build time to enable install telemetry.
/// Without it, record_install() is a no-op.
const GA_API_SECRET: Option<&str> = option_env!("THEATRE_GA_API_SECRET");
const GA_ENDPOINT: &str = "https://www.google-analytics.com/mp/collect";

/// Returns `true` if telemetry is disabled by environment variables.
fn is_opted_out() -> bool {
    opted_out_from(|name| std::env::var(name).ok())
}

fn opted_out_from(lookup: impl Fn(&str) -> Option<String>) -> bool {
    for var in ["DO_NOT_TRACK", "THEATRE_NO_TELEMETRY", "CI"] {
        if let Some(val) = lookup(var) {
            let v = val.trim().to_lowercase();
            if v == "1" || v == "true" || v == "yes" {
                return true;
            }
        }
    }
    false
}

/// Fire-and-forget: send an anonymous install event.
/// Spawns a background thread so it never blocks the CLI.
/// All errors are silently ignored. No-op if the GA API secret
/// was not set at build time.
pub fn record_install() {
    if GA_API_SECRET.is_none() || is_opted_out() {
        return;
    }

    thread::spawn(|| {
        let _ = send_install_event();
    });

    // Give the background thread a brief window to fire the request.
    // If it takes longer than this, we move on — it's best-effort.
    thread::sleep(Duration::from_millis(500));
}

fn send_install_event() -> Result<(), Box<dyn std::error::Error>> {
    let version = env!("CARGO_PKG_VERSION");
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let body = serde_json::json!({
        "client_id": "anonymous",
        "non_personalized_ads": true,
        "events": [{
            "name": "install",
            "params": {
                "theatre_version": version,
                "os": os,
                "arch": arch,
            }
        }]
    });

    let api_secret = GA_API_SECRET.unwrap_or("");
    let url = format!(
        "{}?measurement_id={}&api_secret={}",
        GA_ENDPOINT, GA_MEASUREMENT_ID, api_secret
    );

    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(3)))
        .build()
        .new_agent();

    agent
        .post(&url)
        .header("Content-Type", "application/json")
        .send(body.to_string().as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognized_opt_out_values_are_checked_for_each_environment_key() {
        for key in ["DO_NOT_TRACK", "THEATRE_NO_TELEMETRY", "CI"] {
            for (value, expected) in [
                ("1", true),
                (" true ", true),
                ("YES", true),
                ("0", false),
                ("false", false),
                ("", false),
            ] {
                assert_eq!(
                    opted_out_from(|name| (name == key).then(|| value.to_owned())),
                    expected,
                    "{key}={value}"
                );
            }
        }
    }

    #[test]
    fn not_opted_out_without_environment_configuration() {
        assert!(!opted_out_from(|_| None));
    }
}
