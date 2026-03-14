# Design: Rename Spectator → Stage + Input Injection

## Overview

Two changes shipped together:

1. **Rename** every occurrence of "Spectator" to "Stage" — crate names, binary name,
   GDExtension classes, addon directory, config file, env vars, docs, skills, CI.
2. **Input injection** — four new `spatial_action` action types that let agents
   drive a running Godot game: `action_press`, `action_release`, `inject_key`,
   `inject_mouse_button`.

### Naming Map

| Old | New |
|-----|-----|
| `spectator` (binary) | `stage` |
| `spectator-server` (crate) | `stage-server` |
| `spectator-godot` (crate) | `stage-godot` |
| `spectator-protocol` (crate) | `stage-protocol` |
| `spectator-core` (crate) | `stage-core` |
| `spectator-wire-tests` (crate) | `stage-wire-tests` |
| `SpectatorServer` (struct) | `StageServer` |
| `SpectatorExtension` (struct) | `StageExtension` |
| `SpectatorTCPServer` (GDExtension class) | `StageTCPServer` |
| `SpectatorCollector` (GDExtension class) | `StageCollector` |
| `SpectatorRecorder` (GDExtension class) | `StageRecorder` |
| `SpectatorToml` (struct) | `StageToml` |
| `SpectatorCliFixture` (test) | `StageCliFixture` |
| `SpectatorRuntime` (autoload) | `StageRuntime` |
| `SpectatorDock` (scene node) | `StageDock` |
| `spectator.toml` (config file) | `stage.toml` |
| `spectator.gdextension` | `stage.gdextension` |
| `libspectator_godot.so/dylib` | `libstage_godot.so/dylib` |
| `spectator_godot.dll` | `stage_godot.dll` |
| `addons/spectator/` | `addons/stage/` |
| `SPECTATOR_PORT` (env, deprecated) | keep as deprecated alias |
| `SPECTATOR_PROJECT_DIR` (env) | keep as deprecated alias |
| `spectator=info` (tracing) | `stage=info` |
| `"spectator"` (debugger channel) | `"stage"` |
| `theatre/spectator/...` (project settings) | `theatre/stage/...` |
| `site/spectator/` | `site/stage/` |
| `.agents/skills/spectator/` | `.agents/skills/stage/` |
| `.agents/skills/spectator-dev/` | `.agents/skills/stage-dev/` |

### Wire Protocol Compatibility

The Handshake and HandshakeAck structs have `spectator_version` fields.
These are **wire protocol fields** — renaming them breaks compatibility with
any existing addon/server pairings. Since there are no external users yet,
rename to `stage_version`. Bump `PROTOCOL_VERSION` from 1 → 2 to make any
stale builds fail fast with a clear version mismatch error.

---

## Implementation Units

### Unit 1: Rename Crate Directories

Move the four crate directories and the test crate:

```
crates/spectator-server/    →  crates/stage-server/
crates/spectator-godot/     →  crates/stage-godot/
crates/spectator-protocol/  →  crates/stage-protocol/
crates/spectator-core/      →  crates/stage-core/
```

