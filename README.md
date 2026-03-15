# Theatre

AI agent toolkit for building and debugging Godot games. Two tools — **Stage** (observe & interact with a running game) and **Director** (create and modify scenes) — available via MCP or standalone CLI.

**Documentation**: [godot-theatre.dev](https://godot-theatre.dev)

## The Problem

AI coding agents can read source files and set breakpoints, but they **cannot see your game**. When a physics body tunnels through a wall, when an NPC takes a bizarre pathfinding route, when a signal fires at the wrong time — the agent has no way to observe it.

Theatre connects your agent to the running game through the **Model Context Protocol (MCP)**.

## Two Tools

| Tool | What it does | Interface |
|---|---|---|
| **Stage** | Observe and interact with a running Godot game — spatial snapshots, deltas, watches, clips, live property mutation | 9 MCP tools or `stage <tool> '<json>'` |
| **Director** | Create and modify Godot scenes, resources, tilemaps, and animations | 38 MCP tools or `director <tool> '<json>'` |

## Quick Start

### 1. Install

```bash
curl -LsSf https://github.com/nklisch/theatre/releases/latest/download/install.sh | sh
```

Or build from source:

```bash
git clone https://github.com/nklisch/theatre
cd theatre
cargo run -p theatre-cli -- install
```

Installs `theatre`, `stage`, and `director` binaries to `~/.local/bin/` and addon templates to `~/.local/share/theatre/`.

### 2. Set up your Godot project

```bash
theatre init ~/your-godot-project
```

Interactively copies addons, generates `.mcp.json`, enables plugins, and optionally sets up agent rules. Use `--yes` to skip prompts and accept all defaults.

### 3. Install agent skills (optional)

```bash
skilltap install nklisch/theatre
```

Teaches your agent how to use Stage and Director effectively — tool selection, workflows, and pitfalls. Install [skilltap](https://skilltap.dev) first.

### 4. Run your game and ask

Open your Godot project, press F5, then ask your agent: `"Take a spatial snapshot"`.

## Theatre CLI

| Command | Description |
|---|---|
| `theatre install` | Build and install to `~/.local/{bin,share}` |
| `theatre init <project>` | Interactive project setup (addons, `.mcp.json`, plugins, rules) |
| `theatre deploy <project...>` | Rebuild and redeploy to project(s) |
| `theatre enable <project>` | Enable or disable plugins in `project.godot` |
| `theatre rules <project>` | Generate agent rules file to prevent hand-editing Godot files |
| `theatre mcp <project>` | Generate or regenerate `.mcp.json` without reinstalling addons |

## Agent CLI

Both tools work as standalone CLIs — no MCP server required:

```bash
# Stage — observe a running game
stage spatial_snapshot '{"detail": "summary"}'
stage spatial_inspect '{"node": "player"}'
echo '{"action": "roots"}' | stage scene_tree

# Director — modify project files
director scene_create '{"project_path": "/path/to/game", "scene_path": "res://level.tscn", "root_type": "Node3D"}'
director node_add '{"project_path": "/path/to/game", "scene_path": "res://level.tscn", "parent_path": ".", "node_type": "Sprite2D", "node_name": "Hero"}'
```

Output is always JSON to stdout. Exit codes: 0 success, 1 runtime error, 2 usage error.

## Manual Setup

If you prefer not to use the `theatre` CLI:

```bash
# Build
cargo build --workspace --release

# Copy addons
cp -r addons/stage ~/your-project/addons/
cp -r addons/director ~/your-project/addons/

# Copy GDExtension binary (Linux)
mkdir -p ~/your-project/addons/stage/bin/linux/
cp target/release/libstage_godot.so ~/your-project/addons/stage/bin/linux/
```

Then enable both plugins in Godot: **Project → Project Settings → Plugins**.

Create `.mcp.json` in your project root:

```json
{
  "mcpServers": {
    "stage": {
      "type": "stdio",
      "command": "/absolute/path/to/stage",
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

Use absolute paths — `~` and relative paths are not expanded by most MCP launchers.

## Troubleshooting

**GDExtension not loaded** — the binary wasn't copied, is for the wrong platform, or was built against an incompatible Godot version. Rebuild and redeploy:
```bash
theatre deploy ~/your-project
```
Verify with `godot --headless --quit --path /your/project 2>&1` — expect no `SCRIPT ERROR` or `[panic]` lines.

**MCP server times out** — the game isn't running, or the addon didn't start. Check Godot output for errors. Stage retries the TCP connection every 2 seconds.

**Wrong port** — default is 9077. Override with `THEATRE_PORT=XXXX` for the MCP server and `theatre/stage/connection/port` in Godot Project Settings for the addon.

**GDExtension hash panic** — built against a different Godot API version. The current build targets `api-4-5` with `lazy-function-tables` for forward compatibility with 4.6+. Minimum supported version is Godot 4.5.

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

CI builds cross-platform binaries (Linux, macOS, Windows) and creates a GitHub release.
