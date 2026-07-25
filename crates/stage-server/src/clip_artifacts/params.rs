//! Canonical visual-artifact parameter normalization.
use serde_json::Value;
pub fn canonical(value: &Value) -> Value {
    value.clone()
}
