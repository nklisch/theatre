# Theatre

A toolkit giving AI agents the ability to build and debug Godot games. Two MCP
servers — **Director** (build) and **Spectator** (observe) — that work together
or independently.

**Documentation**: [godot-theatre.dev](https://godot-theatre.dev)

## Tools

| Tool | Purpose | MCP Server | Godot Side |
|---|---|---|---|
| **Director** | Create/modify scenes, resources, tilemaps, animations | `director serve` | GDScript addon (`addons/director/`) |
| **Spectator** | Observe spatial state of a running game | `spectator-server` | GDExtension + GDScript (`addons/spectator/`) |

## Prerequisites

- Rust (stable, 1.80+) — `rustup update stable`
- Godot 4.2–4.6+ (GDExtension built with `api-4-5` + `lazy-function-tables` — runs on any 4.x without recompiling)
- An MCP client — Claude Code, Claude Desktop, or any MCP-compatible agent

---

## Quick Start — Spectator (Runtime Observation)

Spectator gives AI agents spatial awareness of a running Godot game. A Rust
MCP server connects to a GDExtension addon inside Godot via TCP, exposing the
scene tree as MCP tools.

```
AI Agent (Claude, etc.)
    ↕ MCP over stdio
spectator-server  (Rust binary)
    ↕ TCP :9077
Godot Engine  (GDExtension addon)
```

### 1. Build the GDExtension

```bash
cargo build -p spectator-godot
./scripts/copy-gdext.sh          # copies .so into addons/spectator/bin/linux/
```

For a release build:

```bash
cargo build -p spectator-godot --release
./scripts/copy-gdext.sh release
```

**If you have `theatre-deploy` installed** (see [Development](#development) below), you can build and deploy to a Godot project in one step:

```bash
theatre-deploy                        # debug → ~/godot/test-harness (default)
theatre-deploy --release ~/my-game    # release → specific project
```

### 2. Install the Spectator addon in your Godot project

Copy the `addons/spectator/` directory into your Godot project's `addons/` folder:

```bash
cp -r addons/spectator /path/to/your-godot-project/addons/
```

Then in Godot: **Project → Project Settings → Plugins → Spectator → Enable**

When the plugin is enabled it registers a `SpectatorRuntime` autoload that starts the TCP listener on port 9077. No scene changes are required.

### 3. Build the Spectator MCP server

```bash
cargo build -p spectator-server --release
```

The binary is at `target/release/spectator-server`.

### 4. Configure your MCP client for Spectator

**Claude Code** — add to `.mcp.json` in your project root (or `~/.claude/mcp.json` for global):

```json
{
  "mcpServers": {
    "spectator": {
      "type": "stdio",
      "command": "/path/to/theatre/target/release/spectator-server"
    }
  }
}
```

**Claude Desktop** — add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "spectator": {
      "command": "/path/to/theatre/target/release/spectator-server",
      "args": []
    }
  }
}
```

Use `THEATRE_PORT=9078` in the env block if you need a non-default port (and set the same in Godot's Project Settings under `theatre/spectator/connection/port`).

### Testing Spectator

1. Open your Godot project and run the scene (Play button or F5)
2. You should see in Godot's output: `[SpectatorTCPServer] Listening on 127.0.0.1:9077`
3. Start a Claude Code session in your project directory
4. Ask: `call spatial_snapshot with detail summary`

Expected response: a JSON summary of the entities currently in the scene.

If the game isn't running, the tool returns: `Not connected to Godot addon. Is the game running?`

---

## Quick Start — Director (Scene Manipulation)

Director gives AI agents the ability to create and modify Godot scenes, resources, tilemaps, and animations from the editor side.

See [`docs/director-spec.md`](docs/director-spec.md) for full documentation.

---

## Troubleshooting

**`[Spectator] GDExtension not loaded — SpectatorTCPServer class not found`** — the `.so` wasn't copied, is for the wrong platform, or was built against an incompatible Godot version. Rebuild and redeploy:
```bash
cargo build -p spectator-godot && ./scripts/copy-gdext.sh
```
Then verify with `godot --headless --quit --path /your/project 2>&1` — expect `TCP server listening` with no `SCRIPT ERROR` or `[panic]` lines.

**GDExtension panics at init (`failed to load class method ... hash`)** — the `.so` was compiled against a different Godot API version. The current build uses `api-4-5` with `lazy-function-tables`, which handles 4.2–4.6+. If you're on an older or newer Godot, rebuild from source after updating `api-4-5` in `crates/spectator-godot/Cargo.toml` to match your version.

**MCP server times out** — the game isn't running, or the addon didn't start (check Godot output for errors). The server retries the TCP connection every 2 seconds.

**Wrong port** — default is 9077. Override with `THEATRE_PORT=XXXX` for the server and `theatre/spectator/connection/port` in Godot Project Settings for the addon.

---

## Development

```bash
cargo build --workspace       # build everything
cargo test --workspace        # run all tests
cargo clippy --workspace      # lint
./scripts/copy-gdext.sh       # copy .so into addons/ within this repo
```

### theatre-deploy (recommended for active development)

`theatre-deploy` is a shell script that builds and copies the `.so` to one or more installed Godot projects in one command. Install it by symlinking `~/.local/bin/theatre-deploy` → `scripts/theatre-deploy`.

```bash
theatre-deploy                              # debug → default test project
theatre-deploy --release                   # release build
theatre-deploy ~/godot/a ~/godot/b         # deploy to multiple projects
theatre-deploy --release ~/godot/my-game   # release → specific project
```
