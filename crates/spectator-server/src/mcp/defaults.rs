/// Default serde functions shared by snapshot and delta parameter structs.

pub fn default_perspective() -> String {
    "camera".to_string()
}

pub fn default_radius() -> f64 {
    50.0
}

pub fn default_detail() -> String {
    "standard".to_string()
}
