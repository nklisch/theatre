use std::time::Duration;

use crate::harness::GodotFixture;
use serde_json::{Value, json};

fn call(fixture: &mut GodotFixture, method: &str, args: Value) {
    fixture
        .query(
            "execute_action",
            json!({
                "action": "call_method",
                "path": ".",
                "method": method,
                "args": args,
            }),
        )
        .unwrap()
        .unwrap_data();
}

fn diagnostics(fixture: &mut GodotFixture) -> Value {
    fixture
        .query("runtime_diagnostics", json!({}))
        .unwrap()
        .unwrap_data()
}

fn wait_for_message(fixture: &mut GodotFixture, needle: &str) -> Value {
    let mut last = Value::Null;
    for _ in 0..40 {
        let result = diagnostics(fixture);
        if result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["message"].as_str().unwrap_or("").contains(needle))
        {
            return result;
        }
        last = result;
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("timed out waiting for diagnostic containing {needle:?}; last response: {last}");
}

#[test]
#[ignore = "requires Godot 4.7 and built GDExtension"]
fn logger_captures_bounded_current_run_across_threads_reconnect_and_restart() {
    let mut first = GodotFixture::start("test_scene_3d.tscn").unwrap();
    let first_identity = first.handshake.identity.clone();

    call(&mut first, "emit_runtime_diagnostic_basics", json!([]));
    let basics = wait_for_message(&mut first, "deliberate warning");
    assert_eq!(basics["identity"], json!(first_identity));
    assert!(basics["available"].as_bool().unwrap());
    assert!(
        basics["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| {
                entry["kind"] == "error"
                    && entry["message"]
                        .as_str()
                        .unwrap_or("")
                        .contains("deliberate error")
            })
    );

    call(&mut first, "emit_runtime_script_error", json!([]));
    let script = wait_for_message(&mut first, "Out of bounds");
    let script_error = script["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["kind"] == "script_error")
        .expect("runtime type failure must be classified as script_error");
    assert!(
        script_error["origin"]["file"]
            .as_str()
            .unwrap_or("")
            .ends_with("test_scene_3d.gd")
    );
    assert!(script_error["origin"]["line"].as_i64().unwrap() > 0);

    call(&mut first, "emit_worker_runtime_diagnostic", json!([]));
    wait_for_message(&mut first, "worker error");

    call(&mut first, "emit_runtime_diagnostic_overflow", json!([140]));
    let overflow = wait_for_message(&mut first, "overflow 139");
    assert_eq!(overflow["retained_count"], 128);
    assert!(overflow["omitted_count"].as_u64().unwrap() >= 12);
    assert_eq!(overflow["limits"]["queue_capacity"], 128);
    assert_eq!(overflow["limits"]["backtrace_max_frames"], 16);

    let (port, process) = first.disconnect_keep_alive();
    std::thread::sleep(Duration::from_millis(250));
    let mut reconnected = GodotFixture::reconnect(port, process).unwrap();
    let after_reconnect = diagnostics(&mut reconnected);
    assert_eq!(after_reconnect["identity"], json!(first_identity));
    assert!(
        after_reconnect["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| {
                entry["message"]
                    .as_str()
                    .unwrap_or("")
                    .contains("overflow 139")
            })
    );

    drop(reconnected);
    let mut restarted = GodotFixture::start("test_scene_3d.tscn").unwrap();
    let after_restart = diagnostics(&mut restarted);
    assert_ne!(after_restart["identity"], json!(first_identity));
    assert!(
        !after_restart["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| {
                entry["message"]
                    .as_str()
                    .unwrap_or("")
                    .contains("stage-runtime-diagnostics")
            })
    );
}
