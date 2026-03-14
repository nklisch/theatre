# Design: Milestone 5 — Configuration

## Overview

M5 delivers the `spatial_config` MCP tool and a three-tier configuration system: session config (MCP tool, highest priority), project file (`stage.toml`, mid priority), and Godot Project Settings (lowest priority). The agent can configure static node classification, state property tracking, clustering strategy, bearing format, internal variable exposure, poll interval, and token hard cap — all at runtime.

**Depends on:** M1 (snapshot, budget, static classification, clustering)

**Exit Criteria:** Agent calls `spatial_config(static_patterns: ["walls/*"], state_properties: { enemies: ["health"] })` — subsequent snapshots correctly classify walls as static and include only health in enemy state. Human sets port to 9078 in Project Settings — addon listens on 9078. `stage.toml` overrides Project Settings.

---

## Current State Analysis

### What exists (that M5 touches):

1. **Static classification** — `stage-protocol/src/static_classes.rs` has a hardcoded `STATIC_CLASSES` list. `is_static_class()` checks against this list. Used by `snapshot.rs` to separate static/dynamic entities.

2. **Token budget** — `stage-core/src/budget.rs` has `SnapshotBudgetDefaults::HARD_CAP` hardcoded to 5000. `resolve_budget()` clamps to this constant. `inject_budget()` in `mcp/mod.rs` references `SnapshotBudgetDefaults::HARD_CAP` directly.

3. **Clustering** — `stage-core/src/cluster.rs` has `ClusterStrategy` enum and `cluster_by_group()`. Only group-based clustering is implemented. The strategy enum exists but isn't wired to anything — `build_summary_response` always calls `cluster_by_group()`.

4. **State properties** — `EntityData.state` contains exported vars from the addon. The server passes them through without filtering. No per-group/class property selection exists.

5. **Bearing format** — `RelativePosition` always includes both `bearing` (cardinal) and `bearing_deg`. No way to select one or the other.

6. **Session state** — `tcp.rs::SessionState` holds `spatial_index`, `delta_engine`, `watch_engine`. No config field.

7. **Project Settings** — `runtime.gd` reads `stage/connection/port` with default 9077. No other settings are registered.

8. **No stage.toml support** exists.

### What M5 must change:

- Add `SessionConfig` to `SessionState` (server-side config, mutable by `spatial_config`)
- Add `stage.toml` parsing (server reads on startup)
- Extend `plugin.gd` to register Project Settings
- Extend `runtime.gd` to read more settings
- Wire config into: static classification, state property filtering, clustering, bearing formatting, budget resolution
- Add the `spatial_config` MCP tool

---

## Implementation Units

### Unit 1: Session Config Type (`stage-core`)

**File:** `crates/stage-core/src/config.rs` (new)

This is the canonical config type, shared by server and core logic. Pure data — no I/O, no Godot, no MCP.

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::cluster::ClusterStrategy;

/// Bearing output format preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BearingFormat {
    Cardinal,
    Degrees,
    Both,
}

impl Default for BearingFormat {
    fn default() -> Self {
        Self::Both
    }
}

/// Active session configuration.
///
/// Three sources with precedence: spatial_config (session) > stage.toml (project) > Project Settings (machine).
/// This struct holds the merged effective config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Glob patterns for static node classification.
    /// Nodes matching these patterns are treated as static regardless of class.
    #[serde(default)]
    pub static_patterns: Vec<String>,

    /// Properties to include in state output, keyed by group name or class name.
    /// Key "*" applies to all nodes. Empty map = include all exported vars (default).
    #[serde(default)]
    pub state_properties: HashMap<String, Vec<String>>,

    /// How to cluster nodes in summary views.
    #[serde(default)]
    pub cluster_by: ClusterStrategy,

    /// Bearing format preference.
    #[serde(default)]
    pub bearing_format: BearingFormat,

    /// Whether to include non-exported (internal) variables in state output.
    #[serde(default)]
    pub expose_internals: bool,

    /// Physics tick polling rate (every N physics frames). 1 = every frame.
    #[serde(default = "default_poll_interval")]
    pub poll_interval: u32,

    /// Hard cap on token budget for any single response.
    #[serde(default = "default_token_hard_cap")]
    pub token_hard_cap: u32,
}

fn default_poll_interval() -> u32 { 1 }
fn default_token_hard_cap() -> u32 { 5000 }

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            static_patterns: Vec::new(),
            state_properties: HashMap::new(),
            cluster_by: ClusterStrategy::default(),
            bearing_format: BearingFormat::default(),
            expose_internals: false,
            poll_interval: default_poll_interval(),
            token_hard_cap: default_token_hard_cap(),
        }
    }
}

impl SessionConfig {
    /// Merge a partial config update (from spatial_config tool) into this config.
    /// Only fields present in the update are overwritten.
    pub fn apply_update(&mut self, update: &ConfigUpdate) {
        if let Some(ref v) = update.static_patterns {
            self.static_patterns = v.clone();
        }
        if let Some(ref v) = update.state_properties {
            self.state_properties = v.clone();
        }
        if let Some(v) = update.cluster_by {
            self.cluster_by = v;
        }
        if let Some(v) = update.bearing_format {
            self.bearing_format = v;
        }
        if let Some(v) = update.expose_internals {
            self.expose_internals = v;
        }
        if let Some(v) = update.poll_interval {
            self.poll_interval = v;
        }
        if let Some(v) = update.token_hard_cap {
            self.token_hard_cap = v;
        }
    }

    /// Check if a node path matches any static pattern (glob-style).
    /// Supports simple glob: "*" matches any segment, "walls/*" matches "walls/anything".
    pub fn matches_static_pattern(&self, path: &str) -> bool {
        self.static_patterns.iter().any(|pattern| glob_match(pattern, path))
    }

