# Pattern: Inline Test Module with Builder Fixtures

Tests live in `#[cfg(test)] mod tests` blocks alongside source code. Reusable test data is constructed by small builder functions (not structs or frameworks) that take just the fields that vary per test.

## Rationale
Keeps test logic co-located with the code under test. Builder fixture functions minimize boilerplate without introducing a test framework dependency. All 17 test files in the codebase follow this pattern; 71 total test functions.

## Examples

### Example 1: Index tests — entity builder with optional group
**File**: `crates/spectator-core/src/index.rs:143-160`
```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn entity(path: &str, pos: [f64; 3]) -> IndexedEntity {
        IndexedEntity { path: path.into(), class: "Node3D".into(), position: pos, groups: vec![] }
    }

    fn entity_with_group(path: &str, pos: [f64; 3], group: &str) -> IndexedEntity {
        IndexedEntity { path: path.into(), class: "Node3D".into(), position: pos, groups: vec![group.into()] }
    }

    #[test]
    fn nearest_returns_k_closest() {
        let index = SpatialIndex::build(vec![
            entity("a", [0.0, 0.0, 0.0]),
            entity("b", [1.0, 0.0, 0.0]),
            entity("c", [10.0, 0.0, 0.0]),
        ]);
        let results = index.nearest([0.0, 0.0, 0.0], 2, &[], &[]);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, "a");
    }
}
```

### Example 2: Delta tests — entity with state map
**File**: `crates/spectator-core/src/delta.rs:307-320`
```rust
fn entity(path: &str, pos: Position3, state: &[(&str, serde_json::Value)]) -> EntitySnapshot {
    EntitySnapshot {
        path: path.into(),
        class: "CharacterBody3D".into(),
        position: pos,
        rotation_deg: [0.0; 3],
        velocity: [0.0; 3],
        groups: vec![],
        state: state.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
        visible: true,
    }
}
```

### Example 3: Cluster tests — two separate builders for entity and relative position
**File**: `crates/spectator-core/src/cluster.rs:338-360`
```rust
fn make_entity(path: &str, groups: &[&str], is_static: bool) -> RawEntityData {
    RawEntityData {
        path: path.into(),
        groups: groups.iter().map(|s| s.to_string()).collect(),
        is_static,
        // ...other fields with defaults
    }
}

fn make_rel(dist: f64) -> RelativePosition {
    RelativePosition { dist, bearing: Cardinal::Ahead, bearing_deg: 0.0, elevation: Elevation::Level, occluded: false }
}
```

### Example 4: Config tests with TOML file I/O (tempfile)
**File**: `crates/spectator-server/src/config.rs:91-130`
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn load_valid_toml() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("spectator.toml");
        let mut f = File::create(&path).unwrap();
        writeln!(f, "[connection]\nport = 9078").unwrap();
        writeln!(f, "[tracking]\ntoken_hard_cap = 2000").unwrap();

        let port = load_toml_port(dir.path());
        assert_eq!(port, Some(9078));
    }
}
```

## When to Use
- Any new module with testable pure logic: add `#[cfg(test)] mod tests` at the bottom of the file
- Repeated construction of test structs: extract a `fn fixture_name(varying_field: T) -> Struct` helper
- File I/O tests: use `tempfile::TempDir` for isolation

## When NOT to Use
- Integration tests spanning multiple crates — consider `tests/` directory at workspace level
- Tests requiring Godot runtime (GDExtension) — those are tested via GDScript in the editor

## Common Violations
- Constructing full structs inline in every test — extract a builder function when the same struct appears 3+ times
- Using `unwrap()` on fallible operations that could mask test setup bugs — acceptable in tests, but log what failed
- Putting tests in a separate `tests/` directory when the tests are unit tests — keep them inline
