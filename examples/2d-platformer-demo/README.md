# 2D Platformer Demo

Minimal Godot 4 project for testing Spectator with 2D scenes.

## Setup

1. Symlink or copy `addons/spectator/` from the repo root into this project
2. Build the GDExtension: `cargo build -p spectator-godot`
3. Copy the binary: `cp target/debug/libspectator_godot.so addons/spectator/bin/linux/`
4. Open in Godot, enable the Spectator plugin, press Play

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
