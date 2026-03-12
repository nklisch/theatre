use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;
use spectator_protocol::query::{FindBy, GetSceneTreeParams, SceneTreeAction, TreeInclude};

use super::ParseMcpEnum;

/// Parameters for the scene_tree MCP tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SceneTreeToolParams {
    /// Action: "roots", "children", "subtree", "ancestors", "find"
    pub action: String,

    /// Node path — required for children, subtree, ancestors.
    pub node: Option<String>,

    /// Max recursion depth for subtree. Default: 3.
    #[serde(default = "default_depth")]
    pub depth: Option<u32>,

    /// For find: search by "name", "class", "group", or "script".
    pub find_by: Option<String>,

    /// For find: search value.
    pub find_value: Option<String>,

    /// What to include per node: "class", "groups", "script", "visible", "process_mode".
    /// Default: ["class", "groups"].
    #[serde(default = "default_include")]
    pub include: Option<Vec<String>>,

    /// Soft token budget override.
    pub token_budget: Option<u32>,
}

fn default_depth() -> Option<u32> {
    Some(3)
}

fn default_include() -> Option<Vec<String>> {
    Some(vec!["class".into(), "groups".into()])
}

impl super::ParseMcpEnum for SceneTreeAction {
    const FIELD_NAME: &'static str = "action";
    fn variants() -> &'static [(&'static str, Self)] {
        &[
            ("roots", SceneTreeAction::Roots),
            ("children", SceneTreeAction::Children),
            ("subtree", SceneTreeAction::Subtree),
            ("ancestors", SceneTreeAction::Ancestors),
            ("find", SceneTreeAction::Find),
        ]
    }
}

impl super::ParseMcpEnum for FindBy {
    const FIELD_NAME: &'static str = "find_by";
    fn variants() -> &'static [(&'static str, Self)] {
        &[
            ("name", FindBy::Name),
            ("class", FindBy::Class),
            ("group", FindBy::Group),
            ("script", FindBy::Script),
        ]
    }
}

impl super::ParseMcpEnum for TreeInclude {
    const FIELD_NAME: &'static str = "include";
    fn variants() -> &'static [(&'static str, Self)] {
        &[
            ("class", TreeInclude::Class),
            ("groups", TreeInclude::Groups),
            ("script", TreeInclude::Script),
            ("visible", TreeInclude::Visible),
            ("process_mode", TreeInclude::ProcessMode),
        ]
    }
}

/// Build the GetSceneTreeParams from MCP tool params.
pub fn build_scene_tree_params(
    params: &SceneTreeToolParams,
) -> Result<GetSceneTreeParams, McpError> {
    let action = SceneTreeAction::parse(&params.action)?;

    let default_inc = vec!["class".to_string(), "groups".to_string()];
    let include_strs = params.include.as_deref().unwrap_or(&default_inc);
    let include = TreeInclude::parse_list(include_strs)?;

    let find_by = params.find_by.as_deref().map(FindBy::parse).transpose()?;

    Ok(GetSceneTreeParams {
        action,
        node: params.node.clone(),
        depth: params.depth.unwrap_or(3),
        find_by,
        find_value: params.find_value.clone(),
        include,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_action_valid() {
        assert_eq!(
            SceneTreeAction::parse("roots").unwrap(),
            SceneTreeAction::Roots
        );
        assert_eq!(
            SceneTreeAction::parse("find").unwrap(),
            SceneTreeAction::Find
        );
    }

    #[test]
    fn parse_action_invalid() {
        assert!(SceneTreeAction::parse("invalid").is_err());
    }

    #[test]
    fn parse_find_by_valid() {
        assert_eq!(FindBy::parse("class").unwrap(), FindBy::Class);
        assert_eq!(FindBy::parse("group").unwrap(), FindBy::Group);
    }

    #[test]
    fn parse_tree_include_valid() {
        let inc = vec!["class".into(), "script".into()];
        let result = TreeInclude::parse_list(&inc).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn parse_tree_include_invalid() {
        let inc = vec!["invalid".into()];
        assert!(TreeInclude::parse_list(&inc).is_err());
    }
}