    /// Filter state properties for a node based on its groups and class.
    /// Returns None if no filtering configured (include all exported vars).
    /// Returns Some(list) if specific properties should be included.
    pub fn filter_state_properties(
        &self,
        groups: &[String],
        class: &str,
    ) -> Option<Vec<String>> {
        if self.state_properties.is_empty() {
            return None; // No filtering — include all
        }

        let mut props = Vec::new();
        let mut found = false;

        // Check group-based entries
        for group in groups {
            if let Some(group_props) = self.state_properties.get(group) {
                props.extend(group_props.iter().cloned());
                found = true;
            }
        }

        // Check class-based entry
        if let Some(class_props) = self.state_properties.get(class) {
            props.extend(class_props.iter().cloned());
            found = true;
        }

        // Check wildcard
        if let Some(wildcard_props) = self.state_properties.get("*") {
            props.extend(wildcard_props.iter().cloned());
            found = true;
        }

        if found {
            props.sort();
            props.dedup();
            Some(props)
        } else {
            None // No matching rules — include all
        }
    }
}

/// Partial config update — all fields optional.
/// Used by the spatial_config MCP tool.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigUpdate {
    pub static_patterns: Option<Vec<String>>,
    pub state_properties: Option<HashMap<String, Vec<String>>>,
    pub cluster_by: Option<ClusterStrategy>,
    pub bearing_format: Option<BearingFormat>,
    pub expose_internals: Option<bool>,
    pub poll_interval: Option<u32>,
    pub token_hard_cap: Option<u32>,
}

/// Simple glob matching for static patterns.
/// Supports: "walls/*" matches "walls/anything", "*" matches everything,
/// "walls/*/door" matches "walls/foo/door".
fn glob_match(pattern: &str, path: &str) -> bool {
    let pat_parts: Vec<&str> = pattern.split('/').collect();
    let path_parts: Vec<&str> = path.split('/').collect();
    glob_match_parts(&pat_parts, &path_parts)
}

