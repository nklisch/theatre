---
description: "Batch operations — execute multiple Director commands in a single MCP call for efficient scene construction."
---

<script setup>
import { data } from '../.vitepress/data/tools.data'

const batch = data.params['batch'] ?? []

const messages0 = [
  { role: 'human', text: `Build the intro platform section: floor, three step platforms at increasing heights, and a point light.` },
  { role: 'agent', text: `Building in one batch to minimize round-trips. Sending 4 operations at once.` },
  { role: 'tool', text: `director batch — 4 ops: tilemap_set_cells (floor 20 tiles), tilemap_set_cells (6 platform tiles), node_add (OmniLight3D), node_set_properties (light_energy, light_color, omni_range)` },
  { role: 'agent', text: `4 operations completed in 1 round-trip: floor (20 tiles), 3 platforms (6 tiles), OmniLight3D added and configured. Total: 26 tiles placed.` },
]
</script>

# Batch Operations

Execute multiple Director operations in a single round-trip.

## Why group operations

Every Director call crosses the agent and Godot boundary. A batch groups related
operations into one request and one selected backend. This is especially useful
for a one-shot backend because it avoids a separate Godot launch per operation.
Choose a batch for sequentially related work, not for an atomicity guarantee.

## `batch`

Run a list of operations in sequence through the same selected context.

```json
{
  "op": "batch",
  "project_path": "/home/user/my-game",
  "operations": [
    {
      "operation": "scene_create",
      "params": {
        "scene_path": "scenes/room_b.tscn",
        "root_type": "Node3D"
      }
    },
    {
      "operation": "node_add",
      "params": {
        "scene_path": "scenes/room_b.tscn",
        "parent_path": ".",
        "node_type": "StaticBody3D",
        "node_name": "Floor"
      }
    }
  ]
}
```

### Parameters

<ParamTable :params="batch" />

Note: Each operation in the `operations` array does **not** need `project_path` — it is inherited from the batch wrapper.

### Response

```json
{
  "persistence": {
    "saved_paths": ["scenes/room_b.tscn"],
    "unsaved_scene_paths": []
  },
  "results": [
    {
      "operation": "scene_create",
      "success": true,
      "data": {
        "path": "scenes/room_b.tscn",
        "root_type": "Node3D",
        "persistence": {
          "saved_paths": ["scenes/room_b.tscn"],
          "unsaved_scene_paths": []
        }
      },
      "persistence": {
        "saved_paths": ["scenes/room_b.tscn"],
        "unsaved_scene_paths": []
      }
    },
    {
      "operation": "node_add",
      "success": true,
      "data": {
        "node_path": "Floor",
        "type": "StaticBody3D",
        "persistence": {
          "saved_paths": ["scenes/room_b.tscn"],
          "unsaved_scene_paths": []
        }
      },
      "persistence": {
        "saved_paths": ["scenes/room_b.tscn"],
        "unsaved_scene_paths": []
      }
    }
  ],
  "completed": 2,
  "failed": 0
}
```

Results stay in request order. With `stop_on_error: true` (default), execution
stops after the first entry whose `success` is false; later entries are not run.

## Example: Building a platform level

This example builds a platform section with a floor, three raised platforms, collision shapes, and lighting — in one batch.

<AgentConversation :messages="messages0" />

## When to use batch

Use `batch` when later operations should observe earlier results or when grouping
related work avoids repeated backend startup. Keep independent or failure-sensitive
changes separate when their individual persistence is easier to inspect that way.
A single operation does not need a batch wrapper.

## Partial failure handling

With `stop_on_error: false`, the batch continues even if individual operations fail:

```json
{
  "op": "batch",
  "project_path": "/home/user/my-game",
  "stop_on_error": false,
  "operations": [...]
}
```

Use this when operations are independent and you want to apply as many as possible (e.g., setting properties on 20 nodes where 1-2 might not exist).

With `stop_on_error: true` (default), the batch stops after the failing entry.
Earlier successful operations and partial effects remain. Use this for ordered
operations where later steps depend on earlier ones.

## Tips

**Operations share `project_path`.** You do not need to repeat `"project_path"` in each operation — the batch wrapper applies it.

**Inspect ordered results on failure.** Find the first entry with
`"success": false`, then read its `error`, `context`, and `persistence`. Also
inspect the batch-level persistence summary before retrying because the failed
entry itself may have partial effects.

**Batches are not rolled back on failure.** Earlier successful entries remain,
and a failing entry can also report partial effects. Read each entry's result and
persistence data before deciding what to retry. Open-scene mutations create one
native undo entry per changed batch entry and remain unsaved until `scene_save`.
