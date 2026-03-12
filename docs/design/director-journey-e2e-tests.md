# Design: Director Journey E2E Tests

## Overview

Comprehensive journey E2E tests for the Director MCP API, validating multi-step
agent workflows against real Godot instances. These tests simulate the operations
an AI agent would perform to fulfill real human tasks — building levels, wiring
UI, animating characters, managing resources — and verify that actual mutations
occur as expected.

### Current State

The existing test suite has 23 test modules with ~120 tests covering individual
operations per domain (scene, node, animation, tilemap, etc.) plus 2 journey
tests covering basic scene composition. Coverage gaps exist for:

- Multi-domain workflows (e.g. creating a scene, adding materials, setting
  physics, wiring signals — all in one coherent task)
- Daemon-backed multi-operation journeys
- Batch operation pipelines for real tasks
- VisualShader → material → scene attachment pipeline
- Scene diff as a verification/review tool in agent workflows
- Complex property type round-trips (Color, NodePath, resource references)
- `resource_duplicate` with `deep_copy: true`
- `node_find` by property filter
- `style_box_create` for StyleBoxTexture/StyleBoxLine/StyleBoxEmpty variants

### Design Principles

- **Agent-realistic**: Each journey represents a task a human would ask an AI
  agent to do ("build me a platformer level", "set up a main menu")
- **Mutation-verified**: Every mutation is read back and asserted, not just
  checked for success
- **Cross-domain**: Journeys combine operations across multiple tool domains
- **Primarily oneshot**: Most tests use `DirectorFixture` (one-shot backend).
  A few daemon-specific journeys test multi-operation persistence.
- **Unique prefixed names**: Each journey uses `tmp/j_<journey>_*.tscn` naming

---

## Implementation Units

### Unit 1: Harness Helper — `scene_has_node`

**File**: `tests/director-tests/src/harness.rs`

```rust
impl DirectorFixture {
    /// Read a scene and find a node by path, returning its JSON object.
    /// Panics with a clear message if the node is not found.
    pub fn read_node(
        &self,
        scene_path: &str,
        node_path: &str,
    ) -> serde_json::Value;

    /// Temp resource path for journey tests.
    pub fn temp_resource_path(name: &str) -> String {
        format!("tmp/j_{name}.tres")
    }

    /// Temp scene path for journey tests (distinct prefix from unit tests).
    pub fn journey_scene_path(name: &str) -> String {
        format!("tmp/j_{name}.tscn")
    }
}
```

**Implementation Notes**:
- `read_node` calls `scene_read` then walks `root` → `children` recursively,
  splitting `node_path` on `/` to navigate. Returns the matching JSON node
  object.
- Used by journey tests to assert node properties without manually navigating
  the JSON tree in every test.

**Acceptance Criteria**:
- [ ] `read_node("scene.tscn", "Player/Sprite")` returns the Sprite node JSON
- [ ] `read_node("scene.tscn", ".")` returns the root node
- [ ] `read_node("scene.tscn", "Nonexistent")` panics with descriptive message

---

### Unit 2: Journey — Build 2D Platformer Level

**File**: `tests/director-tests/src/test_journey_platformer.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_build_2d_platformer_level()
```

