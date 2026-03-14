//! Default serde functions shared by snapshot, delta, and query parameter structs.

pub fn default_radius() -> f64 {
    50.0
}

pub fn default_k() -> usize {
    5
}

pub fn default_query_radius() -> f64 {
    20.0
}
