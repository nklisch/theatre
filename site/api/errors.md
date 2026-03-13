# Error Reference

Error codes and messages returned by Theatre's MCP tools.

## MCP error structure

When a tool call fails, the MCP response contains an `isError: true` flag and the error content in the response body. Theatre errors have this structure:

```json
{
  "content": [
    {
      "type": "text",
      "text": "{ \"error\": \"Node not found\", \"node\": \"NonExistent\", \"hint\": \"Use scene_tree to list available nodes\" }"
    }
  ],
  "isError": true
}
```

The text content is a JSON object with at minimum an `"error"` field, and often a `"hint"` field with a suggestion.

## Spectator errors

### Connection errors

| Error | Cause | Resolution |
|---|---|---|
| `Game not running` | Port 9077 is not reachable | Start the game (F5) and ensure the Spectator addon is enabled |
| `Connection refused` | Nothing listening on 9077 | Game is not running, or addon failed to load |
| `Connection timed out` | Game is running but not responding | Check for script errors in the Godot output panel |
| `Handshake version mismatch` | Addon and server version mismatch | Rebuild and redeploy with `theatre-deploy` |

### Node errors

| Error | Cause | Resolution |
|---|---|---|
| `Node not found: "X"` | No tracked node named X | Use `scene_tree` to see available node names; check spelling |
| `Node not tracked: "X"` | Node exists but its type is not in tracked_types | Add the class to `extra_tracked_types` via `spatial_config` |
| `Ambiguous node name: "X"` | Multiple nodes share the same name | Use the full scene path (e.g., `"Level/Enemy_0"` not `"Enemy_0"`) |

### Frame errors

| Error | Cause | Resolution |
|---|---|---|
| `Frame out of buffer range` | `since_frame` is older than the ring buffer | Use a more recent frame, or start a recording for longer history |
| `No data collected yet` | Snapshot called before any physics tick | Wait a moment and retry; the collector needs at least 1 frame |

### Recording errors

| Error | Cause | Resolution |
|---|---|---|
| `Clip not found: "X"` | clip_id does not exist | Use `clips { "action": "list" }` to see available clips |
| `No active recording` | `stop` or `mark` called when not recording | Start a recording first with `action: "start"` |
| `Frame out of clip range` | `frame` is beyond the clip's frame count | Check `frame_count` in the clip list before querying |
| `Write error: disk full` | No space for clip file | Free disk space; change `record_path` via `spatial_config` |

### Query errors

| Error | Cause | Resolution |
|---|---|---|
| `Invalid query type: "X"` | Unknown `type` field | Use one of: `nearest`, `radius`, `area`, `raycast`, `path_distance`, `relationship` |
| `Navigation not available` | No NavigationRegion3D baked | Bake navmesh in the Godot editor |
| `Raycast origin out of range` | Origin node outside capture radius | Increase `capture_radius` via `spatial_config` |

### Action errors

| Error | Cause | Resolution |
|---|---|---|
| `Property not found: "X"` | Property does not exist on this node class | Use `spatial_inspect` to see available properties |
| `Type error: expected X, got Y` | Wrong value type for the property | Match the Godot property type (see [Wire Format](/api/wire-format) type mapping) |
| `Method not found: "X"` | Method does not exist or is private | Check the script for the exact method name |

## Director errors

### Backend errors

| Error | Cause | Resolution |
|---|---|---|
| `No backend available` | Editor closed, no daemon, godot not on PATH | Start editor or daemon; or add `godot` to PATH for one-shot |
| `Backend connection refused` | Port 6550/6551 not listening | Enable Director addon or start daemon |
| `Godot process failed` | One-shot Godot process exited with error | Check stderr output; project path may be wrong |
| `Project not found: "X"` | `project_path` does not contain `project.godot` | Use the absolute path to the project root directory |

### Scene errors

| Error | Cause | Resolution |
|---|---|---|
| `Scene not found: "X"` | Scene path does not exist | Use `scene_list` to find available scenes |
| `Node not found: "X"` | Node path does not exist in scene | Use `scene_read` to see the scene structure |
| `Parent not found: "X"` | Parent path does not exist | Create parent nodes first |
| `Duplicate node name: "X"` | A node with this name already exists under the parent | Use a different name or use `node_reparent` with `new_name` first |

### Property errors

| Error | Cause | Resolution |
|---|---|---|
| `Property not found: "X"` | Property does not exist on this Godot class | Check Godot documentation for valid property names |
| `Invalid property value` | Value type does not match property type | See type mapping in [Wire Format](/api/wire-format) |
| `Read-only property: "X"` | Property cannot be set (computed/const) | Some Godot properties are read-only at editor time |

### Resource errors

| Error | Cause | Resolution |
|---|---|---|
| `Resource type not found: "X"` | Unknown Godot resource class | Check spelling; use full class name (`StandardMaterial3D` not `Material`) |
| `Resource not found: "X"` | .tres/.res file does not exist | Use `resource_list` to find available resources |
| `Save failed: "X"` | Could not write resource file | Check directory exists and is writable |

### TileMap errors

| Error | Cause | Resolution |
|---|---|---|
| `TileMap has no TileSet` | TileMap node has no TileSet assigned | Assign a TileSet to the TileMap in the editor first |
| `Invalid source_id: N` | TileSet does not have this source | Check source IDs in the TileSet resource |
| `Invalid atlas_coords` | Coordinates out of range for the atlas | Check atlas dimensions in the TileSet |

### Animation errors

| Error | Cause | Resolution |
|---|---|---|
| `Animation not found: "X"` | AnimationPlayer does not have this animation | Use `scene_read` to see the AnimationPlayer's animations list |
| `Track index out of range: N` | Track index from `animation_add_track` is wrong | Use the exact `track_index` from the add_track response |
| `Invalid track path: "X"` | NodePath:property format is wrong | Ensure node path is relative to AnimationPlayer root |

## Common mistakes

### Wrong `project_path`

```
Error: Project not found at "/home/user/game"
```

`project_path` must point to the directory containing `project.godot`, not a subdirectory or parent.

```
Wrong: "/home/user/game/scenes"
Right: "/home/user/game"
```

### Case-sensitive node names

Godot node names are case-sensitive. `"player"` and `"Player"` are different nodes.

```
Error: Node not found: "player"
```

Use `scene_tree` or `spatial_snapshot` to see exact names.

### Using `scene_tree` paths in Director

The scene path in Director is relative to the scene's root node, not from the game's root. If `scene_read` shows the root as `"Player"`, use `"Player/CollisionShape3D"` (not `"/root/Player/CollisionShape3D"`).

### Forgetting to start the game before using Spectator

Spectator requires the game to be running. Director does not.

```
Error: Game not running
```

Press F5 in the Godot editor to start the game, then retry.
