# Spectator — Agent Instructions

## What This Is

Spectator: Rust MCP server + Rust GDExtension addon giving AI agents spatial
awareness of running Godot games. Two Rust compilation targets that communicate
over TCP.

## Repository Layout

```
crates/
  spectator-server/     — MCP binary (rmcp + tokio), stdio transport
  spectator-godot/      — GDExtension cdylib (gdext), loaded by Godot
  spectator-protocol/   — Shared TCP wire format types
  spectator-core/       — Shared spatial logic (no Godot, no MCP)
addons/spectator/       — Godot addon (GDScript + GDExtension manifest)
docs/                   — Design documents
docs/design/            — Implementation designs per milestone
```

## Build Commands

```bash
# Build everything
cargo build --workspace

# Build specific crate
cargo build -p spectator-server
cargo build -p spectator-godot

# Run ALL tests — unit + integration + scenarios + E2E journeys
# No feature flags — all tests run unconditionally
# E2E tests require deploying GDExtension first:
spectator-deploy ~/dev/spectator/tests/godot-project
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

Use the `spectator-deploy` shell script (`~/.local/bin/spectator-deploy`) to
build and copy the `.so` into one or more Godot projects in one step:

```bash
# Debug build → default target (~/godot/test-harness)
spectator-deploy

# Release build → default target
spectator-deploy --release

# Debug build → specific project
spectator-deploy ~/godot/my-game

# Release build → multiple projects
spectator-deploy --release ~/godot/test-harness ~/godot/my-game
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

- spectator-godot depends on spectator-protocol, NOT on spectator-core
- spectator-server depends on both spectator-protocol and spectator-core
- spectator-core has zero Godot or MCP dependencies — pure logic
- TCP protocol: length-prefixed JSON (4-byte BE u32 + JSON payload)
- Addon listens (port 9077), server connects (ephemeral)

## Git Conventions

- Commit messages: short imperative subject line (≤72 chars), no body needed
  for routine work. Example: `add SpectatorTCPServer handshake`
- Do NOT add `Co-Authored-By: Claude` trailers to commits
- Do NOT add AI attribution footers of any kind
