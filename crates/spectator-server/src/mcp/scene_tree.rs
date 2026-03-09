use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;
use spectator_protocol::query::{FindBy, GetSceneTreeParams, SceneTreeAction, TreeInclude};

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

pub fn parse_action(s: &str) -> Result<SceneTreeAction, McpError> {
    super::parse_enum_param(
        s,
        "action",
        &[
            ("roots", SceneTreeAction::Roots),
            ("children", SceneTreeAction::Children),
            ("subtree", SceneTreeAction::Subtree),
            ("ancestors", SceneTreeAction::Ancestors),
            ("find", SceneTreeAction::Find),
        ],
    )
}

pub fn parse_find_by(s: &str) -> Result<FindBy, McpError> {
    super::parse_enum_param(
        s,
        "find_by",
        &[
            ("name", FindBy::Name),
            ("class", FindBy::Class),
            ("group", FindBy::Group),
            ("script", FindBy::Script),
        ],
    )
}

pub fn parse_tree_include(strings: &[String]) -> Result<Vec<TreeInclude>, McpError> {
    super::parse_enum_list(
        strings,
        "include",
        &[
            ("class", TreeInclude::Class),
            ("groups", TreeInclude::Groups),
            ("script", TreeInclude::Script),
            ("visible", TreeInclude::Visible),
            ("process_mode", TreeInclude::ProcessMode),
        ],
    )
}

/// Build the GetSceneTreeParams from MCP tool params.
pub fn build_scene_tree_params(
    params: &SceneTreeToolParams,
) -> Result<GetSceneTreeParams, McpError> {
    let action = parse_action(&params.action)?;

    let default_inc = vec!["class".to_string(), "groups".to_string()];
    let include_strs = params.include.as_deref().unwrap_or(&default_inc);
    let include = parse_tree_include(include_strs)?;

    let find_by = params.find_by.as_deref().map(parse_find_by).transpose()?;

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
        assert_eq!(parse_action("roots").unwrap(), SceneTreeAction::Roots);
        assert_eq!(parse_action("find").unwrap(), SceneTreeAction::Find);
    }

    #[test]
    fn parse_action_invalid() {
        assert!(parse_action("invalid").is_err());
    }

    #[test]
    fn parse_find_by_valid() {
        assert_eq!(parse_find_by("class").unwrap(), FindBy::Class);
        assert_eq!(parse_find_by("group").unwrap(), FindBy::Group);
    }

    #[test]
    fn parse_tree_include_valid() {
        let inc = vec!["class".into(), "script".into()];
        let result = parse_tree_include(&inc).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn parse_tree_include_invalid() {
        let inc = vec!["invalid".into()];
        assert!(parse_tree_include(&inc).is_err());
    }
}
