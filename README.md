# Spectator

Gives AI agents spatial awareness of a running Godot game. A Rust MCP server connects to a GDExtension addon inside Godot via TCP, exposing the scene tree as MCP tools.

```
AI Agent (Claude, etc.)
    ↕ MCP over stdio
spectator-server  (Rust binary)
    ↕ TCP :9077
Godot Engine  (GDExtension addon)
```

## Prerequisites

- Rust (stable, 1.80+) — `rustup update stable`
- Godot 4.2+
- An MCP client — Claude Code, Claude Desktop, or any MCP-compatible agent

## Setup

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

### 2. Install the addon in your Godot project

Copy the `addons/spectator/` directory into your Godot project's `addons/` folder:

```bash
cp -r addons/spectator /path/to/your-godot-project/addons/
```

Then in Godot: **Project → Project Settings → Plugins → Spectator → Enable**

When the plugin is enabled it registers a `SpectatorRuntime` autoload that starts the TCP listener on port 9077. No scene changes are required.

### 3. Build the MCP server

```bash
cargo build -p spectator-server --release
```

The binary is at `target/release/spectator-server`.

### 4. Configure your MCP client

**Claude Code** — add to `.mcp.json` in your project root (or `~/.claude/mcp.json` for global):

```json
{
  "mcpServers": {
    "spectator": {
      "type": "stdio",
      "command": "/path/to/spectator/target/release/spectator-server"
    }
  }
}
```

**Claude Desktop** — add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "spectator": {
      "command": "/path/to/spectator/target/release/spectator-server",
      "args": []
    }
  }
}
```

Use `SPECTATOR_PORT=9078` in the env block if you need a non-default port (and set the same in Godot's Project Settings under `spectator/connection/port`).

## Testing

1. Open your Godot project and run the scene (Play button or F5)
2. You should see in Godot's output: `[SpectatorTCPServer] Listening on 127.0.0.1:9077`
3. Start a Claude Code session in your project directory
4. Ask: `call spatial_snapshot with detail summary`

Expected response: a JSON summary of the entities currently in the scene.

If the game isn't running, the tool returns: `Not connected to Godot addon. Is the game running?`

### Quick smoke test

```
# In Claude Code with spectator configured:
Use the spatial_snapshot tool with detail="summary" to show me what's in the scene.
```

A working response looks like:

```json
{
  "frame": 142,
  "entity_count": 8,
  "perspective": { "position": [0, 5, 10], ... },
  "clusters": [
    { "label": "enemies", "count": 3, "nearest_dist": 4.2 },
    { "label": "static_geometry", "count": 5 }
  ]
}
```

## Troubleshooting

**`SpectatorTCPServer class not found`** — the `.so` wasn't copied or is for the wrong platform. Rebuild and re-copy: `cargo build -p spectator-godot && ./scripts/copy-gdext.sh`

**MCP server times out** — the game isn't running, or the addon didn't start (check Godot output for errors). The server retries the TCP connection every 2 seconds.

**Wrong port** — default is 9077. Override with `SPECTATOR_PORT=XXXX` for the server and `spectator/connection/port` in Godot Project Settings for the addon.

## Development

```bash
cargo build --workspace       # build everything
cargo test --workspace        # run all tests
cargo clippy --workspace      # lint
./scripts/copy-gdext.sh       # update addon after rebuilding spectator-godot
```
