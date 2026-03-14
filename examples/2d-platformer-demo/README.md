# 2D Platformer Demo

Minimal Godot 4 project for testing Stage with 2D scenes.

## Setup

1. Symlink or copy `addons/stage/` from the repo root into this project
2. Build the GDExtension: `cargo build -p stage-godot`
3. Copy the binary: `cp target/debug/libstage_godot.so addons/stage/bin/linux/`
4. Open in Godot, enable the Stage plugin, press Play

## Example Agent Queries

```
# 2D snapshot — positions are [x, y], bearings have no elevation
spatial_snapshot(detail: "standard")

# Find enemies by group
spatial_snapshot(groups: ["enemies"])

# 2D raycast — uses PhysicsServer2D
spatial_query(query_type: "raycast", from: "Player", to: "Enemy1")

# Inspect a 2D CharacterBody2D
spatial_inspect(node: "Player", include: ["transform", "physics", "state"])
```
