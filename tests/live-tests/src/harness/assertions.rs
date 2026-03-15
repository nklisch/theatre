#![allow(dead_code)]
use serde_json::Value;

pub fn assert_pos_approx(actual: &[f64], expected: &[f64], tolerance: f64, label: &str) {
    assert_eq!(actual.len(), expected.len(), "{label}: dimension mismatch");
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        assert!(
            (a - e).abs() < tolerance,
            "{label}: component {i} — expected ~{e}, got {a} (tolerance {tolerance})"
        );
    }
}

pub fn extract_position(entity: &Value) -> Vec<f64> {
    entity["global_position"]
        .as_array()
        .expect("entity should have global_position")
        .iter()
        .map(|v| v.as_f64().expect("position component should be f64"))
        .collect()
}

pub fn find_entity<'a>(entities: &'a [Value], name: &str) -> &'a Value {
    entities
        .iter()
        .find(|e| {
            e["path"]
                .as_str()
                .map(|p| p.contains(name))
                .unwrap_or(false)
        })
        .unwrap_or_else(|| {
            let paths: Vec<&str> = entities
                .iter()
                .filter_map(|e| e["path"].as_str())
                .collect();
            panic!("Entity containing '{name}' not found. Available: {paths:?}");
        })
}
