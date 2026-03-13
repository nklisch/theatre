# Theatre — Agent Instructions

## What This Is

Theatre is a Godot AI agent toolkit containing two tools:
- **Spectator**: Rust MCP server + Rust GDExtension addon giving AI agents spatial awareness of running Godot games.
- **Director**: Rust MCP server + GDScript addon giving AI agents the ability to create and modify Godot scenes, resources, tilemaps, and animations.

Both tools communicate with Godot over TCP.

## Repository Layout

```
crates/
  spectator-server/     — Spectator MCP binary (rmcp + tokio), stdio transport
  spectator-godot/      — Spectator GDExtension cdylib (gdext), loaded by Godot
  spectator-protocol/   — Shared TCP wire format types
  spectator-core/       — Shared spatial logic (no Godot, no MCP)
  director/             — Director MCP binary
addons/spectator/       — Spectator Godot addon (GDScript + GDExtension manifest)
addons/director/        — Director Godot addon (GDScript)
docs/                   — Design documents & audit report
docs/design/            — Active (in-progress) designs
docs/design/completed/  — Archived completed designs (see warning below)
tests/
  wire-tests/           — Spectator E2E tests
  director-tests/       — Director E2E tests
```

## Build Commands

```bash
# Build everything
cargo build --workspace

# Build specific crate
cargo build -p spectator-server
cargo build -p spectator-godot
cargo build -p director

# Run ALL tests — unit + integration + scenarios + E2E journeys
# No feature flags — all tests run unconditionally
# E2E tests require deploying GDExtension first:
theatre-deploy ~/dev/spectator/tests/godot-project
cargo test --workspace
# IMPORTANT: All test layers must pass. Never skip E2E journey tests.

# Lint
cargo clippy --workspace
cargo fmt --check

# Copy GDExtension to addon dir within this repo (Linux)
./scripts/copy-gdext.sh          # debug
./scripts/copy-gdext.sh release  # release
```

## Deploying to a Godot Project

Use the `theatre-deploy` shell script (`scripts/theatre-deploy`, symlink to `~/.local/bin/theatre-deploy`) to
build and copy the `.so` into one or more Godot projects in one step:

```bash
# Debug build → default target (~/godot/test-harness)
theatre-deploy

# Release build → default target
theatre-deploy --release

# Debug build → specific project
theatre-deploy ~/godot/my-game

# Release build → multiple projects
theatre-deploy --release ~/godot/test-harness ~/godot/my-game
```

The script runs `cargo build -p spectator-godot` then copies
`target/<mode>/libspectator_godot.so` to
`<project>/addons/spectator/bin/linux/libspectator_godot.so`.

### Verifying the deployed build

```bash
godot --headless --quit --path ~/godot/test-harness 2>&1
```

Expected: no `SCRIPT ERROR` or `[panic]` lines; Spectator TCP server starts
and stops cleanly.

## GDExtension Compatibility

- `spectator-godot` targets `api-4-5` with `lazy-function-tables` enabled.
- `lazy-function-tables` defers method hash validation to first call, allowing
  the extension to load on Godot 4.2–4.6+ without panicking on method hash
  changes in classes spectator never uses.
- `compatibility_minimum = "4.2"` in `spectator.gdextension` remains accurate.
- To target a newer API, bump `api-4-5` to `api-4-6` (etc.) in
  `crates/spectator-godot/Cargo.toml` once godot-rust adds that feature flag.

## GDScript Adapter Notes

`addons/spectator/runtime.gd` avoids static type annotations for GDExtension
types (`SpectatorTCPServer`, `SpectatorCollector`, `SpectatorRecorder`) and
uses `ClassDB.instantiate(&"ClassName")` instead of `ClassName.new()`. This
prevents GDScript parse errors when the extension fails to load. The
`ClassDB.class_exists` guard provides the runtime safety check.

## Key Constraints

- **stdout is sacred**: spectator-server uses stdout for MCP protocol. ALL
  logging goes to stderr via `tracing` / `eprintln!`. Never use `println!`.
- **Main thread only**: spectator-godot runs on Godot's main thread. No
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
- `anyhow` for application errors in spectator-server
- `thiserror` or manual `impl Error` for library errors in protocol/core
- serde for all serialization, `#[serde(rename_all = "snake_case")]` for enums
- Tests alongside source in `#[cfg(test)] mod tests`
- No unwrap in library code; unwrap OK in tests and main.rs setup

## Architecture Rules

### Spectator
- spectator-godot depends on spectator-protocol, NOT on spectator-core
- spectator-server depends on both spectator-protocol and spectator-core
- spectator-core has zero Godot or MCP dependencies — pure logic
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
  for routine work. Example: `add SpectatorTCPServer handshake`
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

When you complete a meaningful unit of work, post an update using the
`post_update` MCP tool with the project ID above. Use status "in-progress"
for normal progress, "blocked" if you hit an obstacle, or "error" for
failures. Include relevant tags for categorization.
