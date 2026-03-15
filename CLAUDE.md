# Theatre — Agent Instructions

## What This Is

Theatre is a Godot AI agent toolkit containing two tools:
- **Stage**: Rust MCP server + Rust GDExtension addon giving AI agents spatial awareness of running Godot games.
- **Director**: Rust MCP server + GDScript addon giving AI agents the ability to create and modify Godot scenes, resources, tilemaps, and animations.

Both tools communicate with Godot over TCP.

## Repository Layout

```
crates/
  stage-server/         — Stage MCP binary (rmcp + tokio), stdio transport
  stage-godot/          — Stage GDExtension cdylib (gdext), loaded by Godot
  stage-protocol/       — Shared TCP wire format types
  stage-core/           — Shared spatial logic (no Godot, no MCP)
  director/             — Director MCP binary
  theatre-cli/          — CLI binary: install, init, deploy, enable (clap + dialoguer)
addons/stage/           — Stage Godot addon (GDScript + GDExtension manifest)
addons/director/        — Director Godot addon (GDScript)
docs/                   — Design documents & audit report
docs/design/            — Active (in-progress) designs
docs/design/completed/  — Archived completed designs (see warning below)
scripts/                — Release and install helper scripts
tests/
  wire-tests/           — Stage E2E tests
  director-tests/       — Director E2E tests
```

## Build Commands

```bash
# Build everything
cargo build --workspace

# Build specific crate
cargo build -p stage-server
cargo build -p stage-godot
cargo build -p director
cargo build -p theatre-cli

# Run ALL tests — unit + integration + scenarios + E2E journeys
# No feature flags — all tests run unconditionally
# E2E tests require deploying GDExtension first:
theatre deploy ~/dev/theatre/tests/godot-project
cargo test --workspace
# IMPORTANT: All test layers must pass. Never skip E2E journey tests.

# Lint
cargo clippy --workspace
cargo fmt --check

# Copy GDExtension to addon dir within this repo (Linux)
./scripts/copy-gdext.sh          # debug
./scripts/copy-gdext.sh release  # release
```

## Theatre CLI

The `theatre` CLI (`crates/theatre-cli`) unifies installation, project setup,
and deployment. Binary name is `theatre`; crate name is `theatre-cli`.

```bash
theatre install                  # build release + copy to ~/.local/{bin,share}
theatre init ~/godot/my-game     # interactive project setup (addons, .mcp.json, plugins)
theatre deploy ~/godot/my-game   # rebuild + redeploy to project(s)
theatre enable ~/godot/my-game   # non-interactive plugin enable/disable
```

- `install` builds from source and populates `~/.local/bin/` (binaries) and
  `~/.local/share/theatre/` (addon templates + GDExtension).
- `init` reads from the installed share dir (not the repo). Copies addons,
  generates `.mcp.json`, enables plugins. Interactive by default, `--yes` for
  non-interactive.
- `deploy` rebuilds from source, updates the share dir, then copies to
  target project(s). Accepts `--release`.
- `enable` toggles plugins in `project.godot` without copying files.

Dependencies: `clap` + `dialoguer` + `console` + `serde_json` + `anyhow`.
No tokio, no rmcp, no Godot deps.

See `docs/design/theatre-cli.md` for the full design.

### Legacy: theatre-deploy shell script

The old `theatre-deploy` shell script (`scripts/theatre-deploy`) still works
but is superseded by `theatre deploy`. Use `theatre deploy` for new workflows.

### Verifying the deployed build

```bash
godot --headless --quit --path ~/godot/test-harness 2>&1
```

Expected: no `SCRIPT ERROR` or `[panic]` lines; Stage TCP server starts
and stops cleanly.

## GDExtension Compatibility

- `stage-godot` targets `api-4-5` with `lazy-function-tables` enabled.
- `lazy-function-tables` defers method hash validation to first call, providing
  forward compatibility with Godot 4.6+ without panicking on method hash
  changes in classes stage never uses.
- `compatibility_minimum = "4.5"` in `stage.gdextension`. The `api-4-5`
  feature flag requires Godot 4.5+ at runtime (API version ≤ runtime version).
- To target a newer API, bump `api-4-5` to `api-4-6` (etc.) in
  `crates/stage-godot/Cargo.toml` once godot-rust adds that feature flag.

## GDScript Adapter Notes

`addons/stage/runtime.gd` uses typed annotations for GDExtension types
(`var tcp_server: StageTCPServer`, etc.) and direct constructors
(`StageTCPServer.new()`, etc.). The `ClassDB.class_exists` guard checks
whether the extension loaded before attempting instantiation, providing
graceful degradation if the binary is missing.

