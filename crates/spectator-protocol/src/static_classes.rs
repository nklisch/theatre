/// Godot node classes treated as static/non-dynamic by Spectator.
///
/// These classes represent world geometry, lighting, and environment — they
/// are always present and rarely change during gameplay. Spectator filters
/// and categorizes them separately from dynamic entities.
pub const STATIC_CLASSES: &[&str] = &[
    // 3D
    "StaticBody3D",
    "CSGShape3D",
    "CSGBox3D",
    "CSGCylinder3D",
    "CSGMesh3D",
    "CSGPolygon3D",
    "CSGSphere3D",
    "CSGTorus3D",
    "CSGCombiner3D",
    "MeshInstance3D",
    "GridMap",
    "WorldEnvironment",
    "DirectionalLight3D",
    "OmniLight3D",
    "SpotLight3D",
    // 2D
    "StaticBody2D",
    "TileMapLayer",
    "Sprite2D",
    "PointLight2D",
    "DirectionalLight2D",
];

/// Returns true if the given class name is treated as static.
pub fn is_static_class(class: &str) -> bool {
    STATIC_CLASSES.contains(&class)
}

/// Returns a category label for a static class (for summary output).
pub fn classify_static_category(class: &str) -> &'static str {
    match class {
        "StaticBody3D" | "StaticBody2D" => "collision",
        c if c.starts_with("CSG") => "csg",
        "GridMap" | "TileMapLayer" => "tilemap",
        "WorldEnvironment" => "environment",
        "MeshInstance3D" | "Sprite2D" => "visual",
        c if c.contains("Light") => "lights",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_body_2d_is_static() {
        assert!(is_static_class("StaticBody2D"));
    }

    #[test]
    fn tilemap_layer_is_static() {
        assert!(is_static_class("TileMapLayer"));
    }

    #[test]
    fn classify_2d_classes() {
        assert_eq!(classify_static_category("StaticBody2D"), "collision");
        assert_eq!(classify_static_category("TileMapLayer"), "tilemap");
        assert_eq!(classify_static_category("Sprite2D"), "visual");
        assert_eq!(classify_static_category("PointLight2D"), "lights");
    }
}
