<script setup>
const messages0 = [
  { role: 'human', text: `Build the intro platform section: floor, three step platforms at increasing heights, and a point light.` },
  { role: 'agent', text: `Building in one batch to minimize round-trips. Sending 4 operations at once.` },
  { role: 'tool', text: `director batch_execute — 4 ops: tilemap_fill (floor 20 tiles), tilemap_set (6 platform tiles), node_add (OmniLight3D), node_set_property (light_energy, light_color, omni_range)` },
  { role: 'agent', text: `4 operations completed in 1 round-trip: floor (20 tiles), 3 platforms (6 tiles), OmniLight3D added and configured. Total: 26 tiles placed.` },
]
</script>

# Batch Operations

Execute multiple Director operations in a single round-trip.

## The cost of individual operations

Every Director operation requires a round-trip through the MCP protocol to Godot (and back). Depending on the backend, each round-trip takes:

- Editor backend: ~10-50ms
- Headless daemon: ~50-200ms
- One-shot: ~500-2000ms (Godot startup overhead)

Building a level with 21 individual `tilemap_set` calls takes 21 round-trips. With the daemon backend, that is 1-4 seconds of pure latency. With one-shot, it could take over 40 seconds.

`batch_execute` collapses all of that into one round-trip.

## `batch_execute`

Run a list of operations atomically in sequence.

```json
{
  "op": "batch_execute",
  "project_path": "/home/user/my-game",
  "operations": [
    {
      "op": "scene_create",
      "path": "scenes/room_b.tscn",
      "root_class": "Node3D",
      "root_name": "RoomB"
    },
    {
      "op": "node_add",
      "scene": "scenes/room_b.tscn",
      "parent": "RoomB",
      "name": "Floor",
      "class": "StaticBody3D"
    },
    {
      "op": "node_add",
      "scene": "scenes/room_b.tscn",
      "parent": "RoomB/Floor",
      "name": "CollisionShape3D",
      "class": "CollisionShape3D"
    },
    {
      "op": "node_add",
      "scene": "scenes/room_b.tscn",
      "parent": "RoomB/Floor",
      "name": "MeshInstance3D",
      "class": "MeshInstance3D"
    },
    {
      "op": "node_set_property",
      "scene": "scenes/room_b.tscn",
      "node": "RoomB/Floor",
      "properties": {
        "collision_layer": 4,
        "collision_mask": 0
      }
    }
  ]
}
```

### Parameters

| Parameter | Type | Description |
|---|---|---|
| `project_path` | `string` | Applied to all operations (no need to repeat in each) |
| `operations` | `array` | List of operation objects |
| `stop_on_error` | `boolean` | Default `true`. If `false`, continue on errors. |

Note: Each operation in the `operations` array does **not** need `project_path` — it is inherited from the batch wrapper.

### Response

```json
{
  "op": "batch_execute",
  "total": 5,
  "succeeded": 5,
  "failed": 0,
  "results": [
    { "op": "scene_create", "path": "scenes/room_b.tscn", "result": "ok" },
    { "op": "node_add", "name": "Floor", "result": "ok" },
    { "op": "node_add", "name": "CollisionShape3D", "result": "ok" },
    { "op": "node_add", "name": "MeshInstance3D", "result": "ok" },
    { "op": "node_set_property", "node": "RoomB/Floor", "result": "ok" }
  ]
}
```

If an error occurs with `stop_on_error: true` (default), the batch stops at the failing operation and the rest are not executed:

```json
{
  "op": "batch_execute",
  "total": 5,
  "succeeded": 2,
  "failed": 1,
  "error_at": 2,
  "results": [
    { "op": "scene_create", "result": "ok" },
    { "op": "node_add", "name": "Floor", "result": "ok" },
    { "op": "node_add", "name": "CollisionShape3D", "result": "error", "error": "Parent node 'RoomB/Floor' not found" }
  ]
}
```

## Example: Building a platform level

This example builds a platform section with a floor, three raised platforms, collision shapes, and lighting — in one batch.

<AgentConversation :messages="messages0" />

## When to use batch

**Always batch multi-step construction.** If you are:
- Building a level (many tiles)
- Creating a scene from scratch (create + add several nodes + set properties)
- Configuring multiple enemies (same properties on N nodes)
- Making related changes across multiple scenes

...use `batch_execute`. It is faster by a factor equal to the number of operations, and it presents the AI agent with a single success/failure response to reason about.

**For single operations, batching is not necessary.** The overhead of wrapping one operation in a batch is negligible but adds syntactic noise.

## Partial failure handling

With `stop_on_error: false`, the batch continues even if individual operations fail:

```json
{
  "op": "batch_execute",
  "project_path": "/home/user/my-game",
  "stop_on_error": false,
  "operations": [...]
}
```

Use this when operations are independent and you want to apply as many as possible (e.g., setting properties on 20 nodes where 1-2 might not exist).

With `stop_on_error: true` (default), the batch is transactional — a failure stops execution. Use this for ordered operations where later steps depend on earlier ones (e.g., create scene → add nodes → set properties).

## Tips

**Operations share `project_path`.** You do not need to repeat `"project_path"` in each operation — the batch wrapper applies it.

**Check `error_at` on failure.** The response tells you exactly which operation in the array failed, making it easy to diagnose and retry.

**Batches are not rolled back on failure.** If operation 8 fails, operations 1-7 are already applied. There is no automatic rollback. If you need atomicity, use git to snapshot the project before a large batch.
