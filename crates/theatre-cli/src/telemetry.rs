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
    for var in ["DO_NOT_TRACK", "THEATRE_NO_TELEMETRY", "CI"] {
        if let Ok(val) = std::env::var(var) {
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

    // SAFETY: These tests manipulate env vars which is unsafe in Rust 2024.
    // Each test runs in its own process via `cargo test`, and these env vars
    // are not read by other threads during the test.

    #[test]
    fn opted_out_with_do_not_track() {
        unsafe { std::env::set_var("DO_NOT_TRACK", "1") };
        assert!(is_opted_out());
        unsafe { std::env::remove_var("DO_NOT_TRACK") };
    }

    #[test]
    fn opted_out_with_theatre_env() {
        unsafe { std::env::set_var("THEATRE_NO_TELEMETRY", "true") };
        assert!(is_opted_out());
        unsafe { std::env::remove_var("THEATRE_NO_TELEMETRY") };
    }

    #[test]
    fn opted_out_with_ci() {
        unsafe { std::env::set_var("CI", "true") };
        assert!(is_opted_out());
        unsafe { std::env::remove_var("CI") };
    }

    #[test]
    fn not_opted_out_by_default() {
        let saved: Vec<_> = ["DO_NOT_TRACK", "THEATRE_NO_TELEMETRY", "CI"]
            .iter()
            .map(|k| (*k, std::env::var(k).ok()))
            .collect();
        for (k, _) in &saved {
            unsafe { std::env::remove_var(k) };
        }

        assert!(!is_opted_out());

        for (k, v) in saved {
            if let Some(val) = v {
                unsafe { std::env::set_var(k, val) };
            }
        }
    }
}
