---
name: gdext
description: Working with godot-rust/gdext (Rust GDExtension bindings for Godot) in the stage-godot crate. Use when writing or modifying Rust code that compiles to the GDExtension library.
---

# gdext — Rust GDExtension for Godot

This skill covers the `godot-rust/gdext` crate used in `crates/stage-godot`. That crate compiles to a `cdylib` loaded by Godot at runtime.

## Cargo.toml Setup

```toml
[lib]
crate-type = ["cdylib"]   # Required — produces .so/.dll/.dylib

[dependencies]
godot = { version = "0.4", features = ["api-4-5"] }
# api-4-5 = minimum Godot version we target (requires Godot 4.5+)
```

## Entry Point

Every gdext library needs exactly one entry point:

```rust
use godot::init::*;

struct StageExtension;

#[gdextension]
unsafe impl ExtensionLibrary for StageExtension {}
```

The `#[gdextension]` macro:
- Exports the symbol `gdext_rust_init` (must match `entry_symbol` in `.gdextension` file)
- Auto-registers all `#[derive(GodotClass)]` classes — no manual registration
- Is `unsafe` because it's FFI initialization

## Defining Classes

```rust
use godot::prelude::*;

#[derive(GodotClass)]
#[class(base=Node)]                    // Inherits from Node (scene tree accessible)
pub struct StageCollector {
    base: Base<Node>,                  // Required field — the superclass instance
    poll_interval: u32,                // Rust-private fields
}

#[godot_api]
impl INode for StageCollector {   // I{BaseName} = virtual method interface
    fn init(base: Base<Node>) -> Self {
        Self { base, poll_interval: 1 }
    }

    fn ready(&mut self) {             // _ready() equivalent
        // Called when added to scene tree
    }

    fn physics_process(&mut self, _delta: f64) {  // _physics_process()
        self.poll();
    }
}

#[godot_api]
impl StageCollector {             // Custom methods — second impl block
    #[func]                           // GDScript-callable
    pub fn get_visible_nodes(&self) -> Array<Dictionary> {
        // ...
    }

    #[func]
    pub fn poll(&mut self) {
        // Called by GDScript autoload or physics_process
    }
}
```

**Base class choice matters:**
- `Base<Node>` — scene tree access, has `_process`/`_physics_process`, no rendering
- `Base<Node3D>` / `Base<Node2D>` — has transform in world space
- `Base<RefCounted>` — no scene tree, lightweight data container
- `Base<Object>` — bare minimum, no scene tree

## Accessing Base/Self

```rust
// Immutable access to base class methods
self.base().get_tree()                 // Option<Gd<SceneTree>>
self.base().get_parent()               // Option<Gd<Node>>
self.base().get_name()                 // StringName

// Mutable access (needed for most Godot API calls that mutate)
self.base_mut().add_child(node.upcast())
self.base_mut().emit_signal("my_signal".into(), &[])
```

## Scene Tree Traversal

```rust
// Safe node access — returns Option, won't panic
let node = self.base().try_get_node_as::<CharacterBody3D>("enemies/scout_02");
if let Some(enemy) = node {
    let pos = enemy.get_global_position();
}

// Panics if not found or wrong type — only use when certain
let enemy = self.base().get_node_as::<CharacterBody3D>("enemies/scout_02");

// Get scene tree root
let tree = self.base().get_tree().expect("not in scene tree");
let root = tree.get_root().expect("no root");

// Iterate children
let parent = self.base().get_node_as::<Node>("enemies");
let count = parent.get_child_count();
for i in 0..count {
    let child = parent.get_child(i).expect("child exists");
    let name = child.get_name();
    let class = child.get_class();
}
```

## Reading Properties from Any Node

```rust
// Via Object::get — returns Variant, works for any property
let node = self.base().get_node_as::<Node>("some/node");
let health: Variant = node.upcast::<Object>().get("health".into());
let health_int: i64 = health.to::<i64>();

// Type-specific classes have typed methods
let body = self.base().get_node_as::<CharacterBody3D>("player");
let velocity = body.get_velocity();    // Vector3
let on_floor = body.is_on_floor();

// Get ALL properties (for exported vars scan)
let properties = node.upcast::<Object>().get_property_list();
```

## Properties and Exports

```rust
#[derive(GodotClass)]
#[class(base=Node)]
struct MyClass {
    #[export]           // Visible in Inspector + accessible from GDScript
    speed: f32,

    #[var]              // Accessible from GDScript only (no Inspector UI)
    internal_id: u32,

    base: Base<Node>,
    // Fields with no attribute are Rust-private
    cached_data: Vec<u8>,
}
```

`#[export]` implies `#[var]`. Use `#[export]` when the human needs to configure it in the editor, `#[var]` when GDScript needs it but not the inspector.

## Signals

```rust
#[godot_api]
impl MyClass {
    #[signal]
    fn data_collected(frame: i64, node_count: i32);  // Declaration only, no body
}

// Emit from Rust:
self.base_mut().emit_signal(
    "data_collected".into(),
    &[frame.to_variant(), count.to_variant()],
);
```

