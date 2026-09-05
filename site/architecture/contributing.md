---
description: "Contributing to Theatre — development setup, code style, testing, and pull request guidelines."
---

# Contributing

How to build, test, and submit changes to Theatre.

## Getting started

### Prerequisites

- Rust 1.94+ (`rustup update stable`)
- Godot 4.7+ on your PATH (for E2E tests)
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

The E2E tests require a Godot project with the Stage GDExtension deployed. Deploy to the test project:

```bash
cargo run -p theatre-cli -- deploy tests/godot-project
```

This builds `stage-godot` and copies the `.so` to the test project's addon directory.

## Running tests

Run ordinary and explicitly ignored tests separately:

```bash
cargo test --workspace
cargo test --workspace -- --ignored --test-threads=1
```

**All required test layers must pass.** The ordinary workspace command does not
run environment-dependent tests marked `#[ignore]`. Those journeys also require
the relevant Stage payload, Godot editor, or graphical display.

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
# Ensure Godot is on PATH and required payloads are deployed
cargo test --workspace -- --ignored --test-threads=1
```

The E2E tests start a real Godot process, send tool calls, and verify responses. They test the full stack: Rust server ↔ TCP ↔ GDExtension ↔ Godot engine.

### Running specific tests

```bash
# All tests in one crate
cargo test -p stage-core

# Specific test by name
cargo test -p stage-server snapshot_budget_trimming

# E2E tests only
cargo test -p wire-tests -- --ignored --test-threads=1
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
- **`anyhow` for application errors** — in `stage-server` and `director` main/tools
- **`thiserror`** for library errors — in `stage-protocol`, `stage-core`
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
refactor: extract codec into stage-protocol
test: add E2E scenario for navmesh disconnection
```

## Project structure for new features

### Adding a new Stage tool

1. Define parameters beside the owning handler in `crates/stage-server/src/mcp/`.
2. Extend shared `stage-protocol` types when the engine boundary needs new data.
3. Add engine dispatch in `stage-godot` and pure reasoning in `stage-core` only where each belongs.
4. Register the handler and output schema in the Stage router.
5. Add the smallest useful unit, transport, and real-engine evidence.
6. Regenerate the public schema reference from the router.

### Adding a new Director operation

1. Define the Rust parameter and response types in `crates/director/src/mcp/`.
2. Add the Godot operation to the relevant `addons/director/ops/` module and shared dispatcher.
3. Preserve editor undo/explicit-save and headless persistence semantics.
4. Add focused Rust and real-Godot evidence at the affected boundary.
5. Regenerate the public schema reference from the router.

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

After changing `stage-godot`:

```bash
cargo run -p theatre-cli -- deploy tests/godot-project
# Then verify it loads:
godot --headless --quit --path tests/godot-project 2>&1
```

### Testing the MCP server manually

You can interact with the MCP server directly using JSON-RPC:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | \
  ./target/debug/stage serve
```

Or for a tool call (with game running):

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"spatial_snapshot","arguments":{"detail":"summary"}}}' | \
  ./target/debug/stage serve
```

You can also use the CLI mode directly (no JSON-RPC wrapping):

```bash
./target/debug/stage spatial_snapshot '{"detail":"summary"}'
```

### Viewing trace output

The server uses `tracing` for structured logging. Set the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug ./target/debug/stage serve
# Or for specific crates:
RUST_LOG=stage_server=trace ./target/debug/stage serve
```

All trace output goes to stderr, so it does not interfere with the MCP stdout protocol.
