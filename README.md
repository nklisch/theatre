# Theatre

A toolkit giving AI agents the ability to build and debug Godot games. Two tools
— **Director** (build) and **Spectator** (observe) — available via MCP servers
or standalone CLI, working together or independently.

**Documentation**: [godot-theatre.dev](https://godot-theatre.dev)

## Tools

| Tool | Purpose | MCP | CLI | Godot Side |
|---|---|---|---|---|
| **Director** | Create/modify scenes, resources, tilemaps, animations | `director serve` | `director <tool> '<json>'` | GDScript addon |
| **Spectator** | Observe spatial state of a running game | `spectator serve` | `spectator <tool> '<json>'` | GDExtension + GDScript |

## Prerequisites

- Rust (stable, 1.80+) — `rustup update stable`
- Godot 4.5+ (GDExtension built with `api-4-5` + `lazy-function-tables` for forward compatibility with 4.6+)
- An MCP client or CLI access — Claude Code, Cursor, or any agent with bash/MCP

---

## Quick Start

### 1. Install Theatre

```bash
git clone https://github.com/nklisch/theatre
cd theatre
cargo run -p theatre-cli -- install
```

This builds all binaries in release mode and installs them to `~/.local/bin/`
and `~/.local/share/theatre/`. Make sure `~/.local/bin` is in your PATH.

### 2. Set up a Godot project

```bash
theatre init ~/path/to/your-godot-project
```

The interactive setup copies addons, generates `.mcp.json` for your AI agent,
and enables plugins in `project.godot`. Use `--yes` to accept all defaults.

### 3. Test it

1. Open your Godot project in the editor
2. Run the game (F5)
3. In your AI agent, ask: `"Take a spatial snapshot"`

The agent should return a JSON summary of the entities in your scene.

### 4. Install agent skills (optional)

```bash
npx skilltap install nklisch/theatre
```

Teaches your agent how to use Spectator and Director effectively — tool selection, workflows, parameter patterns, and debugging strategies.

---

## Agent CLI

Both tools work as standalone CLIs (no MCP server required):

```bash
# Spectator — observe a running game
spectator spatial_snapshot '{"detail": "summary"}'
spectator spatial_inspect '{"node": "player"}'
echo '{"action": "roots"}' | spectator scene_tree

# Director — modify project files
director scene_create '{"project_path": "~/game", "scene_path": "res://level.tscn", "root_type": "Node3D"}'
director node_add '{"project_path": "~/game", "scene_path": "res://level.tscn", "parent_path": ".", "node_type": "Sprite2D", "node_name": "Hero"}'
```

Output is always JSON to stdout. Exit codes: 0 success, 1 runtime error, 2 usage error.

---

## Theatre CLI Commands

| Command | Description |
|---|---|
| `theatre install` | Build and install to `~/.local/{bin,share}` |
| `theatre init <project>` | Interactive project setup (addons, `.mcp.json`, plugins) |
| `theatre deploy <project...>` | Rebuild and redeploy to project(s) |
| `theatre enable <project>` | Enable/disable plugins in `project.godot` |

### Rebuild after code changes

```bash
theatre deploy ~/path/to/your-godot-project           # debug build
theatre deploy --release ~/godot/game1 ~/godot/game2   # release, multiple projects
```

---

## Quick Start — Director (Scene Manipulation)

Director gives AI agents the ability to create and modify Godot scenes, resources, tilemaps, and animations from the editor side.

See [`docs/director-spec.md`](docs/director-spec.md) for full documentation.

---

## Manual Setup (without CLI)

If you prefer not to use the `theatre` CLI:

### Build

```bash
cargo build --workspace --release
```

### Deploy Spectator

```bash
# Copy addon files
cp -r addons/spectator ~/your-project/addons/

# Copy GDExtension binary
mkdir -p ~/your-project/addons/spectator/bin/linux/
cp target/release/libspectator_godot.so ~/your-project/addons/spectator/bin/linux/
```

### Deploy Director

```bash
cp -r addons/director ~/your-project/addons/
```

Then enable both plugins in Godot: **Project → Project Settings → Plugins**.

### Configure MCP

Create `.mcp.json` in your project root:

```json
{
  "mcpServers": {
    "spectator": {
      "type": "stdio",
      "command": "/absolute/path/to/spectator",
      "args": ["serve"]
    },
    "director": {
      "type": "stdio",
      "command": "/absolute/path/to/director",
      "args": ["serve"]
    }
  }
}
```

Both binaries require the `serve` subcommand for MCP mode.

Use absolute paths — `~` and relative paths are not expanded by most MCP launchers.

---

## Troubleshooting

**`[Spectator] GDExtension not loaded — SpectatorTCPServer class not found`** — the `.so` wasn't copied, is for the wrong platform, or was built against an incompatible Godot version. Rebuild and redeploy:
```bash
theatre deploy ~/your-project
```
Then verify with `godot --headless --quit --path /your/project 2>&1` — expect `TCP server listening` with no `SCRIPT ERROR` or `[panic]` lines.

**GDExtension panics at init (`failed to load class method ... hash`)** — the `.so` was compiled against a different Godot API version. The current build uses `api-4-5` with `lazy-function-tables` for forward compatibility with 4.6+. The minimum supported version is Godot 4.5. If you need an older version, rebuild from source after changing `api-4-5` in `crates/spectator-godot/Cargo.toml` to match (e.g. `api-4-3`), but note some features may not work.

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

### Releasing

```bash
./scripts/release.sh patch    # bumps version, commits, tags, pushes
```

The GitHub Actions workflow builds cross-platform binaries (Linux, macOS, Windows) and creates a release at [github.com/nklisch/theatre/releases](https://github.com/nklisch/theatre/releases).
