# Installation

This guide covers everything you need to install Theatre and connect it to your Godot project.

## Prerequisites

### Rust toolchain

Theatre is built in Rust. You need a recent stable toolchain (1.75 or later).

```bash
# Install rustup if you don't have it
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Verify
rustc --version   # should print 1.75.0 or later
cargo --version
```

### Godot 4.2 or later

Theatre's Spectator GDExtension targets Godot 4.2+ with `compatibility_minimum = "4.2"`. It has been tested on 4.2, 4.3, and 4.4. Director works on any Godot version that supports GDScript plugins.

Make sure the `godot` binary is on your PATH if you want to run headless verification commands:

```bash
godot --version   # e.g. 4.3.stable.official
```

### An MCP-capable AI agent

Theatre exposes tools via [Model Context Protocol](https://modelcontextprotocol.io/). Supported agents include:

- **Claude Code** (recommended) — built-in MCP support
- **Cursor** — MCP support in recent versions
- **Windsurf** — MCP support via settings
- Any agent that supports stdio MCP servers

## Build from source

```bash
# Clone the repository
git clone https://github.com/nathanielfernandes/theatre
cd theatre

# Build everything (debug)
cargo build --workspace

# Build release binaries (recommended for normal use)
cargo build --workspace --release
```

Build output:
- `target/release/spectator-server` — Spectator MCP server binary
- `target/release/director` — Director MCP server binary
- `target/release/libspectator_godot.so` — Spectator GDExtension (Linux)
- `target/release/libspectator_godot.dylib` — Spectator GDExtension (macOS)
- `target/release/spectator_godot.dll` — Spectator GDExtension (Windows)

### Platform notes

On **Linux**, the build works out of the box. On **macOS**, you may need the Xcode command-line tools (`xcode-select --install`). On **Windows**, use the MSVC toolchain (`rustup default stable-x86_64-pc-windows-msvc`).

## Deploy Spectator to your Godot project

The `theatre-deploy` script builds and copies the GDExtension binary into your project in one step:

```bash
# Make the script executable and add to PATH
chmod +x scripts/theatre-deploy
ln -s "$(pwd)/scripts/theatre-deploy" ~/.local/bin/theatre-deploy

# Deploy to your project (release build)
theatre-deploy --release ~/path/to/your-godot-project

# Deploy to multiple projects at once
theatre-deploy --release ~/godot/game1 ~/godot/game2
```

The script:
1. Runs `cargo build -p spectator-godot --release`
2. Copies `libspectator_godot.so` (or `.dylib`/`.dll`) to `<project>/addons/spectator/bin/<platform>/`
3. Prints a success confirmation

### Verify the deployment

Run Godot headless to confirm the extension loads without errors:

```bash
godot --headless --quit --path ~/path/to/your-godot-project 2>&1
```

Expected output should **not** contain `SCRIPT ERROR`, `[panic]`, or `ERROR`. You should see the Spectator TCP server start and stop cleanly.

## Install the addons

### Spectator addon

The Spectator GDScript addon (in `addons/spectator/`) needs to be copied to your project:

```bash
cp -r addons/spectator ~/path/to/your-godot-project/addons/
```

Then in Godot:
1. Open **Project → Project Settings**
2. Go to the **Plugins** tab
3. Find **Spectator** and click **Enable**

The editor dock will appear on the right side. If the GDExtension loaded successfully, you'll see "Spectator: Ready" in the dock. If the extension failed to load, you'll see "Spectator: Extension not found" — go back and verify the `.so` was copied correctly.

### Director addon

```bash
cp -r addons/director ~/path/to/your-godot-project/addons/
```

Then in Godot:
1. Open **Project → Project Settings → Plugins**
2. Find **Director** and click **Enable**

Director does not require a GDExtension — it is pure GDScript. It starts a TCP listener on ports 6550 (editor plugin mode) and 6551 (daemon mode) when enabled.

## Configure your AI agent

Add Theatre to your project's MCP configuration. Most agents use `.mcp.json` or `mcp.json` in the project root or home directory.

### Claude Code

Create or edit `.mcp.json` in your project root:

```json
{
  "mcpServers": {
    "spectator": {
      "type": "stdio",
      "command": "/path/to/theatre/target/release/spectator-server"
    },
    "director": {
      "type": "stdio",
      "command": "/path/to/theatre/target/release/director",
      "args": ["serve"]
    }
  }
}
```

### Cursor

Add to your Cursor settings (`~/.cursor/mcp.json` or via the UI):

```json
{
  "mcpServers": {
    "spectator": {
      "command": "/path/to/theatre/target/release/spectator-server",
      "transport": "stdio"
    },
    "director": {
      "command": "/path/to/theatre/target/release/director",
      "args": ["serve"],
      "transport": "stdio"
    }
  }
}
```

### Using absolute paths

The `command` field must be an absolute path. If you build Theatre in `~/dev/theatre`, use:

```
/home/yourname/dev/theatre/target/release/spectator-server
```

Do not use `~` or relative paths — they are not expanded by most MCP launchers.

## Verify the full setup

1. Open your Godot project in the editor
2. Run the game (F5 or the play button)
3. In your AI agent, ask: `"Take a spatial snapshot"`

The agent should call `spatial_snapshot` and return a JSON summary of your scene. If it times out or returns a connection error, check:

- The Spectator addon is enabled and the extension loaded (check the editor dock)
- The game is actually running (not just the editor)
- Port 9077 is not blocked by a firewall

## Troubleshooting

### "Extension not found" in the dock

The GDExtension binary wasn't found or is for the wrong platform. Re-run `theatre-deploy` and check the `addons/spectator/bin/` directory contains the `.so`/`.dylib`/`.dll`.

### Connection refused / timeout

Spectator only accepts connections while the game is running. Make sure you press F5 (Run project) before asking the agent for a snapshot.

### "SCRIPT ERROR: Parse error" in Godot

The GDScript addon has a syntax error, or it is referencing a GDExtension class that didn't load. Check the Godot output panel for the specific error. The addon is designed to gracefully degrade when the extension is missing — if you see a parse error, it is likely a version mismatch.

### Build fails: "linker not found"

On Linux, install `gcc` or `clang`: `sudo apt install build-essential` (Ubuntu) or `sudo dnf install gcc` (Fedora).

### Build fails on macOS: "xcrun: error"

Run `xcode-select --install` to install the command-line developer tools.
