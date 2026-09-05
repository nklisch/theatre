# Pattern: GDExtension Class Export

Rust classes exported to Godot use `#[derive(GodotClass)]` + `#[class(base = Node)]`, implement the `INode` lifecycle trait via `#[godot_api] impl INode`, and expose methods/signals in a separate `#[godot_api] impl` block with `#[func]` and `#[signal]` decorators.

## Rationale
GDExtension registers classes at library load time through gdext's macro system. The split between `INode` (lifecycle) and the main `impl` (exported API) keeps Godot lifecycle concerns separate from application logic. `Gd<T>` is the only safe way to hold references to Godot objects.

## Examples

### Example 1: StageTCPServer — full class with signal and multiple funcs
**File**: `crates/stage-godot/src/tcp_server.rs:10-65`
```rust
#[derive(GodotClass)]
#[class(base = Node)]
pub struct StageTCPServer {
    base: Base<Node>,
    listener: Option<TcpListener>,
    clients: Vec<Option<ClientSlot>>,
    port: i32,
    conn_state: ConnectionState,
    pending_advance: Option<PendingAdvance>,
    collector: Option<Gd<StageCollector>>,
    recorder: Option<Gd<StageRecorder>>,
    client_idle_timeout_secs: u64,
}

#[godot_api]
impl INode for StageTCPServer {
    fn init(base: Base<Node>) -> Self {
        Self { base, listener: None, clients: Vec::new(), port: 9077, conn_state: ConnectionState::Idle, pending_advance: None, collector: None, recorder: None, client_idle_timeout_secs: 30, ... }
    }
}

#[godot_api]
impl StageTCPServer {
    #[signal]
    fn activity_received(entry_type: GString, summary: GString, tool_name: GString);

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
**File**: `crates/stage-godot/src/lib.rs:8-11`
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
- EditorPlugin as a GDExtension base — use GDScript for EditorPlugin (godot#85268 limitation)

## Common Violations
- Storing `Gd<T>` across thread boundaries — not safe; all Godot object access must stay on the main thread
- Using `base` field for logic — `base` is only for Godot engine calls (e.g., `self.base().emit_signal(...)`)
- Forgetting `pub` on `#[func]` methods — they must be `pub` to be visible from GDScript
