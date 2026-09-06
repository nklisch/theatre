---
description: "Install Theatre CLI and the Godot GDExtension addon. Supports macOS, Linux, and Windows."
---

# Installation

This guide covers everything you need to install Theatre and connect it to your Godot project.

## Prerequisites

### Rust toolchain

Theatre is built in Rust. You need a recent stable toolchain (1.94 or later).

```bash
# Install rustup if you don't have it
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Verify
rustc --version   # should print 1.94.0 or later
cargo --version
```

### Godot 4.7 or later

Theatre supports Godot 4.7 or later. Stage uses godot-rust 0.5.5 with
`api-4-7`, `experimental-godot-api`, and `compatibility_minimum = "4.7"`.
Director also uses native editor APIs from this engine floor. The gdext dependency
requires Rust 1.94 or later; current local engine verification used Rust 1.96.1.

Make sure the Godot binary is on your `PATH`, or pass it to initialization with
`--godot-bin`, if you want Theatre to run the initial import and headless
verification commands:

```bash
godot --version   # e.g. 4.7.1.stable.official
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
- `~/.local/share/theatre/` — addon templates, shared support payload, GDExtension binary, and optional client packages

Override install locations with `--bin-dir` and `--share-dir` flags. Use `--no-modify-path` to skip adding `~/.local/bin` to your shell profile.

Supported platforms: Linux x86_64, macOS arm64, macOS x86_64 (Rosetta), Windows x86_64 (MINGW/MSYS).

### Install a specific version

```bash
curl -LsSf https://github.com/nklisch/theatre/releases/latest/download/install.sh | sh -s -- --version 0.5.0
```

## Install from source

If you prefer to build from source, the `theatre` CLI handles the entire process:

```bash
git clone https://github.com/nklisch/theatre
cd theatre
cargo run -p theatre-cli -- install
```

This builds all crates in release mode and installs to the same locations as the one-liner above.

If the configured binary directory is not in your `PATH`, the installer prints
the appropriate command: PowerShell user-environment guidance on Windows or an
`export PATH=...` command on Unix.

### Platform notes

On **Linux**, the build works out of the box. On **macOS**, you may need the Xcode command-line tools (`xcode-select --install`). On **Windows**, use the MSVC toolchain (`rustup default stable-x86_64-pc-windows-msvc`).

The source install writes `stage.exe`, `director.exe`, and `theatre.exe` on
Windows. Unix executable names remain extensionless.

## Set up a Godot project

After installing, use `theatre init` to set up a Godot project interactively:

```bash
theatre init ~/path/to/your-godot-project
```

PowerShell:

```powershell
theatre init .\path\to\your-godot-project
```

This walks you through:
1. **Addon selection** — choose Stage, Director, or both
2. **MCP configuration** — generates portable `.mcp.json` commands resolved through `PATH`
3. **Plugin enabling** — updates `project.godot` to enable plugins and autoloads
4. **Agent rules** — optionally generates a rules file to guide native authoring of `.tscn`/`.tres` files

For non-interactive setup (CI, scripting), use `--yes` to accept all defaults:

```bash
theatre init ~/path/to/your-godot-project --yes
```

If Godot is not on `PATH`, provide it for the bounded initial import:

```powershell
theatre init .\path\to\your-godot-project --yes --godot-bin 'C:\path\to\Godot_console.exe'
```

This argument is used only for initialization. If Director also needs that
location, set `GODOT_BIN` in the environment that launches your AI agent; do
not add a machine-specific path to tracked `.mcp.json`.

### What `theatre init` does

- Copies selected addon files and `addons/theatre_shared` support into your project's `addons/` directory
- Copies the GDExtension binary (`.so`/`.dylib`/`.dll`) for Stage
- Generates `.mcp.json` with the bare `stage` and `director` commands
- Enables plugins in `project.godot` and adds the StageRuntime autoload
- Optionally generates an agent rules file (`.claude/rules/godot.md`, `CLAUDE.md`, or `AGENTS.md`) to prevent hand-editing Godot files
- Runs one bounded headless editor import when Godot can be resolved; otherwise prints the manual recovery command

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

This generates (or overwrites) `.mcp.json` with the portable bare commands
`stage` and `director`. It detects which addons are installed and includes only
those in the config. The configured Theatre binary directory must be on the
`PATH` inherited by the agent process.

Use `--yes` to skip prompts (accepts port 9077 and overwrites any existing file):

```bash
theatre mcp ~/path/to/your-godot-project --yes
```

Use `--port` to specify a non-default port:

```bash
theatre mcp ~/path/to/your-godot-project --port 9078
```

After regenerating, restart your AI agent to pick up the updated server configuration.

### Use a nested sandbox or switch projects

Run `theatre init /absolute/path/to/project` once for each Godot project that
needs Theatre's addons. If you start the agent from a repository root whose MCP
configuration already points at a nested sandbox, keep that root configuration;
do not also load the nested generated `.mcp.json` or duplicate generated rules.

Stage selects a project from `THEATRE_PROJECT_DIR` when its server starts. Set
that environment value in the Stage MCP entry, then restart the MCP connection
or start a new agent session. Changing the file does not retarget an existing
Stage process. That MCP `env` applies only to the Stage subprocess. To make the
optional feedback hook select the same nested project from a repository-root
session, launch the client with the absolute project environment too:

```bash
THEATRE_PROJECT_DIR=/absolute/path/to/project claude --plugin-dir "$HOME/.local/share/theatre/client-plugins/claude"
THEATRE_PROJECT_DIR=/absolute/path/to/project codex
```

An explicit hook selection is authoritative; if it is wrong, the hook stays
quiet rather than surfacing an ancestor project's queue. When it is unset, the
hook keeps the existing nearest-ancestor project lookup. For a one-off Stage CLI
call, override selection directly:

```bash
THEATRE_PROJECT_DIR=/absolute/path/to/project stage runtime_status '{}'
```

Director selects independently: pass the absolute Godot project directory as
`project_path` on every call. Consecutive Director calls can target different
projects without restarting the server.

The default Stage and Director ports are shared local resources. Stop the old
running game before starting another project on the same ports, then verify
Stage's reported project, scene, and run with `runtime_status`. Follow the target
repository's ownership rules when setup files are generated; update the owning
generator rather than making a durable direct edit to its output.

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
cp -r addons/theatre_shared ~/path/to/your-godot-project/addons/
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
      "command": "stage",
      "args": ["serve"]
    },
    "director": {
      "type": "stdio",
      "command": "director",
      "args": ["serve"]
    }
  }
}
```