**Workflow** (simulates: "Build me a 2D platformer level with a player, ground
tiles, enemies, and collectibles"):

1. `scene_create` — Node2D root "Level"
2. `node_add` — CharacterBody2D "Player" at position (64, 192)
3. `node_add` — CollisionShape2D "Hitbox" under Player
4. `shape_create` — CapsuleShape2D attached to Player/Hitbox
5. `node_add` — Sprite2D "PlayerSprite" under Player
6. `node_set_properties` — Set Player position to (64, 192)
7. `node_add` — TileMapLayer "Ground" with tileset
8. `tilemap_set_cells` — Place ground tiles across bottom row (y=0, x=0..15)
9. `tilemap_set_cells` — Place platform tiles at elevated positions
10. `node_add` — Node2D "Enemies" container
11. `node_add` — CharacterBody2D "Goomba" under Enemies
12. `node_set_properties` — Position Goomba at (320, 192)
13. `node_set_groups` — Add Player to "player" group, Goomba to "enemies" group
14. `node_add` — Area2D "Coin" with CollisionShape2D under it
15. `node_set_groups` — Add Coin to "collectibles" group
16. `scene_read` — Read full tree, verify hierarchy and positions
17. `node_find` — Find all "enemies" group members → [Goomba]
18. `node_find` — Find all "collectibles" group members → [Coin]
19. `scene_list` — Verify scene appears in listing

**Acceptance Criteria**:
- [ ] Scene tree has correct hierarchy: Level > {Player > {Hitbox, PlayerSprite}, Ground, Enemies > Goomba, Coin > CollisionShape2D}
- [ ] Player position reads back as (64, 192)
- [ ] Ground TileMapLayer has >= 16 cells set
- [ ] node_find by group returns expected results
- [ ] Shape is attached to Player/Hitbox

---

### Unit 3: Journey — Build 3D Scene with Materials and Physics

**File**: `tests/director-tests/src/test_journey_3d_scene.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_build_3d_scene_with_materials_and_physics()
```

**Workflow** (simulates: "Create a 3D scene with a floor, player with collision,
and a red metallic material"):

1. `scene_create` — Node3D root "World"
2. `material_create` — StandardMaterial3D "floor_mat" with albedo gray, roughness 0.9
3. `material_create` — StandardMaterial3D "player_mat" with albedo red, metallic 0.8
4. `node_add` — StaticBody3D "Floor"
5. `node_add` — CollisionShape3D "FloorCol" under Floor
6. `shape_create` — BoxShape3D (20x0.2x20) attached to Floor/FloorCol
7. `node_add` — MeshInstance3D "FloorMesh" under Floor
8. `node_set_properties` — Set FloorMesh material_override to floor_mat path
9. `node_add` — CharacterBody3D "Player"
10. `node_add` — CollisionShape3D "PlayerCol" under Player
11. `shape_create` — CapsuleShape3D (radius=0.4, height=1.8) attached to Player/PlayerCol
12. `node_set_properties` — Set Player position to (0, 1, 0)
13. `physics_set_layers` — Player collision_layer=1, collision_mask=3
14. `physics_set_layers` — Floor collision_layer=2
15. `resource_read` — Read back player_mat, verify metallic=0.8
16. `scene_read` — Verify full hierarchy

**Acceptance Criteria**:
- [ ] Both materials created with correct properties (verified via resource_read)
- [ ] Floor shape is BoxShape3D (verified via scene_read showing CollisionShape3D)
- [ ] Player collision_layer=1, collision_mask=3
- [ ] Player position reads back as (0, 1, 0)

---

### Unit 4: Journey — Animate a Character

**File**: `tests/director-tests/src/test_journey_animation.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_create_character_animations()
```

**Workflow** (simulates: "Create walk and idle animations for a character with
position movement, sprite color flash, and method call tracks"):

1. `animation_create` — "idle" animation, length=2.0, loop_mode=linear
2. `animation_add_track` — value track on "Sprite2D:modulate" (color pulse)
3. `animation_add_track` — bezier track on "Sprite2D:modulate:a" (alpha breathe)
4. `animation_read` — Verify idle has 2 tracks, correct keyframes
5. `animation_create` — "walk" animation, length=0.8, loop_mode=linear
6. `animation_add_track` — position_3d track on "." (bobbing motion)
7. `animation_add_track` — method track on "." (play_footstep at 0.0 and 0.4)
8. `animation_add_track` — value track on "Sprite2D:frame" (discrete sprite frames)
9. `animation_read` — Verify walk has 3 tracks, correct types and keyframe counts
10. `animation_create` — "attack" animation, length=0.5, loop_mode=none
11. `animation_add_track` — rotation_3d track on "WeaponPivot"
12. `animation_add_track` — scale_3d track on "WeaponPivot/Weapon"
13. `animation_remove_track` — Remove scale_3d track by node_path
14. `animation_read` — Verify attack has 1 track remaining (rotation only)

**Acceptance Criteria**:
- [ ] "idle" has 2 tracks (value + bezier), loop_mode=linear, length=2.0
- [ ] "walk" has 3 tracks (position_3d + method + value), method keyframes have correct method names
- [ ] "attack" starts with 2 tracks, ends with 1 after removal
- [ ] All keyframe values round-trip correctly (position vectors, color strings, bezier handles)
- [ ] Discrete update_mode preserved on sprite frame track

---

### Unit 5: Journey — Wire a UI Menu

**File**: `tests/director-tests/src/test_journey_ui.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_wire_main_menu()
```

**Workflow** (simulates: "Create a main menu with Play, Options, and Quit
buttons, wire them to a handler script"):

1. `scene_create` — Control root "MainMenu"
2. `node_add` — VBoxContainer "ButtonContainer"
3. `node_add` — Button "PlayButton" under ButtonContainer, properties: text="Play"
4. `node_add` — Button "OptionsButton" under ButtonContainer, properties: text="Options"
5. `node_add` — Button "QuitButton" under ButtonContainer, properties: text="Quit"
6. `node_add` — Node "MenuLogic" under root (signal handler)
7. Create a minimal .gd script file on disk
8. `node_set_script` — Attach script to MenuLogic
9. `signal_connect` — PlayButton.pressed → MenuLogic.on_play_pressed
10. `signal_connect` — OptionsButton.pressed → MenuLogic.on_options_pressed
11. `signal_connect` — QuitButton.pressed → MenuLogic.on_quit_pressed
12. `signal_list` — Verify 3 connections total
13. `signal_list` — Filter by "PlayButton" → 1 connection
14. `node_set_meta` — Set meta on root: {"version": 1, "author": "agent"}
15. `scene_read` — Verify all buttons have text property, signal handler exists

**Acceptance Criteria**:
- [ ] Scene has Control > VBoxContainer > {PlayButton, OptionsButton, QuitButton}
- [ ] signal_list returns 3 connections with correct signal_name, source, target, method
- [ ] Filtered signal_list for PlayButton returns exactly 1
- [ ] MenuLogic has script attached (script_path contains the .gd file)
- [ ] Root node meta has "version" and "author" keys

---

### Unit 6: Journey — Scene Composition with Instances

**File**: `tests/director-tests/src/test_journey_composition.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_compose_game_from_reusable_scenes()
```

**Workflow** (simulates: "Create reusable enemy and item scenes, then compose a
level that instances them multiple times"):

1. `scene_create` — "enemy.tscn" with CharacterBody2D root
2. `node_add` — Sprite2D + CollisionShape2D under enemy root
3. `node_set_properties` — Set enemy root properties
4. `scene_create` — "health_pack.tscn" with Area2D root
5. `node_add` — CollisionShape2D under health_pack root
6. `scene_create` — "level.tscn" with Node2D root
7. `node_add` — Node2D "Enemies", Node2D "Items", Node2D "Environment"
8. `scene_add_instance` — Instance enemy.tscn as "Enemy1" under Enemies
9. `scene_add_instance` — Instance enemy.tscn as "Enemy2" under Enemies
10. `scene_add_instance` — Instance health_pack.tscn as "HealthPack1" under Items
11. `node_set_properties` — Set Enemy1 position to (100, 0)
12. `node_set_properties` — Set Enemy2 position to (300, 0)
13. `node_set_properties` — Set HealthPack1 position to (200, -50)
14. `scene_read` — Verify instances are present with correct names
15. `node_reparent` — Move Enemy1 from Enemies to Environment (simulating level editing)
16. `scene_read` — Verify Enemy1 is now under Environment
17. `scene_diff` — Diff level.tscn against itself (regression: no spurious changes)
18. `scene_list` — Verify all 3 scenes appear with correct root types

**Acceptance Criteria**:
- [ ] Level has 3 container nodes with instances placed correctly
- [ ] Instance positions read back correctly after node_set_properties
- [ ] Reparent moves Enemy1 from Enemies to Environment
- [ ] scene_diff of scene with itself shows no changes
- [ ] scene_list shows enemy (CharacterBody2D), health_pack (Area2D), level (Node2D)

---

### Unit 7: Journey — Resource Pipeline (Materials, Shapes, Duplication)

**File**: `tests/director-tests/src/test_journey_resources.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_material_pipeline_create_duplicate_apply()
```

**Workflow** (simulates: "Create a base material, duplicate it with variations,
and create collision shapes"):

1. `material_create` — StandardMaterial3D "base_mat" with metallic=0.5, roughness=0.7
2. `resource_read` — Verify base_mat properties
3. `resource_duplicate` — Duplicate base_mat → "red_variant" with albedo_color override
4. `resource_duplicate` — Duplicate base_mat → "shiny_variant" with metallic=1.0
5. `resource_read` — Verify red_variant has albedo override but same roughness
6. `resource_read` — Verify shiny_variant has metallic=1.0 but same roughness
7. `shape_create` — BoxShape3D saved to file
8. `shape_create` — CapsuleShape3D saved to file
9. `shape_create` — SphereShape3D attached to a scene node
10. `resource_read` — Verify BoxShape3D properties (size)
11. `style_box_create` — StyleBoxFlat with bg_color and corner radii
12. `resource_read` — Verify StyleBoxFlat properties

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_style_box_variants()
```

**Workflow** (fills coverage gap for StyleBox types):
1. `style_box_create` — StyleBoxFlat with all corner radii
2. `style_box_create` — StyleBoxEmpty (no properties needed)
3. `style_box_create` — StyleBoxLine with color and thickness
4. `resource_read` — Verify each type and properties

**Acceptance Criteria**:
- [ ] Base material properties verified via resource_read
- [ ] Duplicated materials have overrides applied, non-overridden props preserved
- [ ] All 3 shape types created and verified
- [ ] StyleBoxFlat, StyleBoxEmpty, StyleBoxLine all created successfully

---

### Unit 8: Journey — TileMap Level Design

**File**: `tests/director-tests/src/test_journey_tilemap.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_tilemap_level_design_workflow()
```

**Workflow** (simulates: "Design a tilemap level with ground, platforms, and a
hazard zone, then edit it"):

1. `scene_create` — Node2D "Level"
2. `node_add` — TileMapLayer "Ground" with tileset
3. `node_add` — TileMapLayer "Hazards" with tileset
4. `tilemap_set_cells` — Fill Ground with floor tiles (row of 20 cells)
5. `tilemap_set_cells` — Add platforms at scattered heights on Ground
6. `tilemap_set_cells` — Add hazard cells on Hazards layer (spikes, lava)
7. `tilemap_get_cells` — Read Ground cells, verify count = 20 + platforms
8. `tilemap_get_cells` — Read Ground with region filter for just platforms area
9. `tilemap_clear` — Clear a region from Ground (create a gap in the floor)
10. `tilemap_get_cells` — Verify gap exists (cell count reduced)
11. `tilemap_get_cells` — Read Hazards layer, verify independent from Ground
12. `scene_read` — Verify both TileMapLayer nodes exist in tree

**Acceptance Criteria**:
- [ ] Ground layer has correct cell count after initial fill
- [ ] Region filter returns only cells within bounds
- [ ] Clearing a region removes only targeted cells
- [ ] Hazards layer is independent (not affected by Ground operations)
- [ ] Both layers visible in scene_read

---

### Unit 9: Journey — GridMap 3D Level Building

**File**: `tests/director-tests/src/test_journey_gridmap.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_gridmap_3d_level_building()
```

**Workflow** (simulates: "Build a 3D dungeon room with floor, walls, and
oriented props"):

1. `scene_create` — Node3D "DungeonRoom"
2. `node_add` — GridMap "Floor" with mesh_library
3. `node_add` — GridMap "Walls" with mesh_library
4. `gridmap_set_cells` — Fill Floor with item 0 in a 5x5 grid (y=0)
5. `gridmap_set_cells` — Place wall cells around perimeter (item 1, various orientations)
6. `gridmap_get_cells` — Read Floor, verify 25 cells
7. `gridmap_get_cells` — Read Walls with bounds filter for one wall
8. `gridmap_clear` — Clear a doorway in the walls (region clear)
9. `gridmap_get_cells` — Verify wall count decreased by doorway cells
10. `scene_read` — Verify both GridMap nodes in tree

**Acceptance Criteria**:
- [ ] Floor has 25 cells (5x5 grid at y=0)
- [ ] Wall cells have correct orientations preserved
- [ ] Bounds filter returns only matching cells
- [ ] Doorway clear removes correct number of cells

---

### Unit 10: Journey — Batch Pipeline

**File**: `tests/director-tests/src/test_journey_batch.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_batch_create_complete_scene()
```

**Workflow** (simulates: "Use batch to efficiently create a scene with nodes,
properties, groups, and signals in a single Godot invocation"):

1. `scene_create` — Create the scene first (separate call, needed before batch)
2. `batch` — Single batch with:
   - `node_add` — CharacterBody2D "Player"
   - `node_add` — Sprite2D "PlayerSprite" under Player
   - `node_add` — CollisionShape2D "Hitbox" under Player
   - `node_set_properties` — Set Player position
   - `node_set_groups` — Add Player to "player" group
   - `node_add` — Area2D "DamageZone"
   - `node_add` — CollisionShape2D "DZShape" under DamageZone
3. Verify batch results: completed=7, failed=0
4. `scene_read` — Verify complete tree was built correctly
5. `node_find` — Find "player" group members → [Player]

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_batch_partial_failure_recovery()
```

**Workflow** (simulates: agent sends batch with one bad operation, uses
stop_on_error=false to get maximum progress):

1. `scene_create` — Create scene
2. `batch` with stop_on_error=false:
   - `node_add` — valid node A
   - `node_set_properties` — invalid property on A (should fail)
   - `node_add` — valid node B
   - `node_add` — valid node C
3. Verify: completed=3, failed=1
4. `scene_read` — Verify A, B, C all exist despite the mid-batch failure

**Acceptance Criteria**:
- [ ] Full batch creates all 7 operations in a single Godot invocation
- [ ] scene_read confirms complete tree matches expected hierarchy
- [ ] Partial failure batch: 3 successes + 1 failure, non-failing ops still applied
- [ ] node_find confirms group membership set during batch

---

### Unit 11: Journey — Scene Diff as Review Tool

**File**: `tests/director-tests/src/test_journey_diff.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_scene_diff_tracks_iterative_changes()
```

**Workflow** (simulates: agent makes changes then uses scene_diff to review what
changed, similar to `git diff`):

1. `scene_create` — "before.tscn" with Node2D + Sprite + Label
2. Create "after.tscn" as copy (scene_create + same nodes)
3. `node_add` — Add new "Particles" node to after
4. `node_remove` — Remove "Label" from after
5. `node_set_properties` — Change Sprite position in after
6. `scene_diff` — Compare before vs after
7. Verify: added=["Particles"], removed=["Label"], changed includes Sprite position

**Acceptance Criteria**:
- [ ] scene_diff correctly identifies the added node
- [ ] scene_diff correctly identifies the removed node
- [ ] scene_diff correctly identifies the property change on Sprite
- [ ] No false positives (unchanged nodes don't appear in diff)

---

### Unit 12: Journey — VisualShader Pipeline

**File**: `tests/director-tests/src/test_journey_shader.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_visual_shader_create_and_apply_to_scene()
```

**Workflow** (simulates: "Create a spatial shader with color and vertex
displacement, then apply it to a mesh"):

1. `visual_shader_create` — Spatial shader with:
   - VisualShaderNodeColorConstant (node_id=2, fragment, properties: constant=red)
   - VisualShaderNodeInput (node_id=3, vertex, properties: input_name="vertex")
   - Connection: node 2 → output node 0 port 0 (albedo) in fragment
2. `resource_read` — Verify the .tres was created with correct type
3. `material_create` — ShaderMaterial referencing the visual shader
4. `scene_create` — Node3D scene
5. `node_add` — MeshInstance3D
6. `node_set_properties` — Set material_override to the ShaderMaterial path
7. `scene_read` — Verify MeshInstance3D exists with material reference

**Acceptance Criteria**:
- [ ] VisualShader .tres created with 2 nodes and 1 connection
- [ ] resource_read confirms VisualShader resource type
- [ ] Material references the shader (if ShaderMaterial supports shader_path)
- [ ] Scene has MeshInstance3D with material_override set

---

### Unit 13: Journey — Daemon Multi-Operation Efficiency

**File**: `tests/director-tests/src/test_journey_daemon.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_daemon_multi_scene_workflow()
```

**Workflow** (simulates: agent uses daemon for multiple operations across
scenes without cold-start penalty):

1. Start DaemonFixture
2. `scene_create` — Scene A
3. `node_add` — Add nodes to Scene A
4. `node_set_properties` — Set properties on Scene A
5. `scene_create` — Scene B
6. `scene_add_instance` — Instance Scene A into Scene B
7. `scene_read` — Read Scene B, verify instance present
8. `scene_diff` — Diff A with B (structural comparison)
9. Verify all operations succeeded via daemon (no cold-start per operation)
10. `quit` — Clean daemon shutdown

**Acceptance Criteria**:
- [ ] All operations succeed through daemon backend
- [ ] Scene B contains instance of Scene A
- [ ] scene_diff works correctly through daemon
- [ ] Clean quit without errors

---

### Unit 14: Journey — Physics Layer Configuration

**File**: `tests/director-tests/src/test_journey_physics.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_physics_layer_setup()
```

**Workflow** (simulates: "Set up physics layers for a game with player, enemies,
environment, and projectiles on separate collision layers"):

1. `physics_set_layer_names` — Name 2d_physics layers: 1=player, 2=enemies, 3=environment, 4=projectiles
2. `scene_create` — Game scene
3. `node_add` — CharacterBody2D "Player"
4. `node_add` — CharacterBody2D "Enemy"
5. `node_add` — StaticBody2D "Wall"
6. `node_add` — Area2D "Bullet"
7. `physics_set_layers` — Player: layer=1, mask=2|3 (collides with enemies + environment)
8. `physics_set_layers` — Enemy: layer=2, mask=1|3|4 (collides with player + env + projectiles)
9. `physics_set_layers` — Wall: layer=3, mask=0 (static, no scanning)
10. `physics_set_layers` — Bullet: layer=4, mask=2 (hits enemies only)
11. `scene_read` — Verify all nodes exist

**Acceptance Criteria**:
- [ ] Layer names set to project.godot (layers_set=4)
- [ ] Player collision_layer=1, collision_mask=6 (binary 110 = layers 2+3)
- [ ] Enemy collision_layer=2, collision_mask=13 (binary 1101 = layers 1+3+4)
- [ ] Wall collision_layer=4 (layer 3 = bit 2 = value 4)
- [ ] Bullet collision_layer=8 (layer 4 = bit 3 = value 8), collision_mask=2

---

### Unit 15: Coverage Gaps — node_find by Property

**File**: `tests/director-tests/src/test_journey_search.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_node_find_complex_search()
```

**Workflow** (simulates: agent searching for specific nodes to modify):

1. `scene_create` — Scene with diverse node tree
2. Add nodes: multiple Sprite2D, CharacterBody2D, Area2D nodes at various depths
3. `node_set_groups` — Add some to "enemies", some to "items"
4. `node_set_properties` — Set visible=false on some nodes
5. `node_find` — class_name="Sprite2D" → all sprites
6. `node_find` — group="enemies" → only enemy nodes
7. `node_find` — name_pattern="Enemy*" → name-matched nodes
8. `node_find` — Combined: class_name="CharacterBody2D" AND group="enemies"
9. `node_find` — property filter (if supported): visible=false

**Acceptance Criteria**:
- [ ] Class filter returns correct count
- [ ] Group filter returns correct count
- [ ] Name pattern filter with wildcard works
- [ ] Combined filters return intersection (AND logic)

---

### Unit 16: Journey — UID and Project Utilities

**File**: `tests/director-tests/src/test_journey_project.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_uid_workflow()
```

**Workflow** (simulates: agent creating scenes and resolving UIDs):

1. `scene_create` — Create 3 scenes
2. `uid_get` — Resolve UID for each scene
3. Verify all UIDs are unique and start with "uid://"
4. `uid_update_project` — Scan tmp/ directory
5. Verify files_scanned > 0

**Acceptance Criteria**:
- [ ] Each scene gets a unique UID
- [ ] All UIDs match "uid://" format
- [ ] uid_update_project scans successfully

---

### Unit 17: Journey — Full Game Scene from Scratch

**File**: `tests/director-tests/src/test_journey_full_game.rs`

```rust
#[test]
#[ignore = "requires Godot binary"]
fn journey_full_game_scene_everything_together()
```

**Workflow** (simulates: "Create a complete small game scene using every major
tool domain"):

This is the capstone test combining ALL tool domains in a single coherent task:

1. **Project setup**: `physics_set_layer_names` for 2d_physics
2. **Player scene**: `scene_create` + `node_add` (CharacterBody2D + Sprite2D + CollisionShape2D)
3. **Shape**: `shape_create` CapsuleShape2D attached to player collision
4. **Properties**: `node_set_properties` position, `node_set_groups` "player"
5. **Script**: `node_set_script` attach a .gd file
6. **Meta**: `node_set_meta` set editor metadata
7. **Material**: `material_create` StandardMaterial3D (for 3D elements)
8. **Animation**: `animation_create` + `animation_add_track` walk animation
9. **Level scene**: `scene_create` + `node_add` structure
10. **TileMap**: `node_add` TileMapLayer + `tilemap_set_cells`
11. **Instance**: `scene_add_instance` player into level
12. **Physics**: `physics_set_layers` on player and environment
13. **Signals**: `signal_connect` on UI buttons (if present)
14. **Batch**: Use `batch` for bulk node additions to level
15. **Diff**: `scene_diff` level against itself (sanity check)
16. **Read**: `scene_read` full level tree
17. **List**: `scene_list` verify all created scenes
18. **Find**: `node_find` verify searchability
19. **UID**: `uid_get` resolve level scene UID

**Acceptance Criteria**:
- [ ] Every tool domain (scene, node, resource, tilemap, animation, physics, signal, shader, batch, diff, project) is exercised
- [ ] All mutations verified via read-back
- [ ] Final scene_read returns coherent tree with instances, properties, and groups
- [ ] scene_list returns all created scenes
- [ ] uid_get returns valid UID for the level

---

## Implementation Order

1. **Unit 1**: Harness helpers (dependency for all journey tests)
2. **Unit 2**: Platformer journey (exercises most common operations)
3. **Unit 5**: UI menu journey (signals + scripts + meta)
4. **Unit 6**: Composition journey (instancing + reparenting)
5. **Unit 3**: 3D scene journey (materials + physics)
6. **Unit 4**: Animation journey (multi-track, multi-animation)
7. **Unit 7**: Resource pipeline (duplication + shapes + styleboxes)
8. **Unit 8**: TileMap journey
9. **Unit 9**: GridMap journey
10. **Unit 10**: Batch pipeline
11. **Unit 11**: Scene diff journey
12. **Unit 12**: VisualShader pipeline
13. **Unit 13**: Daemon journey
14. **Unit 14**: Physics layer setup
15. **Unit 15**: Search/find coverage
16. **Unit 16**: UID/project utilities
17. **Unit 17**: Full game capstone (last — depends on understanding from all previous)

## Testing

### File Structure

```
tests/director-tests/src/
├── lib.rs                          # Add new mod declarations
├── harness.rs                      # Add read_node + helper methods
├── test_journey_platformer.rs      # Unit 2
├── test_journey_3d_scene.rs        # Unit 3
├── test_journey_animation.rs       # Unit 4
├── test_journey_ui.rs              # Unit 5
├── test_journey_composition.rs     # Unit 6
├── test_journey_resources.rs       # Unit 7
├── test_journey_tilemap.rs         # Unit 8
├── test_journey_gridmap.rs         # Unit 9
├── test_journey_batch.rs           # Unit 10
├── test_journey_diff.rs            # Unit 11
├── test_journey_shader.rs          # Unit 12
├── test_journey_daemon.rs          # Unit 13
├── test_journey_physics.rs         # Unit 14
├── test_journey_search.rs          # Unit 15
├── test_journey_project.rs         # Unit 16
└── test_journey_full_game.rs       # Unit 17
```

### Test Naming Convention

All journey tests use:
- `#[ignore = "requires Godot binary"]` marker
- `journey_` prefix in function name
- `tmp/j_<journey_name>_*.tscn` scene paths
- `tmp/j_<journey_name>_*.tres` resource paths

### Running

```bash
# All director tests (unit + journey)
cargo test -p director-tests -- --include-ignored

# Journey tests only
cargo test -p director-tests journey -- --include-ignored

# Single journey
cargo test -p director-tests journey_build_2d_platformer -- --include-ignored
```

## Verification Checklist

```bash
# 1. Build
cargo build -p director

# 2. Deploy GDExtension to test project
theatre-deploy ~/dev/theatre/tests/godot-project

# 3. Run all director tests
cargo test -p director-tests -- --include-ignored

# 4. Run only journey tests
cargo test -p director-tests journey -- --include-ignored

# 5. Verify no clippy warnings
cargo clippy -p director-tests

# 6. Verify formatting
cargo fmt -p director-tests --check
```
