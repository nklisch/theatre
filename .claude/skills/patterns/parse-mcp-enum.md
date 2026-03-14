# Pattern: ParseMcpEnum Trait

Standardized enum parsing from MCP string parameters. Every enum accepted as a string in an MCP tool parameter implements `ParseMcpEnum` with a `FIELD_NAME` constant and a `variants()` static slice. The default `parse()` / `parse_list()` methods generate a consistent error message on unknown values.

## Rationale

MCP tool parameters arrive as JSON strings. Without a standard approach each handler has ad-hoc match logic that produces inconsistent error messages. `ParseMcpEnum` centralises parsing and produces uniform `McpError::invalid_params` messages that include the field name and the set of allowed values.

## Examples

### Example 1: Two enums for spatial_config (cluster_by, bearing_format)
**File**: `crates/stage-server/src/mcp/config.rs:13-33`
```rust
impl super::ParseMcpEnum for ClusterStrategy {
    const FIELD_NAME: &'static str = "cluster_by";
    fn variants() -> &'static [(&'static str, Self)] {
        &[
            ("group",     ClusterStrategy::Group),
            ("class",     ClusterStrategy::Class),
            ("proximity", ClusterStrategy::Proximity),
            ("none",      ClusterStrategy::None),
        ]
    }
}

impl super::ParseMcpEnum for BearingFormat {
    const FIELD_NAME: &'static str = "bearing_format";
    fn variants() -> &'static [(&'static str, Self)] {
        &[
            ("cardinal", BearingFormat::Cardinal),
            ("degrees",  BearingFormat::Degrees),
            ("both",     BearingFormat::Both),
        ]
    }
}
```

### Example 2: List parsing for InspectCategory (include field accepts multiple values)
**File**: `crates/stage-server/src/mcp/inspect.rs:35-48`
```rust
impl super::ParseMcpEnum for InspectCategory {
    const FIELD_NAME: &'static str = "include";
    fn variants() -> &'static [(&'static str, Self)] {
        &[
            ("transform",       InspectCategory::Transform),
            ("physics",         InspectCategory::Physics),
            ("state",           InspectCategory::State),
            ("children",        InspectCategory::Children),
            ("signals",         InspectCategory::Signals),
            ("script",          InspectCategory::Script),
            ("spatial_context", InspectCategory::SpatialContext),
            ("resources",       InspectCategory::Resources),
        ]
    }
}

// Usage: parse a Vec<String> parameter into Vec<InspectCategory>
let include = InspectCategory::parse_list(&params.include.unwrap_or_default())?;
```

### Example 3: Three enums for scene_tree (action, find_by, include)
**File**: `crates/stage-server/src/mcp/scene_tree.rs:44-79`
```rust
impl super::ParseMcpEnum for SceneTreeAction {
    const FIELD_NAME: &'static str = "action";
    fn variants() -> &'static [(&'static str, Self)] {
        &[
            ("roots",    SceneTreeAction::Roots),
            ("children", SceneTreeAction::Children),
            ("find",     SceneTreeAction::Find),
            ("path",     SceneTreeAction::Path),
        ]
    }
}
```

### Trait definition (for reference)
**File**: `crates/stage-server/src/mcp/mod.rs:107-118`
```rust
pub(crate) trait ParseMcpEnum: Sized + Clone + 'static {
    const FIELD_NAME: &'static str;
    fn variants() -> &'static [(&'static str, Self)];

    fn parse(s: &str) -> Result<Self, McpError> {
        parse_enum_param(s, Self::FIELD_NAME, Self::variants())
    }

    fn parse_list(values: &[String]) -> Result<Vec<Self>, McpError> {
        parse_enum_list(values, Self::FIELD_NAME, Self::variants())
    }
}
```

## When to Use

- Any enum type that arrives as a string in an MCP tool parameter
- Use `parse()` for single-value fields, `parse_list()` for `Vec<String>` fields

## When NOT to Use

- Enums that are serde-deserialized directly from the parameter struct (handle via `#[serde(rename_all)]` instead)
- Enums internal to the server that never cross the MCP boundary

## Common Violations

- Matching string literals inline in a handler instead of implementing the trait — produces inconsistent errors
- Forgetting to include all variants in the `variants()` slice — silently rejects valid inputs
