# Contract Rules

Rules for JSON wire format and MCP response field naming. Apply these any time
you add or modify fields in protocol structs, JSON response blocks, or MCP
parameter structs.

## ID Fields: Always `<resource>_id`

Any field carrying an identifier for a named resource **must** be named
`<resource>_id`, never a bare `"id"`.

```
recording_id   ✓    id      ✗
watch_id       ✓    id      ✗
session_id     ✓    id      ✗
marker_id      ✓    id      ✗
```

This applies everywhere the field appears: create responses, status responses,
list entries, delete responses, query parameters, and event payloads. The name
must be the same in every context so agents can correlate resources across
calls without reading docs.

**List entries are not exempt.** A `recordings` array whose entries use `"id"`
while every other recording endpoint uses `"recording_id"` is a contract
violation.

**Delete/remove responses must echo the id, not a boolean.** Returning
`{ "result": "ok", "deleted": true }` forces agents to track the id themselves.
Return `{ "result": "ok", "recording_id": "..." }` instead.

## Distance Fields: Always `distance`, Never `dist`

All distance/length fields in responses must use the full name `"distance"`.
The abbreviated form `"dist"` is banned in wire format.

```
"distance": 12.4   ✓    "dist": 12.4   ✗
```

This applies to: query result entries (nearest, radius), relationship results,
inspect spatial context, and any future tool that reports a measured distance.

## Schema Must Match Implementation

Any field in a MCP parameter struct (`#[derive(Deserialize, JsonSchema)]`)
**must** be forwarded to the addon or have a documented effect in the server.
Fields that are accepted but silently ignored are forbidden — they create false
affordances in the tool schema that agents will rely on.

If a feature is not yet implemented:
- Remove the field from the struct, or
- Return an explicit error when the field is set, explaining it is unimplemented.

## Field Names Follow Godot Naming Where a Godot Equivalent Exists

When a field maps to a Godot property or method, use the Godot name (or its
direct derivative per the rules below). Do not invent a different name.

```
global_position   ✓    abs              ✗   (Godot: global_position)
rotation_deg      ✓    rot              ✗   (Godot: rotation_degrees → rotation_deg)
rotation_y_deg    ✓    rot_y            ✗   (Y component of rotation_degrees)
position          ✓    local_origin     ✗   (Godot: position)
```

**Derivation rules** (see `godot-naming` skill / `GODOT-NAMING.md` for the full dictionary):
- Godot property → use exact name: `velocity`, `scale`, `visible`, `collision_layer`
- Godot `get_X()` method → drop `get_`: `get_class()` → `class`, `get_path()` → `path`
- Godot `is_X()` method → drop `is_`: `is_on_floor()` → `on_floor`
- `rotation_degrees` → `rotation_deg` (Spectator abbreviation convention — the only allowed abbreviation)
- Spectator-computed fields (no Godot equivalent) → descriptive snake_case, no abbreviations:
  `relative`, `bearing`, `bearing_deg`, `distance`, `occluded`, `timestamp_ms`

## Echo Fields Must Match Input Field Names

When a response confirms or echoes back a value the caller submitted, the
response field name must match the input parameter name exactly.

```
Input:  { "watch": { "node": "player", "track": ["position"] } }
Output: { "watch_id": "w1", "node": "player", "track": ["position"] }   ✓
Output: { "watch_id": "w1", "watching": "player", "tracking": [...] }   ✗
```

This means an agent can read a response using the same field names it used
to write the request, with no renaming or aliasing required.

## Consistent Envelope: `result` vs `results`

- Use `"results"` (plural array) for queries that return a ranked/filtered list:
  `nearest`, `radius`, `area`.
- Use `"result"` (singular object) for queries that return one answer:
  `raycast`, `path_distance`, `relationship`.

Do not mix these within the same tool. An agent reading the schema should be
able to predict the envelope shape from the query semantics alone.
