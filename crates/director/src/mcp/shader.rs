use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A node in a VisualShader graph.
#[serde_with::skip_serializing_none]
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct VisualShaderNode {
    /// Unique integer ID for this node within the shader. IDs 0 and 1 are
    /// reserved (output node). Start custom nodes at 2+.
    pub node_id: i32,

    /// VisualShader node class name (e.g. "VisualShaderNodeInput",
    /// "VisualShaderNodeVectorOp", "VisualShaderNodeColorConstant").
    /// Must be a valid ClassDB class that extends VisualShaderNode.
    #[serde(rename = "type")]
    pub node_type: String,

    /// Which shader function graph this node belongs to.
    /// Valid values: "vertex", "fragment", "light".
    /// For particles mode: "start", "process", "collide".
    pub shader_function: String,

    /// Position in the visual shader editor graph (for layout).
    #[serde(default)]
    pub position: Option<[f64; 2]>,

    /// Properties to set on the node after creation.
    /// Type conversion is automatic (same as node_set_properties).
    #[serde(default)]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,
}

/// A connection between two nodes in a VisualShader graph.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct VisualShaderConnection {
    /// Source node ID.
    pub from_node: i32,

    /// Source port index on the from_node.
    pub from_port: i32,

    /// Destination node ID.
    pub to_node: i32,

    /// Destination port index on the to_node.
    pub to_port: i32,

    /// Which shader function graph this connection belongs to.
    /// Must match the shader_function of the connected nodes.
    pub shader_function: String,
}

/// Parameters for `visual_shader_create`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct VisualShaderCreateParams {
    /// Absolute path to the Godot project directory.
    pub project_path: String,

    /// Save path for the VisualShader resource (relative to project,
    /// e.g. "shaders/water.tres").
    pub resource_path: String,

    /// Shader processing mode. Valid values: "spatial" (3D), "canvas_item" (2D),
    /// "particles", "sky", "fog".
    pub shader_mode: String,

    /// Nodes to add to the shader graph. The output node (ID 0) exists
    /// automatically — do not include it. Node IDs must be unique and >= 2.
    pub nodes: Vec<VisualShaderNode>,

    /// Connections between nodes. Each connection links an output port on
    /// one node to an input port on another.
    #[serde(default)]
    pub connections: Vec<VisualShaderConnection>,
}
