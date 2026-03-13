# Contributing

How to build, test, and submit changes to Theatre.

## Getting started

### Prerequisites

- Rust 1.75+ (`rustup update stable`)
- Godot 4.2+ on your PATH (for E2E tests)
- `cargo` (comes with Rust)
- Linux, macOS, or Windows (Linux is the primary development platform)

### Clone and build

```bash
git clone https://github.com/nklisch/theatre
cd theatre

# Build everything (debug)
cargo build --workspace

# Build release
cargo build --workspace --release
```

### First-time setup for tests

The E2E tests require a Godot project with the Spectator GDExtension deployed. Deploy to the test project:

```bash
theatre-deploy ~/dev/theatre/tests/godot-project
```

Or using the script directly:

```bash
./scripts/theatre-deploy ~/dev/theatre/tests/godot-project
```

This builds `spectator-godot` and copies the `.so` to the test project's addon directory.

## Running tests

Run all tests — unit, integration, scenario, and E2E — with one command:

```bash
cargo test --workspace
```

**All test layers must pass.** Do not skip E2E tests when submitting a PR. The E2E tests are marked `#[ignore = "requires Godot binary"]` so they only run if `godot` is on your PATH and the test project has the extension deployed.

### Test layers

**Unit tests** — in `#[cfg(test)] mod tests` blocks, co-located with source:
```bash
cargo test --workspace --lib
```

**Integration tests** — in `tests/` directories within each crate:
```bash
cargo test --workspace --test '*'
```

**E2E tests** — require Godot:
```bash
# Ensure godot is on PATH and extension is deployed
cargo test --workspace -- --include-ignored
```

The E2E tests start a real Godot process, send tool calls, and verify responses. They test the full stack: Rust server ↔ TCP ↔ GDExtension ↔ Godot engine.

### Running specific tests

```bash
# All tests in one crate
cargo test -p spectator-core

# Specific test by name
cargo test -p spectator-server snapshot_budget_trimming

# E2E tests only
cargo test -p wire-tests -- --include-ignored
```

## Linting

Before submitting a PR, run:

```bash
# Check formatting
cargo fmt --check

# Run clippy (no warnings allowed)
cargo clippy --workspace -- -D warnings
```

Apply formatting automatically:

```bash
cargo fmt
```

Clippy warnings are treated as errors in CI. Fix all warnings before opening a PR.

## Code style

### Rust conventions

- **Edition 2024** for all crates
- **`tracing` for all logging** — never `println!` in library code; never in server code (stdout is MCP protocol). Use `eprintln!` only for one-off debug prints that you will remove before committing.
- **`anyhow` for application errors** — in `spectator-server` and `director` main/tools
- **`thiserror`** for library errors — in `spectator-protocol`, `spectator-core`
- **No `unwrap()` in library code** — use `?` or explicit error handling. `unwrap()` is acceptable in tests and `main()` setup.
- **`serde(rename_all = "snake_case")`** for enums; `serde(tag = "type")` for protocol message enums

### Test style

- Tests live in `#[cfg(test)] mod tests` inside the source file they test
- Use small builder functions for test fixtures (`fn make_entity(...)`, not test frameworks)
- File I/O tests use `tempfile::TempDir`
- E2E tests are marked `#[ignore = "requires Godot binary"]`
- Never gate tests behind feature flags — all tests run unconditionally

### Commit messages

- Short imperative subject line, ≤72 characters
- No body needed for routine changes
- No `Co-Authored-By: Claude` or AI attribution footers

Examples:
```
add spatial_watch delete action
fix budget trimmer excluding focus_node on truncation
refactor: extract codec into spectator-protocol
test: add E2E scenario for navmesh disconnection
```

## Project structure for new features

### Adding a new Spectator tool

1. Add request/response types to `crates/spectator-protocol/src/messages.rs`
2. Add GDExtension handler in `crates/spectator-godot/src/tcp_server.rs`
3. Add any pure-logic in `crates/spectator-core/`
4. Add MCP tool handler in `crates/spectator-server/src/tools/<tool_name>.rs`
5. Register the tool in `crates/spectator-server/src/main.rs`
6. Add unit tests to the relevant crates
7. Add an E2E test in `tests/wire-tests/`

### Adding a new Director operation

1. Add the operation to the Director GDScript addon (`addons/director/plugin.gd`)
2. Add the MCP tool handler in `crates/director/src/tools/`
3. Add tests in `tests/director-tests/`

## Pull request checklist

- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo test --workspace` passes (all test layers)
- [ ] E2E tests pass with Godot binary on PATH
- [ ] No `println!` in server or library code
- [ ] No `unwrap()` in library code
- [ ] New tools/operations have unit tests
- [ ] Wire format changes are documented in the PR description
- [ ] Commit messages follow the project style

## Common development tasks

### Deploying changes to the test project

After changing `spectator-godot`:

```bash
theatre-deploy ~/dev/theatre/tests/godot-project
# Then verify it loads:
godot --headless --quit --path ~/dev/theatre/tests/godot-project 2>&1
```

### Testing the MCP server manually

You can interact with the MCP server directly using JSON-RPC:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | \
  ./target/debug/spectator-server
```

Or for a tool call (with game running):

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"spatial_snapshot","arguments":{"detail":"summary"}}}' | \
  ./target/debug/spectator-server
```

### Viewing trace output

The server uses `tracing` for structured logging. Set the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug ./target/debug/spectator-server
# Or for specific crates:
RUST_LOG=spectator_server=trace ./target/debug/spectator-server
```

All trace output goes to stderr, so it does not interfere with the MCP stdout protocol.
