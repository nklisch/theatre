---
description: "Install Theatre CLI and the Godot GDExtension addon. Supports macOS, Linux, and Windows."
---

# Installation

This guide covers everything you need to install Theatre and connect it to your Godot project.

## Prerequisites

### Rust toolchain

Theatre is built in Rust. You need a recent stable toolchain (1.80 or later).

```bash
# Install rustup if you don't have it
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Verify
rustc --version   # should print 1.80.0 or later
cargo --version
```

### Godot 4.5 or later

Theatre's Stage GDExtension targets Godot 4.5+ with `compatibility_minimum = "4.5"`. The `api-4-5` feature flag in godot-rust requires Godot 4.5 as the minimum runtime version. Director works on any Godot version that supports GDScript plugins.

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

## Install (recommended)

The fastest way to install Theatre — downloads a pre-built release for your platform:

```bash
curl -LsSf https://github.com/nklisch/theatre/releases/latest/download/install.sh | sh
```

This detects your OS and architecture, downloads the correct release, verifies the SHA256 checksum, and installs to:
- `~/.local/bin/` — `theatre`, `stage`, `director` binaries
- `~/.local/share/theatre/` — addon templates and GDExtension binary

Override install locations with `--bin-dir` and `--share-dir` flags. Use `--no-modify-path` to skip adding `~/.local/bin` to your shell profile.

Supported platforms: Linux x86_64, macOS arm64, macOS x86_64 (Rosetta), Windows x86_64 (MINGW/MSYS).

### Install a specific version

```bash
curl -LsSf https://github.com/nklisch/theatre/releases/latest/download/install.sh | sh -s -- --version 0.2.2
```

## Install from source

If you prefer to build from source, the `theatre` CLI handles the entire process:

```bash
git clone https://github.com/nklisch/theatre
cd theatre
cargo run -p theatre-cli -- install
```

This builds all crates in release mode and installs to the same locations as the one-liner above.

If `~/.local/bin` is not in your PATH, the installer will print a warning with the export command to add.

### Platform notes

On **Linux**, the build works out of the box. On **macOS**, you may need the Xcode command-line tools (`xcode-select --install`). On **Windows**, use the MSVC toolchain (`rustup default stable-x86_64-pc-windows-msvc`).

## Set up a Godot project

After installing, use `theatre init` to set up a Godot project interactively:

```bash
theatre init ~/path/to/your-godot-project
```

This walks you through:
1. **Addon selection** — choose Stage, Director, or both
2. **MCP configuration** — generates `.mcp.json` with correct binary paths
3. **Plugin enabling** — updates `project.godot` to enable plugins and autoloads
4. **Agent rules** — optionally generates a rules file to prevent hand-editing `.tscn`/`.tres` files

For non-interactive setup (CI, scripting), use `--yes` to accept all defaults:

```bash
theatre init ~/path/to/your-godot-project --yes
```

### What `theatre init` does

- Copies addon files from `~/.local/share/theatre/addons/` to your project's `addons/` directory
- Copies the GDExtension binary (`.so`/`.dylib`/`.dll`) for Stage
- Generates `.mcp.json` with absolute paths to installed MCP server binaries
- Enables plugins in `project.godot` and adds the StageRuntime autoload
- Optionally generates an agent rules file (`.claude/rules/godot.md`, `CLAUDE.md`, or `AGENTS.md`) to prevent hand-editing Godot files

### Verify the deployment

Run Godot headless to confirm the extension loads without errors:

```bash
godot --headless --quit --path ~/path/to/your-godot-project 2>&1
```

Expected output should **not** contain `SCRIPT ERROR`, `[panic]`, or `ERROR`. You should see the Stage TCP server start and stop cleanly.

## Rebuild and redeploy

After making code changes, use `theatre deploy` to rebuild and update projects:

```bash
# Debug build → single project
theatre deploy ~/path/to/your-godot-project

# Release build → multiple projects
theatre deploy --release ~/godot/game1 ~/godot/game2
```

Deploy rebuilds the GDExtension and MCP servers, updates the share dir, and copies fresh files to all target projects.

## Enable/disable plugins

Toggle plugins without recopying addon files:

```bash
theatre enable ~/path/to/your-godot-project              # enable both
theatre enable ~/path/to/your-godot-project --stage   # stage only
theatre enable ~/path/to/your-godot-project --disable     # disable both
```

## Regenerate MCP config

If you skipped `.mcp.json` generation during `theatre init`, or need to update it after changing the port or binary location, run:

```bash
theatre mcp ~/path/to/your-godot-project
```

This generates (or overwrites) `.mcp.json` with the correct absolute paths to the installed `stage` and `director` binaries. It detects which addons are installed and includes only those in the config.

Use `--yes` to skip prompts (accepts port 9077 and overwrites any existing file):

```bash
theatre mcp ~/path/to/your-godot-project --yes
```

Use `--port` to specify a non-default port:

```bash
theatre mcp ~/path/to/your-godot-project --port 9078
```

After regenerating, restart your AI agent to pick up the updated server configuration.

## Manual setup (alternative)

If you prefer not to use the CLI, you can set things up manually.

### Build from source

```bash
git clone https://github.com/nklisch/theatre
cd theatre
cargo build --workspace --release
```

Build output:
- `target/release/stage` — Stage MCP server + CLI binary
- `target/release/director` — Director MCP server binary
- `target/release/libstage_godot.so` — Stage GDExtension (Linux)
- `target/release/libstage_godot.dylib` — Stage GDExtension (macOS)
- `target/release/stage_godot.dll` — Stage GDExtension (Windows)

### Copy addons

