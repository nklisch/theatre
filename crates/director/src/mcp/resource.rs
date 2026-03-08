use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for `resource_read`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ResourceReadParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Resource file to read (relative to project, e.g. "materials/ground.tres").
    pub resource_path: String,
}

/// Parameters for `material_create`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct MaterialCreateParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Save path for the material relative to project (e.g. "materials/ground.tres").
    pub resource_path: String,

    /// Material class name. Common types: StandardMaterial3D, ORMMaterial3D,
    /// ShaderMaterial, CanvasItemMaterial, ParticleProcessMaterial.
    /// Any ClassDB Material subclass is accepted.
    pub material_type: String,

    /// Optional properties to set on the material after creation.
    /// Type conversion is automatic (Color from "#ff0000" or {"r":1,"g":0,"b":0}).
    #[serde(default)]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,

    /// For ShaderMaterial: path to a .gdshader file (relative to project).
    /// Loaded and assigned as the shader property.
    #[serde(default)]
    pub shader_path: Option<String>,
}

/// Parameters for `shape_create`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ShapeCreateParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Shape class name. 3D: BoxShape3D, SphereShape3D, CapsuleShape3D,
    /// CylinderShape3D, ConcavePolygonShape3D, ConvexPolygonShape3D,
    /// WorldBoundaryShape3D, SeparationRayShape3D, HeightMapShape3D.
    /// 2D: CircleShape2D, RectangleShape2D, CapsuleShape2D, SegmentShape2D,
    /// ConvexPolygonShape2D, ConcavePolygonShape2D, WorldBoundaryShape2D,
    /// SeparationRayShape2D.
    pub shape_type: String,

    /// Shape configuration. Keys are property names on the shape resource
    /// (e.g. "radius", "size", "height"). Type conversion is automatic.
    #[serde(default)]
    pub shape_params: Option<serde_json::Map<String, serde_json::Value>>,

    /// Save the shape as a .tres file at this path (relative to project).
    #[serde(default)]
    pub save_path: Option<String>,

    /// Attach the shape to a CollisionShape2D/3D node in a scene.
    /// Requires scene_path and node_path to also be set.
    /// The shape is assigned to the node's "shape" property.
    #[serde(default)]
    pub scene_path: Option<String>,

    /// Path to the CollisionShape node within the scene tree.
    /// Required when scene_path is set.
    #[serde(default)]
    pub node_path: Option<String>,
}

/// Parameters for `style_box_create`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct StyleBoxCreateParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Save path for the StyleBox relative to project (e.g. "ui/panel.tres").
    pub resource_path: String,

    /// StyleBox class name: StyleBoxFlat, StyleBoxTexture, StyleBoxLine,
    /// or StyleBoxEmpty.
    pub style_type: String,

    /// Optional properties to set on the StyleBox after creation.
    #[serde(default)]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Parameters for `resource_duplicate`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ResourceDuplicateParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Path to the source resource file (relative to project).
    pub source_path: String,

    /// Path where the duplicate will be saved (relative to project).
    pub dest_path: String,

    /// Property overrides to apply after duplication. Keys are property names.
    #[serde(default)]
    pub property_overrides: Option<serde_json::Map<String, serde_json::Value>>,

    /// Deep copy sub-resources (making them independent). Default: false
    /// (shallow copy — sub-resources are shared references).
    #[serde(default)]
    pub deep_copy: Option<bool>,
}