## Editor Classes — `#[class(tool)]`

For code that runs inside the editor (not just in-game):

```rust
#[derive(GodotClass)]
#[class(tool, init, base=EditorPlugin)]
struct MyEditorPlugin {
    base: Base<EditorPlugin>,
}

#[godot_api]
impl IEditorPlugin for MyEditorPlugin {
    fn enter_tree(&mut self) { /* plugin enabled */ }
    fn exit_tree(&mut self) { /* plugin disabled */ }
}
```

**CRITICAL GODOT LIMITATION:** GDScript cannot inherit from a GDExtension-derived EditorPlugin (godot#85268). In Stage, we solve this with the hybrid pattern:
- GDExtension provides `StageCollector`, `StageTCPServer`, `StageRecorder` as plain `Node` subclasses
- GDScript `plugin.gd` is the actual `EditorPlugin` and instantiates those Rust classes
- Do NOT make GDExtension classes inherit `EditorPlugin`

## StringName — Cache for Performance

StringName construction is expensive (FFI + interning). Cache at module level:

```rust
use godot::builtin::StringName;
use std::sync::OnceLock;

fn sn_health() -> &'static StringName {
    static NAME: OnceLock<StringName> = OnceLock::new();
    NAME.get_or_init(|| StringName::from("health"))
}

// Use the cached version
let val = node.get(sn_health().clone());
```

For property names used in tight loops (frame capture, bulk collection), always cache.

## Threading — Main Thread Only for Godot APIs

Godot's scene tree and object APIs are **not thread-safe**. `Gd<T>` is neither `Send` nor `Sync`.

**Rules:**
- All scene tree access in `_physics_process`, `_ready`, `_process` (main thread callbacks) — safe
- Do NOT pass `Gd<T>` to `std::thread::spawn`
- For background computation: collect raw data (positions, Strings, primitives) → send via channel → process in thread → send results back → apply on main thread
- TCP I/O: use non-blocking sockets polled from `_physics_process`, or Godot's `StreamPeerTCP`

```rust
// Safe pattern: collect on main thread, send primitives to background
fn physics_process(&mut self, _delta: f64) {
    // Collect data on main thread (scene tree access — safe)
    let snapshot = self.collect_snapshot();   // returns plain Rust structs, no Gd<T>

    // Send to background thread for heavy processing
    let _ = self.tx.try_send(snapshot);       // tx: mpsc::Sender

    // Also poll for TCP (non-blocking)
    self.tcp_server.poll();
}
```

## Calling Methods on Nodes

```rust
let node: Gd<Node> = self.base().get_node_as("some/node").upcast();

// Call any GDScript method
let result: Variant = node.call("take_damage".into(), &[50i32.to_variant()]);

// Or if you have a typed Gd<T> and the method is bound in gdext
let mut body = self.base().get_node_as::<CharacterBody3D>("player");
body.set_global_position(Vector3::new(5.0, 0.0, -3.0));
```

## Converting Between Types

```rust
// Upcast (always safe)
let node3d: Gd<Node3D> = ...;
let node: Gd<Node> = node3d.upcast();
let obj: Gd<Object> = node3d.upcast();

// Downcast (can fail)
let node: Gd<Node> = ...;
let body: Option<Gd<CharacterBody3D>> = node.try_cast::<CharacterBody3D>();

// Variant conversions
let v: Variant = 42i32.to_variant();
let i: i32 = v.to::<i32>();

// Vector3 to/from array
let pos: Vector3 = Vector3::new(1.0, 2.0, 3.0);
let arr = [pos.x, pos.y, pos.z];   // just destructure
```

## Common Gotchas

**`bind()` vs `bind_mut()`:** When you hold a `Gd<T>`, you can't directly call methods. Use `bind()` for `&self` access or `bind_mut()` for `&mut self`:
```rust
let collector: Gd<StageCollector> = ...;
let count = collector.bind().get_node_count();   // calls &self method
collector.bind_mut().poll();                      // calls &mut self method
```

**`init` vs manual `init`:** `#[class(init)]` generates a default `init()`. Without it, you must implement `fn init(base: Base<T>) -> Self` in the `I{Base}` trait impl.

**`get_node_as` panics on wrong type:** If the node exists but is a different class, it panics. Use `try_get_node_as` and handle `None`.

**Properties vs methods for Godot built-ins:** Godot's `CharacterBody3D.velocity` is a property, accessed via `get_velocity()` / `set_velocity()` in gdext, not as a field.

**`.gdextension` file must match entry symbol:**
```ini
[configuration]
entry_symbol = "gdext_rust_init"
compatibility_minimum = "4.5"
reloading = true           # enables hot-reload in Godot 4.5+

[libraries]
linux.debug.x86_64 = "bin/linux/libstage_godot.debug.so"
linux.release.x86_64 = "bin/linux/libstage_godot.so"
```
