/// Godot node classes treated as static/non-dynamic by Spectator.
///
/// These classes represent world geometry, lighting, and environment — they
/// are always present and rarely change during gameplay. Spectator filters
/// and categorizes them separately from dynamic entities.
pub const STATIC_CLASSES: &[&str] = &[
    "StaticBody3D",
    "StaticBody2D",
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
        "GridMap" => "gridmap",
        "WorldEnvironment" => "environment",
        "MeshInstance3D" => "mesh",
        c if c.contains("Light") => "lights",
        _ => "other",
    }
}