fn glob_match_parts(pattern: &[&str], path: &[&str]) -> bool {
    match (pattern.first(), path.first()) {
        (None, None) => true,
        (Some(&"*"), _) if pattern.len() == 1 => true, // trailing * matches rest
        (Some(&"*"), Some(_)) => {
            // * matches this segment, continue
            glob_match_parts(&pattern[1..], &path[1..])
        }
        (Some(p), Some(s)) if p == s => {
            glob_match_parts(&pattern[1..], &path[1..])
        }
        _ => false,
    }
}
```

**File:** `crates/stage-core/src/lib.rs` — add `pub mod config;`

**Implementation Notes:**
- `glob_match` is deliberately simple — no `**` recursive glob. Just `*` matching a single path segment. This covers the documented use cases (`walls/*`, `terrain/*`, `props/*`).
- `filter_state_properties` returns `None` when no filtering is configured (default behavior = show all exported vars). Returns `Some(vec)` when specific properties should be shown.
- `ConfigUpdate` is the partial update shape — all `Option` fields. `apply_update` only overwrites fields that are `Some`.

**Acceptance Criteria:**
- [ ] `SessionConfig::default()` has all expected defaults matching SPEC.md
- [ ] `apply_update` only overwrites provided fields
- [ ] `matches_static_pattern("walls/*", "walls/segment_01")` returns true
- [ ] `matches_static_pattern("walls/*", "enemies/scout")` returns false
- [ ] `filter_state_properties` returns None when `state_properties` is empty
- [ ] `filter_state_properties` returns correct merged properties from groups + class + wildcard

---

### Unit 2: stage.toml Parsing (`stage-server`)

**File:** `crates/stage-server/src/config.rs` (new)

Reads the `stage.toml` project file on server startup. The server doesn't know the Godot project path directly — it receives the project path hint from the handshake or via environment variable.

```rust
use anyhow::Result;
use serde::Deserialize;
use stage_core::config::{BearingFormat, SessionConfig};
use stage_core::cluster::ClusterStrategy;
use std::collections::HashMap;
use std::path::Path;

/// TOML file shape — maps to the documented stage.toml format.
/// All sections and fields are optional.
#[derive(Debug, Default, Deserialize)]
pub struct StageToml {
    pub connection: Option<ConnectionConfig>,
    pub tracking: Option<TrackingConfig>,
    pub recording: Option<RecordingConfig>,
    pub display: Option<DisplayConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ConnectionConfig {
    pub port: Option<u16>,
}

#[derive(Debug, Default, Deserialize)]
pub struct TrackingConfig {
    pub static_patterns: Option<Vec<String>>,
    pub token_hard_cap: Option<u32>,
    pub state_properties: Option<HashMap<String, Vec<String>>>,
    pub cluster_by: Option<ClusterStrategy>,
    pub bearing_format: Option<BearingFormat>,
    pub expose_internals: Option<bool>,
    pub poll_interval: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
pub struct RecordingConfig {
    pub storage_path: Option<String>,
    pub max_frames: Option<u32>,
    pub capture_interval: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
pub struct DisplayConfig {
    pub show_agent_notifications: Option<bool>,
    pub show_recording_indicator: Option<bool>,
}

/// Load `stage.toml` from a directory. Returns `SessionConfig` with the
/// tracking-related fields applied. Returns default config if file not found.
pub fn load_toml_config(project_dir: &Path) -> SessionConfig {
    let toml_path = project_dir.join("stage.toml");
    match std::fs::read_to_string(&toml_path) {
        Ok(contents) => match toml::from_str::<StageToml>(&contents) {
            Ok(parsed) => {
                tracing::info!("Loaded config from {}", toml_path.display());
                toml_to_session_config(&parsed)
            }
            Err(e) => {
                tracing::warn!("Failed to parse {}: {}", toml_path.display(), e);
                SessionConfig::default()
            }
        },
        Err(_) => {
            tracing::debug!("No stage.toml found at {}", toml_path.display());
            SessionConfig::default()
        }
    }
}

/// Extract port from TOML config (separate from SessionConfig since port
/// is a connection concern, not a session config concern).
pub fn load_toml_port(project_dir: &Path) -> Option<u16> {
    let toml_path = project_dir.join("stage.toml");
    let contents = std::fs::read_to_string(toml_path).ok()?;
    let parsed: StageToml = toml::from_str(&contents).ok()?;
    parsed.connection.and_then(|c| c.port)
}

fn toml_to_session_config(toml: &StageToml) -> SessionConfig {
    let mut config = SessionConfig::default();
    if let Some(ref tracking) = toml.tracking {
        if let Some(ref v) = tracking.static_patterns {
            config.static_patterns = v.clone();
        }
        if let Some(ref v) = tracking.state_properties {
            config.state_properties = v.clone();
        }
        if let Some(v) = tracking.cluster_by {
            config.cluster_by = v;
        }
        if let Some(v) = tracking.bearing_format {
            config.bearing_format = v;
        }
        if let Some(v) = tracking.expose_internals {
            config.expose_internals = v;
        }
        if let Some(v) = tracking.poll_interval {
            config.poll_interval = v;
        }
        if let Some(v) = tracking.token_hard_cap {
            config.token_hard_cap = v;
        }
    }
    config
}
```

**Dependencies:** Add `toml = "0.8"` to `crates/stage-server/Cargo.toml`.

**Implementation Notes:**
- The server finds the project directory via `THEATRE_PROJECT_DIR` env var (set by the MCP client in the `.mcp.json` config), or falls back to the current working directory.
- Port from `stage.toml` is loaded separately and used to override the default/env port before connecting.
- Recording and display configs are parsed but not consumed by the server — they're forwarded to the addon if connected. For M5 scope, we only use `tracking` fields.

**Acceptance Criteria:**
- [ ] `load_toml_config` returns default config when file doesn't exist
- [ ] `load_toml_config` correctly parses a valid `stage.toml` with all fields
- [ ] `load_toml_config` logs warning and returns defaults for invalid TOML
- [ ] Partial TOML (only `[tracking]` section) works without error
- [ ] `load_toml_port` returns `Some(port)` when `[connection] port` is set

---

### Unit 3: Add Config to Session State (`stage-server`)

**File:** `crates/stage-server/src/tcp.rs` (modify)

Add `config` field to `SessionState` and load from TOML on startup.

```rust
// In SessionState struct, add:
pub struct SessionState {
    // ... existing fields ...
    /// Active session configuration (merged from TOML defaults + spatial_config overrides).
    pub config: stage_core::config::SessionConfig,
}

// In Default impl:
impl Default for SessionState {
    fn default() -> Self {
        Self {
            // ... existing fields ...
            config: stage_core::config::SessionConfig::default(),
        }
    }
}
```

**File:** `crates/stage-server/src/main.rs` (modify)

Load TOML config on startup and seed SessionState:

```rust
// In main(), after parsing port:
let project_dir = std::env::var("THEATRE_PROJECT_DIR")
    .map(std::path::PathBuf::from)
    .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

// Load TOML config (port override + session config defaults)
let toml_port = config::load_toml_port(&project_dir);
let port = toml_port.unwrap_or(port); // TOML overrides env/default

let base_config = config::load_toml_config(&project_dir);

let state = Arc::new(Mutex::new(SessionState {
    config: base_config,
    ..Default::default()
}));
```

**Implementation Notes:**
- Config survives reconnection (not reset when TCP drops — same as watch engine).
- `spatial_config` tool calls apply on top of the TOML-loaded base config.
- The base config (from TOML) is stored separately if we need to "reset to defaults" later. For M5, we just store the merged config.

**Acceptance Criteria:**
- [ ] `SessionState` has a `config` field
- [ ] Config is initialized from TOML on startup
- [ ] Config survives TCP disconnection/reconnection
- [ ] Port from TOML overrides default port

---

### Unit 4: `spatial_config` MCP Tool (`stage-server`)

**File:** `crates/stage-server/src/mcp/config.rs` (new)

```rust
use rmcp::model::ErrorData as McpError;
use schemars::JsonSchema;
use serde::Deserialize;
use stage_core::cluster::ClusterStrategy;
use stage_core::config::{BearingFormat, ConfigUpdate};
use std::collections::HashMap;

use super::serialize_response;

/// MCP parameters for the spatial_config tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SpatialConfigParams {
    /// Glob patterns for static node classification.
    /// Nodes matching these are treated as static. Example: ["walls/*", "terrain/*"]
    pub static_patterns: Option<Vec<String>>,

    /// Properties to include in state output per group or class.
    /// Key "*" applies to all nodes. Example: { "enemies": ["health", "alert_level"] }
    pub state_properties: Option<HashMap<String, Vec<String>>>,

    /// How to cluster nodes in summary views: "group", "class", "proximity", or "none".
    pub cluster_by: Option<ClusterStrategy>,

    /// Bearing format: "cardinal" (e.g. "ahead_left"), "degrees" (e.g. 322), or "both" (default).
    pub bearing_format: Option<BearingFormat>,

    /// Include non-exported (internal) variables in state output. Default: false.
    pub expose_internals: Option<bool>,

    /// Collection frequency: every N physics frames. Default: 1.
    pub poll_interval: Option<u32>,

    /// Hard cap on tokens for any single response. Default: 5000.
    pub token_hard_cap: Option<u32>,
}

impl SpatialConfigParams {
    pub fn to_config_update(&self) -> ConfigUpdate {
        ConfigUpdate {
            static_patterns: self.static_patterns.clone(),
            state_properties: self.state_properties.clone(),
            cluster_by: self.cluster_by,
            bearing_format: self.bearing_format,
            expose_internals: self.expose_internals,
            poll_interval: self.poll_interval,
            token_hard_cap: self.token_hard_cap,
        }
    }
}

pub async fn handle_spatial_config(
    params: SpatialConfigParams,
    state: &std::sync::Arc<tokio::sync::Mutex<crate::tcp::SessionState>>,
) -> Result<String, McpError> {
    let update = params.to_config_update();

    let effective_config = {
        let mut s = state.lock().await;
        s.config.apply_update(&update);
        s.config.clone()
    };

    let response = serde_json::json!({
        "result": "ok",
        "config": effective_config,
        "budget": {
            "used": 50,
            "limit": 200,
            "hard_cap": effective_config.token_hard_cap,
        }
    });

    serialize_response(&response)
}
```

**File:** `crates/stage-server/src/mcp/mod.rs` (modify)

Add `pub mod config;` and register the tool:

```rust
pub mod config;

// In the use block, add:
use config::{SpatialConfigParams, handle_spatial_config};

// In the #[tool_router] impl, add the tool method:
/// Configure tracking behavior — static patterns, state properties,
/// clustering, bearing format, and token limits. Changes apply for the
/// current session. Call with no parameters to see current config.
#[tool(description = "Configure tracking behavior. Set static_patterns (glob patterns for static nodes like [\"walls/*\"]), state_properties (per-group/class property tracking like {\"enemies\": [\"health\"]}), cluster_by (group/class/proximity/none), bearing_format (cardinal/degrees/both), expose_internals (include non-exported vars), poll_interval (collection frequency), token_hard_cap (max tokens per response). Changes apply for the current session.")]
pub async fn spatial_config(
    &self,
    Parameters(params): Parameters<SpatialConfigParams>,
) -> Result<String, McpError> {
    handle_spatial_config(params, &self.state).await
}
```

**Implementation Notes:**
- Calling `spatial_config` with no parameters returns the current config (all fields are optional in the params).
- The response always includes the full effective config after applying any updates.
- Budget on config responses is minimal (the response itself is small).

**Acceptance Criteria:**
- [ ] `spatial_config()` with no params returns current config
- [ ] `spatial_config(static_patterns: ["walls/*"])` updates static patterns and returns effective config
- [ ] `spatial_config(state_properties: { "enemies": ["health"] })` updates state properties
- [ ] Multiple successive calls are additive (each updates only provided fields)
- [ ] Config survives between MCP tool calls (stored in SessionState)
- [ ] `hard_cap` in budget block reflects the configured value

---

### Unit 5: Wire Config into Static Classification (`stage-server`)

**File:** `crates/stage-server/src/mcp/snapshot.rs` (modify)

Currently `is_static_class(&entity.class)` is the only static check. M5 adds pattern-based classification from config.

```rust
/// Check if an entity should be treated as static.
/// Uses both class-based heuristics AND config-based pattern matching.
fn is_entity_static(entity: &EntityData, config: &SessionConfig) -> bool {
    // 1. Config patterns take priority
    if config.matches_static_pattern(&entity.path) {
        return true;
    }
    // 2. Fall back to class-based heuristic
    is_static_class(&entity.class)
}
```

Replace all calls to `is_static_class(&entity.class)` in `snapshot.rs` with `is_entity_static(entity, &config)`. The config is obtained from SessionState at the start of the snapshot handler (single lock acquisition).

Changes in `build_summary_response`, `build_standard_response`, `build_full_response`:
- Add `config: &SessionConfig` parameter
- Pass to `to_raw_entity` or use directly for static checks
- The `to_raw_entity` function gets a config parameter to set `is_static` based on both pattern and class

```rust
fn to_raw_entity(e: &EntityData, config: &SessionConfig) -> RawEntityData {
    RawEntityData {
        // ... existing fields ...
        is_static: config.matches_static_pattern(&e.path) || is_static_class(&e.class),
        // ...
    }
}
```

In the `spatial_snapshot` handler (in `mod.rs`), read config from state:

```rust
// After acquiring state lock for spatial index update:
let config = {
    let state = self.state.lock().await;
    state.config.clone()
};

// Pass config to build_* functions
let response = match detail {
    DetailLevel::Summary => {
        build_summary_response(&raw_data, &entities_with_rel, &persp, budget_limit, config.token_hard_cap, &config)
    }
    // ... etc
};
```

**Implementation Notes:**
- Config is cloned once from SessionState and passed down. This avoids holding the lock during response building.
- The `hard_cap` parameter in build functions now comes from `config.token_hard_cap` instead of `SnapshotBudgetDefaults::HARD_CAP`.

**Acceptance Criteria:**
- [ ] Nodes matching `static_patterns` globs are classified as static in snapshot responses
- [ ] Class-based heuristic still works when no patterns match
- [ ] Nodes matching patterns appear in `static_summary` (standard) or `static_nodes` (full)
- [ ] Changing patterns via `spatial_config` affects subsequent snapshots

---

### Unit 6: Wire Config into State Property Filtering (`stage-server`)

**File:** `crates/stage-server/src/mcp/snapshot.rs` (modify)

Apply state property filtering when building output entities. Currently `build_output_entity` includes all state properties. With config, only configured properties are included (when configured).

```rust
fn build_output_entity(
    entity: &EntityData,
    rel: &RelativePosition,
    full: bool,
    config: &SessionConfig,
) -> OutputEntity {
    // Filter state properties based on config
    let state = match config.filter_state_properties(&entity.groups, &entity.class) {
        Some(allowed_props) => {
            entity.state
                .iter()
                .filter(|(k, _)| allowed_props.contains(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        }
        None => entity.state.clone(), // No filtering — include all
    };

    OutputEntity {
        // ... use filtered state instead of entity.state.clone() ...
        state,
        // ... rest unchanged ...
    }
}
```

Also apply to `all_exported_vars` in full detail mode — if `expose_internals` is false (default), internal vars are not included. This is already the addon's behavior for standard detail, but for full detail, the addon may send all exported vars regardless. The filtering happens server-side.

**Implementation Notes:**
- When `state_properties` is empty (default), `filter_state_properties` returns `None` and all exported vars are included — backward compatible.
- The wildcard key `"*"` matches all nodes, letting agents configure globally tracked properties.
- `expose_internals` from config should be sent to the addon in the query params so the addon knows whether to collect internal vars. This requires adding the field to `GetSnapshotDataParams` (Unit 8).

**Acceptance Criteria:**
- [ ] Default behavior (no config): all exported vars included — backward compatible
- [ ] `state_properties: { "enemies": ["health"] }` — enemy nodes only show `health` in `state`
- [ ] `state_properties: { "*": ["visible"] }` — all nodes show `visible` in `state`
- [ ] Properties from multiple matching rules (group + class + wildcard) are merged

---

### Unit 7: Wire Config into Clustering and Bearing Format (`stage-server`)

**File:** `crates/stage-core/src/cluster.rs` (modify)

Add `cluster_by_class` and `cluster_by_proximity` functions, and a dispatch function:

```rust
/// Dispatch clustering based on strategy.
pub fn cluster_entities(
    entities: &[(RawEntityData, RelativePosition)],
    strategy: ClusterStrategy,
) -> Vec<Cluster> {
    match strategy {
        ClusterStrategy::Group => cluster_by_group(entities),
        ClusterStrategy::Class => cluster_by_class(entities),
        ClusterStrategy::Proximity => cluster_by_proximity(entities),
        ClusterStrategy::None => cluster_none(entities),
    }
}

/// Cluster by Godot class name.
pub fn cluster_by_class(
    entities: &[(RawEntityData, RelativePosition)],
) -> Vec<Cluster> {
    let mut class_map: HashMap<String, Vec<(&RawEntityData, &RelativePosition)>> = HashMap::new();
    let mut static_count = 0usize;

    for (entity, rel) in entities {
        if entity.is_static {
            static_count += 1;
        } else {
            class_map.entry(entity.class.clone()).or_default().push((entity, rel));
        }
    }

    let mut clusters: Vec<Cluster> = class_map
        .into_iter()
        .map(|(label, members)| build_cluster(label, &members, None))
        .collect();

    clusters.sort_by(|a, b| a.label.cmp(&b.label));

    if static_count > 0 {
        clusters.push(Cluster {
            label: "static_geometry".to_string(),
            count: static_count,
            nearest: None,
            farthest_dist: 0.0,
            summary: None,
            note: Some("unchanged".to_string()),
        });
    }

    clusters
}

/// Cluster by spatial proximity (simple nearest-seed algorithm).
pub fn cluster_by_proximity(
    entities: &[(RawEntityData, RelativePosition)],
) -> Vec<Cluster> {
    let dynamic: Vec<(&RawEntityData, &RelativePosition)> = entities
        .iter()
        .filter(|(e, _)| !e.is_static)
        .map(|(e, r)| (e, r))
        .collect();
    let static_count = entities.len() - dynamic.len();

    if dynamic.is_empty() {
        let mut clusters = Vec::new();
        if static_count > 0 {
            clusters.push(Cluster {
                label: "static_geometry".to_string(),
                count: static_count,
                nearest: None,
                farthest_dist: 0.0,
                summary: None,
                note: Some("unchanged".to_string()),
            });
        }
        return clusters;
    }

    // Simple proximity clustering: merge threshold 10 units
    const MERGE_THRESHOLD: f64 = 10.0;
    let mut cluster_centers: Vec<[f64; 3]> = Vec::new();
    let mut cluster_members: Vec<Vec<(&RawEntityData, &RelativePosition)>> = Vec::new();

    for (entity, rel) in &dynamic {
        let pos = entity.position;
        let mut assigned = false;
        for (i, center) in cluster_centers.iter().enumerate() {
            let dx = pos[0] - center[0];
            let dy = pos[1] - center[1];
            let dz = pos[2] - center[2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist < MERGE_THRESHOLD {
                cluster_members[i].push((entity, rel));
                assigned = true;
                break;
            }
        }
        if !assigned {
            cluster_centers.push(pos);
            cluster_members.push(vec![(entity, rel)]);
        }
    }

    let mut clusters: Vec<Cluster> = cluster_members
        .iter()
        .enumerate()
        .map(|(i, members)| {
            let label = format!("cluster_{}", i + 1);
            build_cluster(label, members, None)
        })
        .collect();

    if static_count > 0 {
        clusters.push(Cluster {
            label: "static_geometry".to_string(),
            count: static_count,
            nearest: None,
            farthest_dist: 0.0,
            summary: None,
            note: Some("unchanged".to_string()),
        });
    }

    clusters
}

/// No clustering — each entity is its own entry (except static).
fn cluster_none(
    entities: &[(RawEntityData, RelativePosition)],
) -> Vec<Cluster> {
    let mut clusters = Vec::new();
    let mut static_count = 0usize;

    for (entity, rel) in entities {
        if entity.is_static {
            static_count += 1;
            continue;
        }
        clusters.push(Cluster {
            label: entity.path.clone(),
            count: 1,
            nearest: Some(ClusterNearest {
                node: entity.path.clone(),
                dist: rel.dist,
                bearing: rel.bearing,
            }),
            farthest_dist: rel.dist,
            summary: None,
            note: None,
        });
    }

    if static_count > 0 {
        clusters.push(Cluster {
            label: "static_geometry".to_string(),
            count: static_count,
            nearest: None,
            farthest_dist: 0.0,
            summary: None,
            note: Some("unchanged".to_string()),
        });
    }

    clusters
}
```

**File:** `crates/stage-server/src/mcp/snapshot.rs` (modify)

In `build_summary_response`, replace `cluster::cluster_by_group(...)` with `cluster::cluster_entities(..., config.cluster_by)`.

**Bearing format:** In `build_output_entity`, conditionally omit fields based on `config.bearing_format`:

```rust
// In build_output_entity (or a post-processing step):
fn format_rel(rel: &RelativePosition, format: BearingFormat) -> serde_json::Value {
    match format {
        BearingFormat::Both => serde_json::to_value(rel).unwrap_or_default(),
        BearingFormat::Cardinal => serde_json::json!({
            "dist": rel.dist,
            "bearing": rel.bearing,
            "elevation": rel.elevation,
            "occluded": rel.occluded,
        }),
        BearingFormat::Degrees => serde_json::json!({
            "dist": rel.dist,
            "bearing_deg": rel.bearing_deg,
            "elevation": rel.elevation,
            "occluded": rel.occluded,
        }),
    }
}
```

Since `OutputEntity` currently serializes `rel` as a struct, the bearing format requires changing the `rel` field to `serde_json::Value` or using conditional serialization. The simplest approach: change `OutputEntity.rel` to `serde_json::Value` and format it at construction time.

**Acceptance Criteria:**
- [ ] `cluster_by: "class"` produces class-based clusters in summary view
- [ ] `cluster_by: "proximity"` groups nearby entities together
- [ ] `cluster_by: "none"` lists each entity as its own cluster
- [ ] `bearing_format: "cardinal"` omits `bearing_deg` from output
- [ ] `bearing_format: "degrees"` omits cardinal `bearing` from output
- [ ] `bearing_format: "both"` (default) includes both

---

### Unit 8: Wire Config into Budget Resolution (`stage-server`)

**File:** `crates/stage-server/src/mcp/mod.rs` (modify)

The `inject_budget` function currently uses `SnapshotBudgetDefaults::HARD_CAP`. Change to use the config value:

```rust
/// Inject a `budget` block into a JSON object value.
fn inject_budget(response: &mut serde_json::Value, used: u32, limit: u32, hard_cap: u32) {
    if let serde_json::Value::Object(map) = response {
        map.insert(
            "budget".to_string(),
            serde_json::json!({
                "used": used,
                "limit": limit,
                "hard_cap": hard_cap,
            }),
        );
    }
}
```

Update all call sites of `inject_budget` and `resolve_budget` to use `config.token_hard_cap` instead of `SnapshotBudgetDefaults::HARD_CAP`.

Affected files:
- `mcp/mod.rs` — `inject_budget` signature change, `spatial_snapshot`, `spatial_inspect`, `scene_tree`, `spatial_action`
- `mcp/delta.rs` — `handle_spatial_delta`
- `mcp/query.rs` — `handle_spatial_query`
- `mcp/watch.rs` — `handle_spatial_watch`

Pattern in each handler:

```rust
// At start of handler, get config:
let config = {
    let s = self.state.lock().await;
    s.config.clone()
};

// Use config.token_hard_cap in budget resolution:
let budget_limit = resolve_budget(params.token_budget, tier_default, config.token_hard_cap);
// Use config.token_hard_cap in inject_budget:
inject_budget(&mut response, used, budget_limit, config.token_hard_cap);
```

**Implementation Notes:**
- The config is cloned once per tool call (cheap — it's just strings and numbers).
- All 7 existing tools + the new `spatial_config` tool use `config.token_hard_cap`.

**Acceptance Criteria:**
- [ ] `spatial_config(token_hard_cap: 2000)` → subsequent snapshot budgets capped at 2000
- [ ] Budget block in every response reflects the configured hard cap
- [ ] Default (no config change) still uses 5000

---

### Unit 9: Expose Internals Flag in Protocol (`stage-protocol`)

**File:** `crates/stage-protocol/src/query.rs` (modify)

Add `expose_internals` to `GetSnapshotDataParams` and `GetNodeInspectParams`:

```rust
pub struct GetSnapshotDataParams {
    // ... existing fields ...
    /// Whether to include internal (non-exported) variables.
    #[serde(default)]
    pub expose_internals: bool,
}

pub struct GetNodeInspectParams {
    // ... existing fields ...
    /// Whether to include internal variables.
    #[serde(default)]
    pub expose_internals: bool,
}
```

This tells the addon whether to collect internal variables. The addon already has the `InspectState.internal` field — it just needs to conditionally populate it based on this flag.

**GDExtension side:** The addon's collector needs to check this flag. For M5, we pass the flag in the query; the GDExtension respects it when collecting node state. (The GDExtension already collects exported vars; collecting internals means iterating `get_property_list()` for non-exported properties.)

**Implementation Notes:**
- The `expose_internals` flag is forwarded from `SessionConfig` to the addon query params in each tool handler that queries state.
- `#[serde(default)]` ensures backward compatibility — if the addon doesn't see the field, it defaults to `false`.

**Acceptance Criteria:**
- [ ] `GetSnapshotDataParams` serializes with `expose_internals` field
- [ ] Default is `false` (backward compatible with existing addon)

---

### Unit 10: Register Project Settings (`addons/stage/plugin.gd`)

**File:** `addons/stage/plugin.gd` (modify)

Register Stage settings in the Godot Project Settings when the plugin is enabled.

```gdscript
@tool
extends EditorPlugin


func _enable_plugin() -> void:
    _register_settings()
    add_autoload_singleton("StageRuntime", "res://addons/stage/runtime.gd")


func _disable_plugin() -> void:
    remove_autoload_singleton("StageRuntime")


func _register_settings() -> void:
    _add_setting("stage/connection/port", TYPE_INT, 9077,
        PROPERTY_HINT_RANGE, "1024,65535")
    _add_setting("stage/connection/auto_start", TYPE_BOOL, true)
    _add_setting("stage/recording/storage_path", TYPE_STRING,
        "user://stage_recordings/")
    _add_setting("stage/recording/max_frames", TYPE_INT, 36000,
        PROPERTY_HINT_RANGE, "600,360000")
    _add_setting("stage/recording/capture_interval", TYPE_INT, 1,
        PROPERTY_HINT_RANGE, "1,60")
    _add_setting("stage/display/show_agent_notifications", TYPE_BOOL, true)
    _add_setting("stage/display/show_recording_indicator", TYPE_BOOL, true)
    _add_setting("stage/tracking/default_static_patterns",
        TYPE_PACKED_STRING_ARRAY, PackedStringArray())
    _add_setting("stage/tracking/token_hard_cap", TYPE_INT, 5000,
        PROPERTY_HINT_RANGE, "500,50000")


func _add_setting(path: String, type: int, default_value: Variant,
        hint: int = PROPERTY_HINT_NONE, hint_string: String = "") -> void:
    if not ProjectSettings.has_setting(path):
        ProjectSettings.set_setting(path, default_value)
    ProjectSettings.set_initial_value(path, default_value)
    ProjectSettings.add_property_info({
        "name": path,
        "type": type,
        "hint": hint,
        "hint_string": hint_string,
    })
```

**Implementation Notes:**
- Settings are registered on plugin enable, not on every editor start. Once set, they persist in `project.godot`.
- `_add_setting` checks `has_setting` first to avoid overwriting user-configured values.
- `set_initial_value` tells Godot what the default is (for the "reset" button in the editor).
- Keybinding settings (`stage/keybindings/*`) are deferred to M7 (when keyboard shortcuts are implemented).

**Acceptance Criteria:**
- [ ] Enabling the plugin creates `stage/` settings in Project Settings
- [ ] Settings have correct types, defaults, and hints
- [ ] Re-enabling the plugin doesn't overwrite user-changed values
- [ ] Settings persist across editor restarts (saved in project.godot)

---

### Unit 11: Read Settings in Runtime (`addons/stage/runtime.gd`)

**File:** `addons/stage/runtime.gd` (modify)

Read more settings beyond just port.

```gdscript
extends Node

var tcp_server: StageTCPServer
var collector: StageCollector


func _ready() -> void:
    if not ClassDB.class_exists(&"StageTCPServer"):
        push_error("[Stage] GDExtension not loaded — StageTCPServer class not found.")
        return

    var auto_start: bool = ProjectSettings.get_setting(
        "stage/connection/auto_start", true)
    if not auto_start:
        return

    collector = StageCollector.new()
    add_child(collector)

    tcp_server = StageTCPServer.new()
    add_child(tcp_server)
    tcp_server.set_collector(collector)

    var port: int = ProjectSettings.get_setting("stage/connection/port", 9077)
    tcp_server.start(port)


func _physics_process(_delta: float) -> void:
    if tcp_server:
        tcp_server.poll()


func _exit_tree() -> void:
    if tcp_server:
        tcp_server.stop()
```

**Implementation Notes:**
- `auto_start` allows users to disable auto-connect (useful when they want manual control).
- Other settings (recording, display) are consumed by M6/M7 when those features are built. For M5, the addon just reads connection settings.

**Acceptance Criteria:**
- [ ] Setting `auto_start = false` prevents TCP server from starting
- [ ] Changing port in Project Settings changes the TCP listen port
- [ ] Settings read correctly with defaults when not explicitly set

---

## Implementation Order

1. **Unit 1:** Core config types (`stage-core/src/config.rs`) — no dependencies, foundation for everything
2. **Unit 10:** Register Project Settings (`plugin.gd`) — can be done in parallel with Unit 1
3. **Unit 11:** Read settings in runtime (`runtime.gd`) — depends on Unit 10
4. **Unit 2:** TOML parsing (`stage-server/src/config.rs`) — depends on Unit 1
5. **Unit 3:** Add config to SessionState (`tcp.rs`, `main.rs`) — depends on Units 1, 2
6. **Unit 4:** `spatial_config` MCP tool (`mcp/config.rs`) — depends on Units 1, 3
7. **Unit 9:** Expose internals flag in protocol — can be done in parallel
8. **Unit 5:** Wire config into static classification — depends on Units 1, 3
9. **Unit 6:** Wire config into state property filtering — depends on Units 1, 3
10. **Unit 7:** Wire config into clustering and bearing format — depends on Units 1, 3
11. **Unit 8:** Wire config into budget resolution — depends on Units 1, 3

Units 5-8 can be implemented in any order after Unit 3 is complete.

## Testing

### Unit Tests: `crates/stage-core/src/config.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = SessionConfig::default();
        assert!(config.static_patterns.is_empty());
        assert!(config.state_properties.is_empty());
        assert_eq!(config.cluster_by, ClusterStrategy::Group);
        assert_eq!(config.bearing_format, BearingFormat::Both);
        assert!(!config.expose_internals);
        assert_eq!(config.poll_interval, 1);
        assert_eq!(config.token_hard_cap, 5000);
    }

    #[test]
    fn apply_partial_update() {
        let mut config = SessionConfig::default();
        let update = ConfigUpdate {
            static_patterns: Some(vec!["walls/*".into()]),
            ..Default::default()
        };
        config.apply_update(&update);
        assert_eq!(config.static_patterns, vec!["walls/*"]);
        // Other fields unchanged
        assert_eq!(config.token_hard_cap, 5000);
    }

    #[test]
    fn glob_match_simple() {
        assert!(glob_match("walls/*", "walls/segment_01"));
        assert!(glob_match("walls/*", "walls/door_02"));
        assert!(!glob_match("walls/*", "enemies/scout"));
        assert!(!glob_match("walls/*", "walls")); // no trailing segment
    }

    #[test]
    fn glob_match_exact() {
        assert!(glob_match("player", "player"));
        assert!(!glob_match("player", "players"));
    }

    #[test]
    fn glob_match_wildcard_all() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", "walls/segment_01"));
    }

    #[test]
    fn filter_state_properties_empty_config() {
        let config = SessionConfig::default();
        assert!(config.filter_state_properties(&["enemies".into()], "CharacterBody3D").is_none());
    }

    #[test]
    fn filter_state_properties_by_group() {
        let mut config = SessionConfig::default();
        config.state_properties.insert(
            "enemies".into(),
            vec!["health".into(), "alert_level".into()],
        );
        let result = config.filter_state_properties(
            &["enemies".into()], "CharacterBody3D"
        );
        assert_eq!(result, Some(vec!["alert_level".into(), "health".into()]));
    }

    #[test]
    fn filter_state_properties_wildcard() {
        let mut config = SessionConfig::default();
        config.state_properties.insert("*".into(), vec!["visible".into()]);
        let result = config.filter_state_properties(&[], "Node3D");
        assert_eq!(result, Some(vec!["visible".into()]));
    }

    #[test]
    fn filter_state_properties_merged() {
        let mut config = SessionConfig::default();
        config.state_properties.insert("enemies".into(), vec!["health".into()]);
        config.state_properties.insert("*".into(), vec!["visible".into()]);
        let result = config.filter_state_properties(
            &["enemies".into()], "CharacterBody3D"
        );
        let mut expected = vec!["health".into(), "visible".into()];
        expected.sort();
        assert_eq!(result, Some(expected));
    }
}
```

### Unit Tests: `crates/stage-core/src/cluster.rs` (additions)

```rust
#[test]
fn cluster_by_class_basic() {
    let entities = vec![
        (make_entity("enemies/e1", &["enemies"], false), make_rel(5.0)),
        (make_entity("pickups/p1", &["pickups"], false), make_rel(3.0)),
    ];
    // Override class to test class-based clustering
    // (make_entity uses "Node3D" by default)
    let clusters = cluster_by_class(&entities);
    assert_eq!(clusters.len(), 1); // All Node3D
    assert_eq!(clusters[0].label, "Node3D");
}

#[test]
fn cluster_none_each_entity() {
    let entities = vec![
        (make_entity("enemies/e1", &["enemies"], false), make_rel(5.0)),
        (make_entity("enemies/e2", &["enemies"], false), make_rel(10.0)),
    ];
    let clusters = cluster_none(&entities);
    assert_eq!(clusters.len(), 2);
}

#[test]
fn cluster_dispatch() {
    let entities = vec![
        (make_entity("enemies/e1", &["enemies"], false), make_rel(5.0)),
    ];
    let clusters = cluster_entities(&entities, ClusterStrategy::Group);
    assert_eq!(clusters[0].label, "enemies");
}
```

### Unit Tests: `crates/stage-server/src/config.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn load_missing_toml() {
        let dir = TempDir::new().unwrap();
        let config = load_toml_config(dir.path());
        assert_eq!(config.token_hard_cap, 5000); // defaults
    }

    #[test]
    fn load_valid_toml() {
        let dir = TempDir::new().unwrap();
        let toml_path = dir.path().join("stage.toml");
        let mut f = std::fs::File::create(&toml_path).unwrap();
        writeln!(f, r#"
[connection]
port = 9078

[tracking]
static_patterns = ["walls/*", "terrain/*"]
token_hard_cap = 3000
cluster_by = "class"
"#).unwrap();

        let config = load_toml_config(dir.path());
        assert_eq!(config.static_patterns, vec!["walls/*", "terrain/*"]);
        assert_eq!(config.token_hard_cap, 3000);
        assert_eq!(config.cluster_by, ClusterStrategy::Class);
    }

    #[test]
    fn load_toml_port_present() {
        let dir = TempDir::new().unwrap();
        let toml_path = dir.path().join("stage.toml");
        std::fs::write(&toml_path, "[connection]\nport = 9078\n").unwrap();
        assert_eq!(load_toml_port(dir.path()), Some(9078));
    }

    #[test]
    fn load_toml_port_absent() {
        let dir = TempDir::new().unwrap();
        assert_eq!(load_toml_port(dir.path()), None);
    }
}
```

### Integration Test Pattern: `spatial_config` Round-Trip

For integration-level verification (requires running server+addon):

```
1. spatial_config() → returns default config (all defaults)
2. spatial_config(static_patterns: ["walls/*"]) → returns config with walls/* pattern
3. spatial_snapshot(detail: "standard") → walls/* nodes appear in static_summary
4. spatial_config(token_hard_cap: 2000) → budget.hard_cap = 2000
5. spatial_snapshot(detail: "standard") → budget.hard_cap = 2000 in response
6. spatial_config(state_properties: { "enemies": ["health"] }) → only health in enemy state
7. spatial_snapshot(detail: "standard") → enemy entities have state: { health: N } only
```

## Verification Checklist

```bash
# Build everything
cargo build --workspace

# Run all tests
cargo test --workspace

# Lint
cargo clippy --workspace
cargo fmt --check

# Verify new module compiles
cargo test -p stage-core config
cargo test -p stage-server config

# Check TOML parsing
cargo test -p stage-server config::tests

# Check cluster dispatch
cargo test -p stage-core cluster::tests
```