```bash
cp -r addons/stage ~/path/to/your-godot-project/addons/
cp -r addons/director ~/path/to/your-godot-project/addons/
```

Copy the GDExtension binary to the correct platform subdirectory:

```bash
mkdir -p ~/path/to/your-godot-project/addons/stage/bin/linux/
cp target/release/libstage_godot.so ~/path/to/your-godot-project/addons/stage/bin/linux/
```

Then in Godot: **Project → Project Settings → Plugins** → enable Stage and Director.

### Configure MCP

Create `.mcp.json` in your project root:

```json
{
  "mcpServers": {
    "stage": {
      "type": "stdio",
      "command": "/home/yourname/.local/bin/stage",
      "args": ["serve"]
    },
    "director": {
      "type": "stdio",
      "command": "/home/yourname/.local/bin/director",
      "args": ["serve"]
    }
  }
}
```

The `command` field must be an absolute path. Do not use `~` or relative paths — they are not expanded by most MCP launchers. Both binaries require the `serve` subcommand for MCP mode (without it, they run in CLI mode).

Use `THEATRE_PORT=9078` in an `env` block if you need a non-default port.

## Install agent skills (optional)

Theatre ships agent skills that teach AI agents how to use Stage and Director effectively — tool selection, parameter patterns, debugging workflows, and common pitfalls.

### Via skilltap (recommended)

[skilltap](https://skilltap.dev) is a package manager for agent skills. Install it first, then add the Theatre tap and install skills:

```bash
# Install all Theatre skills to the current project
skilltap install nklisch/theatre

# Or install globally (available to all projects)
skilltap install nklisch/theatre --global
```

You can also add Theatre as a tap for browsing and discovery:

```bash
skilltap tap add theatre nklisch/theatre
skilltap tap install   # interactive skill picker
```

Available skills:
- **theatre-stage** — 9 spatial observation tools for debugging a running Godot game: snapshots, deltas, queries, watches, clips, and live property mutation
- **theatre-director** — 38 Director tools for creating and modifying Godot scenes, nodes, resources, tilemaps, animations, and signals
- **godot-gdscript-patterns** — Godot 4 GDScript patterns: signals, state machines, object pooling, component systems, and performance tips

### Manual installation

Copy the skill directories from the Theatre repo directly:

```bash
# From within the theatre repo
cp -r .agents/skills/theatre-stage <your-project>/.agents/skills/
cp -r .agents/skills/theatre-director <your-project>/.agents/skills/
```

## Agent rules (recommended)

AI agents will sometimes try to directly edit `.tscn` and `.tres` files instead of using Director. This breaks UIDs, resource references, and Godot's serialization format. Theatre can generate a rules file that prevents this.

### Via the CLI

`theatre init` prompts for this automatically. To add rules to an existing project:

```bash
theatre rules ~/path/to/your-godot-project
```

This gives you three options:
- **`.claude/rules/godot.md`** — Claude Code auto-loads this (recommended for Claude Code users)
- **`CLAUDE.md`** — appends rules to your project's CLAUDE.md
- **`AGENTS.md`** — appends rules for non-Claude agents

Use `--yes` to skip prompts and generate `.claude/rules/godot.md`:

```bash
theatre rules ~/path/to/your-godot-project --yes
```

### Manual snippet

If you prefer to add the rules yourself, paste this into your project's `CLAUDE.md`, `AGENTS.md`, or `.claude/rules/godot.md`:

<<< @/../rules-template.md

## Using the CLI (alternative to MCP)

Both Stage and Director can be used as standalone CLIs without an MCP server. This is useful when your agent prefers shell commands over MCP, or for scripting.

```bash
# Stage — observe a running game
stage spatial_snapshot '{"detail": "summary"}'
stage spatial_inspect '{"node": "player"}'
stage scene_tree '{"action": "roots"}'

# Director — modify project files
director scene_create '{"project_path": "/home/user/game", "scene_path": "res://level.tscn", "root_type": "Node3D"}'
director scene_read '{"project_path": "/home/user/game", "scene_path": "res://level.tscn"}'

# Stdin piping works too
echo '{"detail": "summary"}' | stage spatial_snapshot

# Help and version
stage --help
director --help
stage --version   # {"version": "0.2.2"}
```

CLI output is always JSON to stdout. Errors are structured JSON with exit codes: 0 (success), 1 (runtime error), 2 (usage error).

## Verify the full setup

1. Open your Godot project in the editor
2. Run the game (F5 or the play button)
3. In your AI agent, ask: `"Take a spatial snapshot"`

The agent should call `spatial_snapshot` and return a JSON summary of your scene. If it times out or returns a connection error, check:

- The Stage addon is enabled and the extension loaded (check the editor dock)
- The game is actually running (not just the editor)
- Port 9077 is not blocked by a firewall

## Troubleshooting

### "Extension not found" in the dock

The GDExtension binary wasn't found or is for the wrong platform. Re-run `theatre deploy` and check the `addons/stage/bin/` directory contains the `.so`/`.dylib`/`.dll`.

### Connection refused / timeout

Stage only accepts connections while the game is running. Make sure you press F5 (Run project) before asking the agent for a snapshot.

### "SCRIPT ERROR: Parse error" in Godot

The GDScript addon has a syntax error, or it is referencing a GDExtension class that didn't load. Check the Godot output panel for the specific error. The addon is designed to gracefully degrade when the extension is missing — if you see a parse error, it is likely a version mismatch.

### Build fails: "linker not found"

On Linux, install `gcc` or `clang`: `sudo apt install build-essential` (Ubuntu) or `sudo dnf install gcc` (Fedora).

### Build fails on macOS: "xcrun: error"

Run `xcode-select --install` to install the command-line developer tools.