Test crate directory stays at `tests/wire-tests/` (it's fine) but the
crate name changes in its `Cargo.toml`.

**Acceptance Criteria**:
- [ ] `ls crates/` shows `stage-server`, `stage-godot`, `stage-protocol`, `stage-core`
- [ ] Old directories do not exist

---

### Unit 2: Rename Addon Directory

```
addons/spectator/  →  addons/stage/
```

Inside the new `addons/stage/`:
- Rename `spectator.gdextension` → `stage.gdextension`
- Rename `bin/linux/libspectator_godot.so` → `bin/linux/libstage_godot.so` (and other platforms)
- Update `plugin.cfg`: `name="Stage"`
- Update `stage.gdextension`: all library paths use `addons/stage/bin/<platform>/libstage_godot.*`

**Acceptance Criteria**:
- [ ] `addons/stage/plugin.cfg` exists with `name="Stage"`
- [ ] `addons/stage/stage.gdextension` paths reference `addons/stage/bin/...`
- [ ] `addons/spectator/` does not exist

---

### Unit 3: Update All Cargo.toml Files

**Root `Cargo.toml`**:
```toml
members = [
    "crates/stage-server",
    "crates/stage-godot",
    "crates/stage-protocol",
    "crates/stage-core",
    # ... director, theatre-cli, theatre-docs-gen unchanged
]
default-members = [
    "crates/stage-server",
    "crates/stage-godot",
    "crates/stage-protocol",
    "crates/stage-core",
    # ... director, theatre-cli unchanged
]

[workspace.dependencies]
stage-protocol = { path = "crates/stage-protocol" }
stage-core = { path = "crates/stage-core" }
```

**`crates/stage-server/Cargo.toml`**:
- `name = "stage-server"`
- `[[bin]] name = "stage"`
- deps: `stage-protocol`, `stage-core`

**`crates/stage-godot/Cargo.toml`**:
- `name = "stage-godot"`
- `[lib] name = "stage_godot"` (controls output filename `libstage_godot.so`)
- dep: `stage-protocol`

**`crates/stage-protocol/Cargo.toml`**: `name = "stage-protocol"`

**`crates/stage-core/Cargo.toml`**: `name = "stage-core"`

**`crates/director/Cargo.toml`**: dep `stage-protocol` (was `spectator-protocol`)

**`crates/theatre-cli/Cargo.toml`**: no direct spectator dep (it's pure CLI)

**`crates/theatre-docs-gen/Cargo.toml`**: dep `stage-server` (was `spectator-server`)

**`tests/wire-tests/Cargo.toml`**: `name = "stage-wire-tests"`, dep `stage-protocol`

**`tests/director-tests/Cargo.toml`**: dep `stage-protocol` (was `spectator-protocol`)

**Acceptance Criteria**:
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace --no-run` compiles
- [ ] Binary is `target/debug/stage`

---

### Unit 4: Rename Rust Structs and Imports

All `use spectator_protocol::` → `use stage_protocol::`, etc.

**Struct renames** (find+replace within each crate):

| File(s) | Old | New |
|---------|-----|-----|
| `stage-server/src/server.rs`, `main.rs`, `mcp/mod.rs` | `SpectatorServer` | `StageServer` |
| `stage-godot/src/lib.rs` | `SpectatorExtension` | `StageExtension` |
| `stage-godot/src/tcp_server.rs` | `SpectatorTCPServer` | `StageTCPServer` |
| `stage-godot/src/collector.rs` | `SpectatorCollector` | `StageCollector` |
| `stage-godot/src/recorder.rs` | `SpectatorRecorder` | `StageRecorder` |
| `stage-server/src/config.rs` | `SpectatorToml` | `StageToml` |
| `stage-server/tests/support/cli_fixture.rs` | `SpectatorCliFixture` | `StageCliFixture` |

**Import renames** (all files):
- `use spectator_protocol::` → `use stage_protocol::`
- `use spectator_core::` → `use stage_core::`
- `use spectator_server::` → `use stage_server::`
- `spectator_protocol` in `extern crate` or path refs → `stage_protocol`

**String literal renames**:
- `"spectator=info"` / `"spectator=warn"` → `"stage=info"` / `"stage=warn"` (tracing directives)
- `"spectator.toml"` → `"stage.toml"` (config file name)
- `spectator_version` field in `Handshake`, `HandshakeAck`, `HandshakeError` → `stage_version`
- `env!("CARGO_BIN_EXE_spectator")` → `env!("CARGO_BIN_EXE_stage")` (test fixtures)
- `make_spectator_error` → `make_stage_error` (if exists in tcp.rs)

**Env var handling** (in `main.rs` and `cli.rs`):
- `SPECTATOR_PORT` — keep as deprecated fallback (already behind `THEATRE_PORT`)
- `SPECTATOR_PROJECT_DIR` — keep as deprecated fallback

**Protocol version bump** (in `stage-protocol/src/handshake.rs`):
```rust
pub const PROTOCOL_VERSION: u32 = 2;
```

**Acceptance Criteria**:
- [ ] `cargo build --workspace` succeeds with zero warnings about `spectator`
- [ ] `cargo test --workspace` passes
- [ ] `grep -r "spectator" crates/ --include="*.rs" -l` returns zero results (except deprecated env var fallback strings)

---

### Unit 5: Update GDScript Files

**`addons/stage/runtime.gd`**:
- `ClassDB.class_exists(&"StageTCPServer")` (was `SpectatorTCPServer`)
- `ClassDB.instantiate(&"StageTCPServer")`, `&"StageCollector"`, `&"StageRecorder"`
- `[Stage]` prefix in push_error/push_warning (was `[Spectator]`)
- `SPECTATOR_PORT` env var — keep as fallback with comment
- Project settings: `"theatre/stage/connection/..."` (was `theatre/spectator/...`)
- Debugger channel: `"stage"`, messages `"stage:status"`, `"stage:command"`, `"stage:activity"`
- Autoload name if referenced: `"StageRuntime"`

**`addons/stage/plugin.gd`**:
- `preload("res://addons/stage/dock.tscn")`
- `preload("res://addons/stage/debugger_plugin.gd")`
- `add_autoload_singleton("StageRuntime", "res://addons/stage/runtime.gd")`
- `remove_autoload_singleton("StageRuntime")`
- Project settings: `"theatre/stage/..."` (was `theatre/spectator/...`)

**`addons/stage/debugger_plugin.gd`**:
- `return prefix == "stage"` (was `"spectator"`)
- Message handlers: `"stage:status"`, `"stage:activity"`

**`addons/stage/dock.tscn`**:
- `path="res://addons/stage/dock.gd"`
- `[node name="StageDock" ...]`

**`addons/stage/plugin.cfg`**:
- `name="Stage"`

**Acceptance Criteria**:
- [ ] `grep -r "spectator" addons/stage/ -l` returns zero results (except deprecated env var comment)
- [ ] `grep -r "Spectator" addons/stage/ -l` returns zero results

---

### Unit 6: Update Theatre CLI

**`crates/theatre-cli/src/paths.rs`**:
- `gdext_filename()`: `"libstage_godot.so"`, `"libstage_godot.dylib"`, `"stage_godot.dll"`
- `gdext_binary()`: `.join("stage")` (was `.join("spectator")`)
- `validate_installed()`: check `addon_dir.join("stage").join("plugin.cfg")`
- Tests: update expected filenames

**`crates/theatre-cli/src/install.rs`**:
- `"stage-godot"` (was `"spectator-godot"`)
- `"stage-server"` (was `"spectator-server"`)
- `"stage"` binary name (was `"spectator"`)
- addon path: `"stage"` (was `"spectator"`)

**`crates/theatre-cli/src/deploy.rs`**:
- Same pattern: crate names, binary name, addon paths

**`crates/theatre-cli/src/init.rs`**:
- `SPECTATOR_PLUGIN_CFG` → `STAGE_PLUGIN_CFG` = `"res://addons/stage/plugin.cfg"`
- `SPECTATOR_RUNTIME_NAME` → `STAGE_RUNTIME_NAME` = `"StageRuntime"`
- `SPECTATOR_RUNTIME_SCRIPT` → `STAGE_RUNTIME_SCRIPT` = `"*res://addons/stage/runtime.gd"`
- MCP config key: `"stage"` (was `"spectator"`)

**`crates/theatre-cli/src/enable.rs`**:
- Same constants as init.rs

**`crates/theatre-cli/src/project.rs`**:
- `"stage"` MCP key, `"StageRuntime"` autoload, `addons/stage/` paths

**Acceptance Criteria**:
- [ ] `cargo build -p theatre-cli` succeeds
- [ ] `cargo test -p theatre-cli` passes
- [ ] `grep -r "spectator" crates/theatre-cli/ --include="*.rs" -l` returns zero

---

### Unit 7: Update Test Projects and Harnesses

**`tests/godot-project/project.godot`**:
- `StageRuntime="*res://addons/stage/runtime.gd"` (autoload)
- Plugin path: `"res://addons/stage/plugin.cfg"`

**`tests/godot-project/addons/`**:
- If symlinked to `addons/spectator/`, update symlink to `addons/stage/`
- If copied, rename directory

**`examples/2d-platformer-demo/project.godot`**:
- Same autoload and plugin path changes

**Test harness files** (`tests/wire-tests/src/harness.rs`):
- `spectator_version` → `stage_version` in handshake
- session_id: `"wire-test-session"` (unchanged)

**`crates/stage-server/tests/support/`**:
- `e2e_harness.rs`: `StageServer::new(...)` (was `SpectatorServer`)
- `cli_fixture.rs`: `StageCliFixture` (was `SpectatorCliFixture`)
- `mod.rs`: `use stage_server::server::StageServer` etc.
- `godot_process.rs`: stderr log prefix `"stage_godot_{port}"` (was `spectator_godot_`)

**`crates/stage-server/tests/cli_binary.rs`**:
- `env!("CARGO_BIN_EXE_stage")` (was `spectator`)

**`crates/stage-server/tests/cli_journeys.rs`**:
- `StageCliFixture` (was `SpectatorCliFixture`)

**Acceptance Criteria**:
- [ ] `cargo test --workspace --no-run` compiles all test crates
- [ ] `cargo test --workspace` passes (non-E2E)

---

### Unit 8: Input Injection — Protocol Layer

**File**: `crates/stage-protocol/src/query.rs`

Add four new variants to `ActionRequest`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ActionRequest {
    // ... existing variants unchanged ...

    /// Press (hold) a named InputMap action.
    ActionPress {
        /// InputMap action name (e.g. "jump", "move_left").
        action_name: String,
        /// Press strength 0.0–1.0. Default 1.0.
        #[serde(default = "default_strength")]
        strength: f32,
    },
    /// Release a named InputMap action.
    ActionRelease {
        /// InputMap action name.
        action_name: String,
    },
    /// Inject a keyboard key event.
    InjectKey {
        /// Godot key name: "A", "SPACE", "UP", "ESCAPE", etc.
        keycode: String,
        /// true = press, false = release.
        pressed: bool,
        /// Whether this is an echo (key held down repeat). Default false.
        #[serde(default)]
        echo: bool,
    },
    /// Inject a mouse button event.
    InjectMouseButton {
        /// Button name: "left", "right", "middle", "wheel_up", "wheel_down".
        button: String,
        /// true = press, false = release.
        pressed: bool,
        /// Screen position [x, y]. Defaults to current mouse position if absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        position: Option<Vec<f64>>,
    },
}

fn default_strength() -> f32 {
    1.0
}
```

**Implementation Notes**:
- `action_name` (not `action`) to avoid collision with the serde tag field `"action"`.
- `strength` defaults to 1.0 via serde default function.
- Key names follow Godot's `Key` enum naming without the `KEY_` prefix: `"A"`, `"SPACE"`, `"UP"`, `"CTRL"`, etc.
- Mouse button names are lowercase to match serde `rename_all = "snake_case"`.

**Acceptance Criteria**:
- [ ] `ActionRequest::ActionPress { action_name: "jump".into(), strength: 1.0 }` serializes to `{"action":"action_press","action_name":"jump","strength":1.0}`
- [ ] Deserialization round-trips for all four new variants
- [ ] Existing variants still serialize/deserialize correctly

---

### Unit 9: Input Injection — MCP Server Layer

**File**: `crates/stage-server/src/mcp/action.rs`

Add to `ActionType` enum:

```rust
pub enum ActionType {
    // ... existing ...

    /// Hold a named InputMap action. Requires: input_action. Optional: strength.
    ActionPress,
    /// Release a named InputMap action. Requires: input_action.
    ActionRelease,
    /// Inject a key press/release. Requires: keycode, pressed.
    InjectKey,
    /// Inject a mouse button press/release. Requires: button, pressed.
    InjectMouseButton,
}
```

Add fields to `SpatialActionParams`:

```rust
pub struct SpatialActionParams {
    // ... existing fields ...

    /// For action_press/action_release: InputMap action name (e.g. "jump").
    pub input_action: Option<String>,

    /// For action_press: strength 0.0–1.0 (default 1.0).
    pub strength: Option<f32>,

    /// For inject_key: Godot key name ("A", "SPACE", "UP", etc.).
    pub keycode: Option<String>,

    /// For inject_key/inject_mouse_button: whether pressed (true) or released (false).
    pub pressed: Option<bool>,

    /// For inject_key: whether this is an echo event.
    #[serde(default)]
    pub echo: bool,

    /// For inject_mouse_button: button name ("left", "right", "middle",
    /// "wheel_up", "wheel_down").
    pub button: Option<String>,
}
```

Add match arms to `build_action_request()`:

```rust
ActionType::ActionPress => {
    let action_name = require_param!(
        params.input_action.as_ref(),
        "'input_action' is required for action_press"
    );
    Ok(ActionRequest::ActionPress {
        action_name: action_name.clone(),
        strength: params.strength.unwrap_or(1.0),
    })
}
ActionType::ActionRelease => {
    let action_name = require_param!(
        params.input_action.as_ref(),
        "'input_action' is required for action_release"
    );
    Ok(ActionRequest::ActionRelease {
        action_name: action_name.clone(),
    })
}
ActionType::InjectKey => {
    let keycode = require_param!(
        params.keycode.as_ref(),
        "'keycode' (e.g. \"SPACE\", \"W\") is required for inject_key"
    );
    let pressed = require_param!(
        params.pressed,
        "'pressed' (bool) is required for inject_key"
    );
    Ok(ActionRequest::InjectKey {
        keycode: keycode.clone(),
        pressed,
        echo: params.echo,
    })
}
ActionType::InjectMouseButton => {
    let button = require_param!(
        params.button.as_ref(),
        "'button' (\"left\", \"right\", \"middle\") is required for inject_mouse_button"
    );
    let pressed = require_param!(
        params.pressed,
        "'pressed' (bool) is required for inject_mouse_button"
    );
    Ok(ActionRequest::InjectMouseButton {
        button: button.clone(),
        pressed,
        position: params.position.clone(),
    })
}
```

**Implementation Notes**:
- The field is `input_action` (not `action`) because `action` is already the ActionType discriminant.
- `position` is reused from the existing teleport field — it's `Option<Vec<f64>>` which works for both `[x,y,z]` teleport and `[x,y]` mouse position.
- `pressed` is a new field; it doesn't conflict with `paused`.

**Acceptance Criteria**:
- [ ] `build_action_request` with `ActionType::ActionPress` + `input_action="jump"` produces `ActionRequest::ActionPress { action_name: "jump", strength: 1.0 }`
- [ ] Missing `input_action` on `ActionPress` returns `invalid_params` error
- [ ] Missing `keycode` on `InjectKey` returns `invalid_params` error
- [ ] Missing `pressed` on `InjectKey` returns `invalid_params` error
- [ ] Unit tests cover all four new variants (success + missing required param)

---

### Unit 10: Input Injection — GDExtension Handler

**File**: `crates/stage-godot/src/action_handler.rs`

Add match arms in `execute_action()`:

```rust
ActionRequest::ActionPress { action_name, strength } => {
    execute_action_press(action_name, *strength).map(ActionResult::Done)
}
ActionRequest::ActionRelease { action_name } => {
    execute_action_release(action_name).map(ActionResult::Done)
}
ActionRequest::InjectKey { keycode, pressed, echo } => {
    execute_inject_key(keycode, *pressed, *echo).map(ActionResult::Done)
}
ActionRequest::InjectMouseButton { button, pressed, position } => {
    execute_inject_mouse_button(button, *pressed, position.as_deref())
        .map(ActionResult::Done)
}
```

Add handler functions:

```rust
fn execute_action_press(action_name: &str, strength: f32) -> Result<ActionResponse, String> {
    let mut input = godot::classes::Input::singleton();
    let sn = StringName::from(action_name);

    if !InputMap::singleton().has_action(&sn) {
        return Err(format!("Unknown InputMap action: '{action_name}'"));
    }

    input.action_press_ex(&sn).strength(strength as f64).done();

    let mut details = serde_json::Map::new();
    details.insert("action_name".into(), serde_json::json!(action_name));
    details.insert("strength".into(), serde_json::json!(strength));

    Ok(ActionResponse {
        action: "action_press".into(),
        result: "ok".into(),
        details,
        frame: 0, // filled by caller if needed
    })
}

fn execute_action_release(action_name: &str) -> Result<ActionResponse, String> {
    let mut input = godot::classes::Input::singleton();
    let sn = StringName::from(action_name);

    if !InputMap::singleton().has_action(&sn) {
        return Err(format!("Unknown InputMap action: '{action_name}'"));
    }

    input.action_release(&sn);

    let mut details = serde_json::Map::new();
    details.insert("action_name".into(), serde_json::json!(action_name));

    Ok(ActionResponse {
        action: "action_release".into(),
        result: "ok".into(),
        details,
        frame: 0,
    })
}

fn execute_inject_key(
    keycode: &str,
    pressed: bool,
    echo: bool,
) -> Result<ActionResponse, String> {
    let key = parse_key(keycode)?;

    let mut event = InputEventKey::new_gd();
    event.set_keycode(key);
    event.set_pressed(pressed);
    event.set_echo(echo);

    Input::singleton().parse_input_event(&event);

    let mut details = serde_json::Map::new();
    details.insert("keycode".into(), serde_json::json!(keycode));
    details.insert("pressed".into(), serde_json::json!(pressed));

    Ok(ActionResponse {
        action: "inject_key".into(),
        result: "ok".into(),
        details,
        frame: 0,
    })
}

fn execute_inject_mouse_button(
    button: &str,
    pressed: bool,
    position: Option<&[f64]>,
) -> Result<ActionResponse, String> {
    let button_index = parse_mouse_button(button)?;

    let mut event = InputEventMouseButton::new_gd();
    event.set_button_index(button_index);
    event.set_pressed(pressed);

    if let Some(pos) = position {
        if pos.len() >= 2 {
            event.set_position(Vector2::new(pos[0] as f32, pos[1] as f32));
        }
    }

    Input::singleton().parse_input_event(&event);

    let mut details = serde_json::Map::new();
    details.insert("button".into(), serde_json::json!(button));
    details.insert("pressed".into(), serde_json::json!(pressed));

    Ok(ActionResponse {
        action: "inject_mouse_button".into(),
        result: "ok".into(),
        details,
        frame: 0,
    })
}
```

Add key/button parsers:

```rust
use godot::global::Key;
use godot::global::MouseButton;

fn parse_key(name: &str) -> Result<Key, String> {
    // Accept with or without "KEY_" prefix
    let normalized = name.to_uppercase();
    let name = normalized.strip_prefix("KEY_").unwrap_or(&normalized);
    match name {
        "A" => Ok(Key::A), "B" => Ok(Key::B), "C" => Ok(Key::C),
        "D" => Ok(Key::D), "E" => Ok(Key::E), "F" => Ok(Key::F),
        "G" => Ok(Key::G), "H" => Ok(Key::H), "I" => Ok(Key::I),
        "J" => Ok(Key::J), "K" => Ok(Key::K), "L" => Ok(Key::L),
        "M" => Ok(Key::M), "N" => Ok(Key::N), "O" => Ok(Key::O),
        "P" => Ok(Key::P), "Q" => Ok(Key::Q), "R" => Ok(Key::R),
        "S" => Ok(Key::S), "T" => Ok(Key::T), "U" => Ok(Key::U),
        "V" => Ok(Key::V), "W" => Ok(Key::W), "X" => Ok(Key::X),
        "Y" => Ok(Key::Y), "Z" => Ok(Key::Z),
        "0" => Ok(Key::KEY_0), "1" => Ok(Key::KEY_1), "2" => Ok(Key::KEY_2),
        "3" => Ok(Key::KEY_3), "4" => Ok(Key::KEY_4), "5" => Ok(Key::KEY_5),
        "6" => Ok(Key::KEY_6), "7" => Ok(Key::KEY_7), "8" => Ok(Key::KEY_8),
        "9" => Ok(Key::KEY_9),
        "SPACE" => Ok(Key::SPACE),
        "ENTER" | "RETURN" => Ok(Key::ENTER),
        "ESCAPE" | "ESC" => Ok(Key::ESCAPE),
        "TAB" => Ok(Key::TAB),
        "BACKSPACE" => Ok(Key::BACKSPACE),
        "DELETE" => Ok(Key::DELETE),
        "UP" => Ok(Key::UP),
        "DOWN" => Ok(Key::DOWN),
        "LEFT" => Ok(Key::LEFT),
        "RIGHT" => Ok(Key::RIGHT),
        "SHIFT" => Ok(Key::SHIFT),
        "CTRL" | "CONTROL" => Ok(Key::CTRL),
        "ALT" => Ok(Key::ALT),
        "F1" => Ok(Key::F1), "F2" => Ok(Key::F2), "F3" => Ok(Key::F3),
        "F4" => Ok(Key::F4), "F5" => Ok(Key::F5), "F6" => Ok(Key::F6),
        "F7" => Ok(Key::F7), "F8" => Ok(Key::F8), "F9" => Ok(Key::F9),
        "F10" => Ok(Key::F10), "F11" => Ok(Key::F11), "F12" => Ok(Key::F12),
        _ => Err(format!("Unknown key: '{name}'. Use Godot key names like A, SPACE, UP, ESCAPE.")),
    }
}

fn parse_mouse_button(name: &str) -> Result<MouseButton, String> {
    match name.to_lowercase().as_str() {
        "left" => Ok(MouseButton::LEFT),
        "right" => Ok(MouseButton::RIGHT),
        "middle" => Ok(MouseButton::MIDDLE),
        "wheel_up" => Ok(MouseButton::WHEEL_UP),
        "wheel_down" => Ok(MouseButton::WHEEL_DOWN),
        _ => Err(format!(
            "Unknown mouse button: '{name}'. Use: left, right, middle, wheel_up, wheel_down."
        )),
    }
}
```

**Implementation Notes**:
- `action_press_ex` is the gdext builder pattern for optional params. Check the
  actual gdext API — it may be `action_press` with a second `strength` param.
  The implementer should verify the exact method signature from the generated
  bindings. If `action_press_ex` doesn't exist, use `action_press` and call
  `Input::singleton().set_action_strength(action, strength)` separately.
- `Input::singleton()` and `InputMap::singleton()` are safe to call on the main
  thread. They return `Gd<Input>` / `Gd<InputMap>`.
- The `frame` field in `ActionResponse` — look at how existing handlers get
  it (via `get_frame(collector)`). New handlers should do the same.
- All new imports needed: `godot::classes::{Input, InputMap, InputEventKey, InputEventMouseButton}`,
  `godot::global::{Key, MouseButton}`, `godot::builtin::{StringName, Vector2}`.
- `InputEventKey::new_gd()` and `InputEventMouseButton::new_gd()` — verify these
  exist in the gdext generated API. They should since both classes inherit
  `RefCounted` and have default constructors.

**Acceptance Criteria**:
- [ ] `cargo build -p stage-godot` compiles
- [ ] `parse_key("SPACE")` returns `Key::SPACE`
- [ ] `parse_key("w")` returns `Key::W` (case-insensitive)
- [ ] `parse_key("KEY_A")` returns `Key::A` (prefix stripped)
- [ ] `parse_mouse_button("left")` returns `MouseButton::LEFT`
- [ ] Unknown key/button returns descriptive error string

---

### Unit 11: Update Documentation

**`CLAUDE.md`** — full pass replacing all `spectator` references:
- Repository Layout: `stage-server/`, `stage-godot/`, `stage-protocol/`, `stage-core/`
- Build Commands: `cargo build -p stage-server`, `-p stage-godot`, etc.
- Binary name: `stage serve`, `stage <tool>`
- Config: `stage.toml`
- GDExtension notes: `stage-godot`
- GDScript adapter: `StageTCPServer`, `StageCollector`, `StageRecorder`, `StageRuntime`
- Architecture rules: `stage-godot`, `stage-server`, `stage-protocol`, `stage-core`
- stdout constraint: `stage serve`

**`docs/` directory** — update all design docs, specs, migration guide:
- `THEATRE-MIGRATION.md`: `SPECTATOR_PORT` reference stays (it's about migration)
- Active design docs in `docs/design/`: update references
- Completed designs in `docs/design/completed/`: leave as-is (historical)
- `ROADMAP.md`, `SPEC.md`, `CONTRACT.md`, `VISION.md`, `UX.md`, `TECH.md`: update

**Acceptance Criteria**:
- [ ] `grep -ri "spectator" CLAUDE.md` returns zero (except deprecated env var mentions)
- [ ] Active design docs updated

---

### Unit 12: Update Skills and Agent Config

**`.claude/skills/patterns/*.md`** — all 16 pattern files:
- Replace `spectator-server` → `stage-server`, `spectator-godot` → `stage-godot`, etc.
- Replace `SpectatorServer` → `StageServer`, `SpectatorCollector` → `StageCollector`, etc.
- Replace file paths: `crates/spectator-*/` → `crates/stage-*/`

**`.claude/rules/patterns.md`** — pattern index file:
- Update all path references

**`.agents/skills/spectator/` → `.agents/skills/stage/`** (rename directory)
**`.agents/skills/spectator-dev/` → `.agents/skills/stage-dev/`** (rename directory)
- Update SKILL.md content inside each

**`.agents/tap.json`**:
```json
{
    "name": "stage",
    "description": "Use Stage to interact with a running Godot game...",
    "path": ".agents/skills/stage",
    ...
}
```

**`.claude/settings.local.json`** — update MCP tool references if present

**Acceptance Criteria**:
- [ ] `.agents/skills/stage/SKILL.md` exists
- [ ] `.agents/skills/spectator/` does not exist
- [ ] `grep -r "spectator" .claude/skills/ -l` returns zero files
- [ ] `grep -r "spectator" .agents/ -l` returns zero files

---

### Unit 13: Update Site (VitePress)

**Rename directory**: `site/spectator/` → `site/stage/`

**`site/.vitepress/config.mts`**:
- Nav: `{ text: 'Stage', link: '/stage/' }`
- Sidebar key: `'/stage/'` with updated links
- Replace all `spectator` references

**`site/.vitepress/data/tools.data.ts`**:
- `server: 'stage' | 'director'`

**`site/.generated/tools.json`**:
- `"server": "stage"` (all occurrences)
- Update description strings

**All `.md` files in `site/`** — replace "Spectator" → "Stage", "spectator" → "stage"
in tool descriptions, headings, URLs, and code examples.

**`site/scripts/check-staleness.sh`**:
- `for md in "$SITE_DIR"/stage/*.md` (was `spectator`)

**Acceptance Criteria**:
- [ ] `site/stage/` exists with all pages
- [ ] `site/spectator/` does not exist
- [ ] VitePress builds successfully

---

### Unit 14: Update CI/CD

**`.github/workflows/ci.yml`**:
- `cargo build -p stage-godot`
- `mkdir -p tests/godot-project/addons/stage/bin/linux`
- `cp target/debug/libstage_godot.so tests/godot-project/addons/stage/bin/linux/`
- `-p stage-server -p director -p theatre-cli -p stage-godot`
- Step names: "Build Stage server (release)" etc.

**`.github/workflows/release.yml`**:
- `gdext: libstage_godot.so` / `.dylib` / `.dll`
- `-p stage-server -p director -p theatre-cli -p stage-godot`
- `cp ".../stage${BIN_EXT}" "${ARCHIVE_NAME}/bin/"`
- `rsync ... addons/stage/ .../addons/stage/`

**Acceptance Criteria**:
- [ ] `grep -r "spectator" .github/ -l` returns zero

---

### Unit 15: Update Scripts

**`scripts/copy-gdext.sh`**:
- `SRC="target/${MODE}/libstage_godot.so"`
- `DST="addons/stage/bin/linux/"`

**`scripts/theatre-deploy`** (legacy):
- `cargo build -p stage-godot`
- `SRC=".../libstage_godot.so"`
- `DST="$PROJECT/addons/stage/bin/linux/"`

**`scripts/install-release.sh`**:
- `for bin in theatre stage director`
- `"  ✓ addons/stage/"`

**Acceptance Criteria**:
- [ ] `grep -r "spectator" scripts/ -l` returns zero

---

### Unit 16: Update Root Config Files

**`.mcp.json`**:
```json
{
  "stage": {
    "command": "./target/release/stage",
    "args": ["serve"]
  }
}
```

**Acceptance Criteria**:
- [ ] `.mcp.json` uses `"stage"` key and `"stage"` binary

---

## Implementation Order

The rename must be done atomically — partial renames break the build. Order:

1. **Unit 1**: Rename crate directories (`git mv`)
2. **Unit 2**: Rename addon directory (`git mv`)
3. **Unit 3**: Update all Cargo.toml files
4. **Unit 4**: Rename Rust structs and imports (bulk find/replace)
5. **Unit 5**: Update GDScript files
6. **Unit 6**: Update theatre-cli
7. **Unit 7**: Update test projects and harnesses
8. *(checkpoint: `cargo build --workspace && cargo test --workspace`)
9. **Unit 8**: Input injection — protocol layer
10. **Unit 9**: Input injection — MCP server layer
11. **Unit 10**: Input injection — GDExtension handler
12. *(checkpoint: `cargo build --workspace && cargo test --workspace`)*
13. **Unit 11**: Update documentation (CLAUDE.md, docs/)
14. **Unit 12**: Update skills and agent config
15. **Unit 13**: Update site
16. **Unit 14**: Update CI/CD
17. **Unit 15**: Update scripts
18. **Unit 16**: Update root configs

## Testing

### Build Verification
```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
cargo fmt --check
```

### Grep Verification
```bash
# Should return ONLY deprecated env var fallback strings in cli.rs/main.rs
grep -ri "spectator" crates/ --include="*.rs" -l
grep -ri "spectator" addons/ -l
grep -ri "spectator" .github/ -l
grep -ri "spectator" scripts/ -l
grep -ri "spectator" .claude/ -l
grep -ri "spectator" .agents/ -l
grep -ri "spectator" site/ --include="*.md" --include="*.mts" --include="*.ts" --include="*.json" -l
```

### Input Injection Tests
```bash
# Unit tests (no Godot needed)
cargo test -p stage-server action  # tests in mcp/action.rs
cargo test -p stage-protocol      # serde round-trip tests

# E2E tests (Godot needed, non-headless for full input pipeline)
# New test in cli_journeys.rs or e2e_journeys.rs:
# 1. action_press "ui_accept" → ack
# 2. action_release "ui_accept" → ack
# 3. inject_key "SPACE" pressed=true → ack
# 4. inject_key "SPACE" pressed=false → ack
# 5. inject_mouse_button "left" pressed=true position=[100,100] → ack
```

### Full E2E
```bash
theatre deploy tests/godot-project
cargo test --workspace -- --include-ignored --nocapture
```

## Verification Checklist

```bash
# 1. Build
cargo build --workspace

# 2. Tests
cargo test --workspace

# 3. Clippy
cargo clippy --workspace

# 4. Fmt
cargo fmt --check

# 5. Binary exists with new name
ls -la target/debug/stage

# 6. No stale references (allow deprecated env vars)
! grep -ri "spectator" crates/ --include="*.rs" -l | grep -v "deprecated"

# 7. Site builds
cd site && npm run build
```
