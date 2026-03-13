# Refactor Plan: Full Workspace Deduplication

## Summary

The codebase is fully compliant with all established patterns (no violations found). However, significant code duplication exists across the workspace, primarily in three areas: TCP wire format implementations, test harness infrastructure, and GDScript message codecs. This plan addresses each duplication with incremental, testable steps.

## Refactor Steps

### Step 1: Share async TCP codec in Director via spectator-protocol
**Priority**: High
**Risk**: Low
**Files**: `crates/director/Cargo.toml`, `crates/director/src/daemon.rs`, `crates/director/src/editor.rs`, `crates/spectator-protocol/Cargo.toml`

**Current State**: `daemon.rs:214-248` and `editor.rs:149-183` each implement their own `write_message()`/`read_message()` functions using the exact same 4-byte BE u32 + JSON wire format that already exists in `spectator-protocol::codec::async_io`. The only difference is the error type (`DaemonError`/`EditorError` vs `CodecError`).

**Target State**: Both `daemon.rs` and `editor.rs` call `spectator_protocol::codec::async_io::{read_message, write_message}` and map `CodecError` to their local error types at the call site.

**Approach**:
1. Add `spectator-protocol = { path = "../spectator-protocol", features = ["async"] }` to `crates/director/Cargo.toml`
2. Delete `write_message()` and `read_message()` from `daemon.rs`
3. Replace usages with `spectator_protocol::codec::async_io::write_message(&mut self.stream, &request).await.map_err(|e| DaemonError::IoError(...))?`
4. Same for `editor.rs`
5. Add `From<CodecError>` impls on `DaemonError` and `EditorError` if the mapping is cleaner that way

**Verification**:
- `cargo build -p director` compiles
- `cargo test -p director` passes (unit tests for port resolution)
- `cargo test -p director-tests` passes (E2E daemon/editor tests)

---

### Step 2: Share sync TCP codec in test harness via spectator-protocol
**Priority**: High
**Risk**: Low
**Files**: `tests/director-tests/Cargo.toml`, `tests/director-tests/src/harness.rs`

**Current State**: `harness.rs:481-500` implements `daemon_write_message()` and `daemon_read_message()` — sync versions of the same length-prefixed JSON protocol. `spectator_protocol::codec` already has sync `write_message()` and `read_message()`.

**Target State**: `DaemonFixture::run()` and `EditorFixture::run()` call `spectator_protocol::codec::{write_message, read_message}` directly, with error mapping to `anyhow`.

**Approach**:
1. Add `spectator-protocol` dependency to `tests/director-tests/Cargo.toml`
2. Replace `daemon_write_message`/`daemon_read_message` with `codec::write_message`/`codec::read_message` calls
3. Delete the two helper functions

**Verification**:
- `cargo build -p director-tests` compiles
- `cargo test -p director-tests` passes

---

### Step 3: Extract shared test utilities (assert_approx, project_dir, OperationResult)
**Priority**: High
**Risk**: Low
**Files**: `tests/wire-tests/src/harness.rs`, `tests/director-tests/src/harness.rs`, `tests/director-tests/Cargo.toml`

**Current State**:
- `assert_approx()` is identical in both harnesses (wire-tests:190-195, director-tests:161-167)
- `project_dir_path()` is identical in both harnesses (wire-tests:117-122, director-tests:153-159)
- `OperationResult` struct in `director-tests/harness.rs:18-29` duplicates `director::oneshot::OperationResult` (crates/director/src/oneshot.rs:5-16) with added test-only `unwrap_data()`/`unwrap_err()` convenience methods

**Target State**:
- `director-tests` imports `director::oneshot::OperationResult` and adds test-only unwrap methods via an extension trait
- `assert_approx` and `project_dir_path` remain in each harness (cross-crate test util would require a new crate — overkill for 2 functions)

**Approach**:
1. Add `director` as a dev-dependency of `director-tests`
2. Remove duplicate `OperationResult` struct from `harness.rs`
3. Create `trait OperationResultExt` in `harness.rs` with `unwrap_data()`/`unwrap_err()` methods
4. Import `use director::oneshot::OperationResult` and use the extension trait

**Verification**:
- `cargo build -p director-tests` compiles
- `cargo test -p director-tests` passes
- All E2E journey tests pass

---

