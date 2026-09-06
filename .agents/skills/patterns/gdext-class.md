# Pattern: GDExtension Class Export

Rust classes exported to Godot use `#[derive(GodotClass)]` + `#[class(base = Node)]`, implement the `INode` lifecycle trait via `#[godot_api] impl INode`, and expose methods/signals in a separate `#[godot_api] impl` block with `#[func]` and `#[signal]` decorators.

## Rationale
GDExtension registers classes at library load time through gdext's macro system. The split between `INode` (lifecycle) and the main `impl` (exported API) keeps Godot lifecycle concerns separate from application logic. `Gd<T>` is the only safe way to hold references to Godot objects.

## Examples

### Example 1: StageTCPServer — full class with signal and multiple funcs
**File**: `crates/stage-godot/src/tcp_server.rs:62-113`
```rust
#[derive(GodotClass)]
#[class(base = Node)]
pub struct StageTCPServer {
    base: Base<Node>,
    listener: Option<TcpListener>,
    clients: Vec<Option<ClientSlot>>,
    port: i32,
    conn_state: ConnectionState,
    pending_action: Option<PendingAction>,
    collector: Option<Gd<StageCollector>>,
    recorder: Option<Gd<StageRecorder>>,
    runtime_logger: Option<Gd<Object>>,
    client_idle_timeout_secs: u64,
}

#[godot_api]
impl INode for StageTCPServer {
    fn init(base: Base<Node>) -> Self {
        Self { base, listener: None, clients: Vec::new(), port: 9077, conn_state: ConnectionState::default(), pending_action: None, collector: None, recorder: None, runtime_logger: None, client_idle_timeout_secs: 10, ... }
    }
}

#[godot_api]
impl StageTCPServer {
    #[signal]
    fn activity_received(entry_type: GString, summary: GString, tool_name: GString, active_watches: i64);

    #[func]
    pub fn set_collector(&mut self, collector: Gd<StageCollector>) {
        self.collector = Some(collector);
    }

    #[func]
    pub fn get_connection_status(&self) -> GString { ... }

    #[func]
    pub fn start(&mut self, port: i32) { ... }
}
```

### Example 2: StageCollector — class with cross-reference to another GdClass
**File**: `crates/stage-godot/src/collector.rs:28-67`
```rust
#[derive(GodotClass)]
#[class(base = Node)]
pub struct StageCollector {
    base: Base<Node>,
    // ...fields
}

#[godot_api]
impl INode for StageCollector {
    fn init(base: Base<Node>) -> Self { Self { base, ... } }
}

#[godot_api]
impl StageCollector {
    #[func]
    pub fn get_tracked_count(&self) -> u32 { ... }
}
```

### Example 3: Library entry point — ExtensionLibrary registration
**File**: `crates/stage-godot/src/lib.rs:16-19`
```rust
struct StageExtension;

#[gdextension]
unsafe impl ExtensionLibrary for StageExtension {}
```

## When to Use
- Any new Godot-facing Rust class: follow the three-part structure (struct + INode impl + exported impl)
- Godot signals: use `#[signal]` inside `#[godot_api] impl` — NOT in the INode impl
- Cross-class references: store as `Option<Gd<T>>`, set via a `#[func]` setter from GDScript

## When NOT to Use
- Classes that don't need Godot lifecycle — plain Rust structs are fine for internal logic
- EditorPlugin as a GDExtension base — GDScript owns the editor-plugin lifecycle in Theatre; GDExtension classes are plain `Node` subclasses behind GDScript glue

## Common Violations
- Storing `Gd<T>` across thread boundaries — not safe; all Godot object access must stay on the main thread
- Using `base` field for logic — `base` is only for Godot engine calls (e.g., `self.base().emit_signal(...)`)
- Forgetting `pub` on `#[func]` methods — registration does not strictly require `pub`, but this codebase keeps them `pub` by convention
