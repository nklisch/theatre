use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;
use spectator_protocol::query::{FindBy, GetSceneTreeParams, SceneTreeAction, TreeInclude};

/// Parameters for the scene_tree MCP tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SceneTreeToolParams {
    /// Scene tree action to perform.
    pub action: SceneTreeAction,

    /// Node path — required for children, subtree, ancestors.
    pub node: Option<String>,

    /// Max recursion depth for subtree. Default: 3.
    #[serde(default = "default_depth")]
    pub depth: Option<u32>,

    /// For find: criterion to search by.
    pub find_by: Option<FindBy>,

    /// For find: search value.
    pub find_value: Option<String>,

    /// What to include per node. Default: [class, groups].
    #[serde(default = "default_include")]
    pub include: Option<Vec<TreeInclude>>,

    /// Soft token budget override.
    pub token_budget: Option<u32>,
}

fn default_depth() -> Option<u32> {
    Some(3)
}

fn default_include() -> Option<Vec<TreeInclude>> {
    Some(vec![TreeInclude::Class, TreeInclude::Groups])
}

/// Build the GetSceneTreeParams from MCP tool params.
pub fn build_scene_tree_params(
    params: &SceneTreeToolParams,
) -> Result<GetSceneTreeParams, McpError> {
    let include = params
        .include
        .as_deref()
        .unwrap_or(&[TreeInclude::Class, TreeInclude::Groups])
        .to_vec();

    Ok(GetSceneTreeParams {
        action: params.action,
        node: params.node.clone(),
        depth: params.depth.unwrap_or(3),
        find_by: params.find_by,
        find_value: params.find_value.clone(),
        include,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_deserialize_valid() {
        let a: SceneTreeAction = serde_json::from_str(r#""roots""#).unwrap();
        assert_eq!(a, SceneTreeAction::Roots);
        let a: SceneTreeAction = serde_json::from_str(r#""find""#).unwrap();
        assert_eq!(a, SceneTreeAction::Find);
    }

    #[test]
    fn action_deserialize_invalid() {
        assert!(serde_json::from_str::<SceneTreeAction>(r#""invalid""#).is_err());
    }

    #[test]
    fn find_by_deserialize_valid() {
        let f: FindBy = serde_json::from_str(r#""class""#).unwrap();
        assert_eq!(f, FindBy::Class);
        let f: FindBy = serde_json::from_str(r#""group""#).unwrap();
        assert_eq!(f, FindBy::Group);
    }

    #[test]
    fn tree_include_deserialize_valid() {
        let i: TreeInclude = serde_json::from_str(r#""script""#).unwrap();
        assert_eq!(i, TreeInclude::Script);
    }

    #[test]
    fn tree_include_deserialize_invalid() {
        assert!(serde_json::from_str::<TreeInclude>(r#""invalid""#).is_err());
    }
}