### Step 4: Deduplicate DaemonFixture and EditorFixture ready-signal polling
**Priority**: Medium
**Risk**: Low
**Files**: `tests/director-tests/src/harness.rs`

**Current State**: `DaemonFixture::start_with_port()` (lines 191-251) and `EditorFixture::start_with_port()` (lines 323-381) are nearly identical — both spawn a Godot process, wait for the `{"source":"director","status":"ready"}` JSON signal on stdout, then connect TCP. The only differences are: the `--script` path and the env var name.

**Target State**: A shared `fn spawn_godot_fixture(script: &str, port_env: &str, port: u16) -> (Child, TcpStream)` helper extracts the common logic. Both fixtures call it.

**Approach**:
1. Extract `spawn_godot_fixture()` that takes script path, env var name, and port
2. Returns `(Child, TcpStream)` or panics (test code)
3. Refactor `DaemonFixture::start_with_port()` and `EditorFixture::start_with_port()` to call it
4. Keep the unique parts (DaemonFixture has `quit()`, EditorFixture doesn't)

**Verification**:
- `cargo test -p director-tests` passes
- Daemon and editor journey tests still work

---

### Step 5: Extract GDScript message codec to shared utility
**Priority**: Medium
**Risk**: Low
**Files**: `addons/director/daemon.gd`, `addons/director/plugin.gd`, `addons/director/mock_editor_server.gd`

**Current State**: `_try_decode_message()` and `_send_message()` are copy-pasted across all three GDScript files (daemon.gd:112-158, plugin.gd:91-125, mock_editor_server.gd:85-117). All three implement the same length-prefixed JSON codec.

**Target State**: A `addons/director/message_codec.gd` preload script with static functions `encode(data: Dictionary) -> PackedByteArray` and `try_decode(read_buf: PackedByteArray) -> Array` (returns `[decoded_dict, bytes_consumed]`). All three scripts preload and call it.

**Approach**:
1. Create `addons/director/message_codec.gd` with the codec logic extracted from daemon.gd
2. Refactor `daemon.gd` to use `MessageCodec.try_decode()` and `MessageCodec.encode()`
3. Refactor `plugin.gd` the same way
4. Refactor `mock_editor_server.gd` the same way
5. Keep `_read_buf` management (accumulation, trimming) in each script since it's tied to the stream lifecycle

**Verification**:
- `cargo test -p director-tests` E2E tests pass (daemon and editor fixtures exercise the real GDScript)
- Manual: `godot --headless --path tests/godot-project --script addons/director/daemon.gd` starts without errors

---

### Step 6: Name timeout constants in spectator-server
**Priority**: Low
**Risk**: Low
**Files**: `crates/spectator-server/src/tcp.rs`

**Current State**: Spectator server uses inline magic numbers for timeouts: `Duration::from_secs(10)` for handshake, `Duration::from_secs(5)` for query responses (tcp.rs, various lines). Director properly defines named constants (`READY_TIMEOUT`, `OPERATION_TIMEOUT`, `CONNECT_TIMEOUT`).

**Target State**: Named constants at module level in `tcp.rs`:
```rust
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const QUERY_TIMEOUT: Duration = Duration::from_secs(5);
const RECONNECT_DELAY: Duration = Duration::from_secs(2);
```

**Approach**:
1. Identify all timeout magic numbers in `tcp.rs`
2. Replace with named constants at module top
3. No behavioral change

**Verification**:
- `cargo build -p spectator-server` compiles
- `cargo test -p spectator-server` passes
- `cargo test -p wire-tests` passes

---

### Step 7: Centralize Director serde defaults
**Priority**: Low
**Risk**: Low
**Files**: `crates/director/src/mcp/node.rs`, `crates/director/src/mcp/defaults.rs` (new)

**Current State**: Director defines default functions inline below each params struct (e.g., `fn default_root() -> String` in node.rs:83). Multiple params structs use `default_root()` — it's defined once but could benefit from centralization as Director grows.

**Target State**: Create `crates/director/src/mcp/defaults.rs` collecting shared defaults, mirroring Spectator's `defaults.rs` pattern. Inline defaults that are used only once stay inline.

**Approach**:
1. Audit all `#[serde(default = "...")]` across Director params structs
2. Identify defaults used in 2+ structs (currently: `default_root`)
3. Move shared ones to `defaults.rs`
4. Keep single-use defaults inline

**Verification**:
- `cargo build -p director` compiles
- `cargo test --workspace` passes