## Key Constraints

- **stdout is sacred**: `stage serve` uses stdout for MCP protocol. In CLI
  mode (`stage <tool>`), stdout carries JSON results. ALL logging goes to
  stderr via `tracing` / `eprintln!`. Never use `println!` for log messages.
- **Main thread only**: stage-godot runs on Godot's main thread. No
  `Gd<T>` across thread boundaries. All scene tree access in _physics_process.
- **GDExtension ≠ EditorPlugin**: GDExtension classes can't be EditorPlugin
  bases (godot#85268). GDScript `plugin.gd` is the EditorPlugin; Rust classes
  are instantiated by it.
- **Thin addon**: GDExtension answers "what does the engine say?" The server
  does all spatial reasoning, budgeting, diffing, indexing.

## Code Style

- Rust edition 2024, workspace versioning
- `tracing` for all logging (never `println!`, use `eprintln!` only for
  one-off debugging)
- `anyhow` for application errors in stage-server
- `thiserror` or manual `impl Error` for library errors in protocol/core
- serde for all serialization, `#[serde(rename_all = "snake_case")]` for enums
- Tests alongside source in `#[cfg(test)] mod tests`
- No unwrap in library code; unwrap OK in tests and main.rs setup

## Architecture Rules

### Stage
- stage-godot depends on stage-protocol, NOT on stage-core
- stage-server depends on both stage-protocol and stage-core
- stage-core has zero Godot or MCP dependencies — pure logic
- TCP protocol: length-prefixed JSON (4-byte BE u32 + JSON payload)
- Addon listens (port 9077), server connects (ephemeral)

### Director
- director crate is the MCP binary; addon is pure GDScript
- Director talks to Godot headless via subprocess or daemon TCP connection
- See `docs/director-spec.md` for full Director architecture

## Documentation Trust Levels

- **Code is always ground truth.** When docs and code disagree, the code wins.
- **`docs/design/completed/`**: Archived design docs for implemented features.
  These reflect the *design intent at planning time*, NOT the current
  implementation. Field names, parameter lists, response shapes, and
  architectural details may have diverged during implementation. **Never treat
  completed designs as accurate API reference.** Always verify against the
  actual Rust structs and handler code.
- **`site/`**: Public-facing docs (VitePress). Being aligned with code as of
  the docs audit (2026-03-13) — see `docs/DOCS-AUDIT-REPORT.md` for the full
  discrepancy list and fix plan.
- **`docs/design/`** (non-completed): Active designs for in-progress work.
  These are closer to current intent but still require code verification.

## Git Conventions

- Commit messages: short imperative subject line (≤72 chars), no body needed
  for routine work. Example: `add StageTCPServer handshake`
- Do NOT add `Co-Authored-By: Claude` trailers to commits
- Do NOT add AI attribution footers of any kind

## Agent Tracker
- Project ID: fa156e12-1215-491d-88f8-3738d27f3d37
- Project Name: theatre
- Tracker URL: http://localhost:57328/mcp

When you complete a meaningful unit of work, post an update using the
`post_update` MCP tool with the project ID above. Use status "in-progress"
for normal progress, "blocked" if you hit an obstacle, or "error" for
failures. Include relevant tags for categorization.

## Releasing a New Version

Use the release script to automate version bumping, tagging, and pushing:

```bash
./scripts/release.sh patch    # 0.1.0 → 0.1.1
./scripts/release.sh minor    # 0.1.0 → 0.2.0
./scripts/release.sh major    # 0.1.0 → 1.0.0
./scripts/release.sh 2.0.0    # explicit version
```

The script syncs version strings across all locations, commits, tags, and
pushes. The `release.yml` GitHub Actions workflow then builds cross-platform
binaries (Linux x86_64, macOS arm64, Windows x86_64) and creates a GitHub
Release with tarballs containing binaries + addons + install script.

Files updated by the release script:
- `Cargo.toml` — workspace version (all Rust crates inherit automatically)
- `addons/stage/plugin.cfg` — Godot plugin version
- `addons/director/plugin.cfg` — Godot plugin version
- `site/changelog.md` — new version header + footer compare links
- `site/guide/installation.md` — version in CLI output examples
- `site/api/wire-format.md` — handshake version examples
- `Cargo.lock` — regenerated

**Do not update version strings manually.** Always use the release script to
keep everything in sync. If you need to add a new versioned location, add a
`sed` rule to `scripts/release.sh`.

Verify release at https://github.com/nklisch/theatre/releases
