<script setup>
import { data } from '../.vitepress/data/tools.data'
const params = data.params['spatial_config'] ?? []

const messages0 = [
  { role: 'human', text: `Snapshots are including too many irrelevant static background nodes. I only care about physics bodies.` },
  { role: 'agent', text: `I can mark those background nodes as static so Stage stops tracking them. What patterns do their node names follow?` },
  { role: 'human', text: `They all start with "Env_" or "Decor_".` },
  { role: 'agent', text: `Configuration updated. Nodes matching "Env_*" and "Decor_*" are now treated as static and excluded from snapshots. Take a new snapshot to confirm the reduction.` },
]
</script>

# spatial_config

Configure Stage's MCP behavior.

`spatial_config` adjusts how Stage formats and filters responses. Changes take effect immediately and persist for the duration of the server session. They are not saved between sessions.

## When to use it

- **Reducing noise**: mark known-static nodes so they are skipped in snapshots
- **Customizing output**: change bearing format, clustering strategy
- **Adding tracked properties**: register extra properties to capture per class
- **Capping token usage**: set a hard maximum on response size

For most use cases, the defaults work well. You only need `spatial_config` when the default settings are insufficient for your specific investigation.

## Parameters

<ParamTable :params="params" />

### `static_patterns`

Node name patterns (glob-style) for nodes that should be treated as static and excluded from spatial responses. Useful for environment nodes, decorations, and background props that never change:

```json
{
  "static_patterns": ["Env_*", "Decor_*", "Background*"]
}
```

Once set, these nodes are skipped when building snapshot and delta responses, keeping them focused on dynamic game objects.

### `state_properties`

Register additional Godot properties to capture per node class, beyond the defaults:

```json
{
  "state_properties": {
    "CharacterBody3D": ["health", "mana", "state_machine"],
    "Area3D": ["monitoring", "monitorable"]
  }
}
```

This is how you expose custom exported variables (like `health`) in snapshot responses.

### `cluster_by`

Controls how nodes are grouped in snapshot responses:

- `"Group"` — group by Godot groups
- `"Class"` — group by Godot class
- `"Proximity"` — cluster spatially nearby nodes
- `"None"` — flat list (default)

### `bearing_format`

Controls how bearings are reported in relationship and spatial context results:

- `"Cardinal"` — compass directions (N, NE, SW, etc.)
- `"Degrees"` — numeric degrees (0–360)
- `"Both"` — both formats together

### `token_hard_cap`

Sets a hard maximum on response token count. Unlike `token_budget` (which the caller sets per-request), `token_hard_cap` is a server-side ceiling that cannot be exceeded regardless of what the caller requests:

```json
{
  "token_hard_cap": 4000
}
```

## Response format

`spatial_config` with no parameters returns the current configuration:

```json
{
  "static_patterns": [],
  "state_properties": {},
  "cluster_by": "None",
  "bearing_format": "Degrees",
  "expose_internals": false,
  "poll_interval": 100,
  "token_hard_cap": null
}
```

When you set parameters, the response echoes the new values:

```json
{
  "static_patterns": ["Env_*", "Decor_*"],
  "result": "ok"
}
```

## Example conversation

<AgentConversation :messages="messages0" />

## Tips

**Check current config before changing it.** Call `spatial_config` with no parameters to see what is active before you start adjusting values.

**Use `state_properties` to expose custom script variables.** If your nodes have exported properties like `health` or `ai_state`, register them here so they appear in snapshot and inspect responses.

**Use `static_patterns` for large scenes.** If your scene has hundreds of decorative environment nodes that never move, marking them as static dramatically reduces snapshot size without losing any relevant data.

**`token_hard_cap` protects the context window.** If you are concerned about runaway large scenes consuming too much context, set a hard cap as a safety net.