Use the bare commands shown above so checked-in configuration works across
machines and operating systems. Ensure the binary directory is on the agent
process's `PATH`. Both binaries require the `serve` subcommand for MCP mode
(without it, they run in CLI mode).

Use `THEATRE_PORT=9078` in an `env` block if you need a non-default port.

## Install client plugins and agent skills (optional)

The Theatre distribution includes self-contained Claude and Codex plugin roots
under `~/.local/share/theatre/client-plugins/`. Both retain the plugin ID
`theatre-feedback`, bundle the `theatre-stage` and `theatre-director` operating
skills, and provide the optional pending-feedback hook. They do not register
Stage or Director MCP servers; keep using the project's existing `.mcp.json`.

Claude Code can load the installed plugin root for a session without modifying a
marketplace:

```bash
claude --plugin-dir "$HOME/.local/share/theatre/client-plugins/claude"
```

For Codex, the containing directory is a local marketplace:

```bash
codex plugin marketplace add "$HOME/.local/share/theatre/client-plugins"
codex plugin add theatre-feedback@theatre-local
```

Plugin installation and hook trust remain explicit client actions. Restart or
reload the client when it requires that to discover newly installed skills. The
plugins add operating guidance and a feedback notice only; they do not replace
`theatre init`, change the active Godot project, or deliver feedback images.

If a client does not load native plugins, install the same operating skills in
the project instead. Project skills are also useful when guidance should travel
with a repository:

```bash
# From within the Theatre source checkout
cp -r .agents/skills/theatre-stage <your-project>/.agents/skills/
cp -r .agents/skills/theatre-director <your-project>/.agents/skills/
```

A client may discover both plugin and project copies. They describe the same
Theatre tools; do not duplicate MCP registration or agent rules because both are
present. The canonical source is `.agents/skills/` in the Theatre repository,
and distributed plugin copies are synchronized from it.

For broader Godot coding guidance, `godot-gdscript-patterns` remains available
through the Theatre skilltap collection:

```bash
skilltap install nklisch/theatre
```

## Agent rules (recommended)

Read and diff Godot files freely. Prefer Director for structural scene and resource edits so Godot validates types, references, ownership, and serialization. Theatre can generate this guidance for your agent.

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

Both Stage and Director support standalone CLI calls. Stage CLI calls are one-shot: they do not share session state. Use persistent MCP for deltas, watches, session configuration updates, and actions with `return_delta`. The CLI rejects these requests before connection or mutation with `persistent_session_required` and exit code 2. Empty configuration reads, ordinary observations/actions, and clip operations remain available.

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
stage --version   # {"version": "0.5.0"}
```

CLI output is JSON to stdout. Exit codes 1 and 2 report runtime and usage
failures. Director can also return a structured operation result with
`"success": false` at exit code 0, so inspect `success`, `error`, `context`, and
`persistence` — including ordered per-entry batch results — before deciding what
succeeded or should be retried.

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
