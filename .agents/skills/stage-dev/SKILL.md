---
name: stage-dev
description: >
  Orientation for working on the Theatre codebase. Use when developing Stage
  or tracing its relationship to Director, the CLI, protocol, and Godot addons.
---

# Theatre — Developer Orientation

Theatre provides two engine-facing tools: **Director** authors project content;
**Stage** observes and interacts with a running game. Start with
[architecture](../../../docs/ARCHITECTURE.md),
[contracts](../../../docs/CONTRACT.md), and
[principles](../../../docs/PRINCIPLES.md) for durable ownership and semantics.
Use [Workbench conventions](../../../.work/CONVENTIONS.md) for verification
commands rather than maintaining a second command list here.

## Find the owning component

| Component | Responsibility |
|---|---|
| `crates/stage-server` | MCP/CLI tools, session state, spatial reasoning, clip analysis |
| `crates/stage-godot` | GDExtension classes, engine access, runtime actions, capture |
| `crates/stage-protocol` | Shared Stage wire types and boundary helpers |
| `crates/stage-core` | Pure spatial, budget, delta, watch, and projection logic |
| `crates/director` | Authoring MCP/CLI surface and Godot backend/process ownership |
| `crates/theatre-cli` | Installation, project setup, deployment, rules, and feedback CLI helpers |
| `crates/theatre-feedback` | Shared typed reader and management surface for project-local human evidence |
| `crates/theatre-docs-gen` | Tool-schema extraction for the public reference |
| `addons/stage` | GDScript plugin, runtime lifecycle, diagnostics, dock, and runtime feedback entrypoint |
| `addons/director` | GDScript editor, daemon, one-shot, operation, run-control, and editor-feedback implementations |
| `addons/theatre_shared` | Shared feedback producer/composer support payload; not a plugin |

Stage's server binary and GDExtension are separate artifacts in separate
processes. They communicate over TCP; they never link against each other.
`stage-godot` depends on `stage-protocol`, not `stage-core`. Keep Godot objects on
the main thread; workers may process owned plain data such as captured pixels.

The engine reports actual scene and physics state and executes actions.
Server-side logic shapes observations and analyzes retained data. The recorder
owns capture-local buffering, triggers, and clip persistence so recording does
not depend on an agent session. Project-local human feedback is separate from
recording and uses `theatre-feedback` plus `addons/theatre_shared`. Do not infer
recorder ownership from an old "thin addon" example.

## Trace a Stage tool call

1. A tool method in `crates/stage-server/src/mcp/mod.rs` accepts typed parameters.
2. Its handler builds an engine query and registers the pending response through
   `crates/stage-server/src/tcp.rs`.
3. The addon receives length-prefixed JSON through `StageTCPServer`, polled by
   `addons/stage/runtime.gd` on physics frames.
4. The appropriate query/action handler reads or changes engine state on the
   main thread, then returns engine data.
5. The server performs spatial reasoning and response shaping before returning
   text or image content to the agent.

The addon listens; an agent server connects and reconnects. Persisted MCP
sessions retain baselines and watches. One-shot CLI invocations create fresh
session state, so not every multi-call MCP workflow transfers to the shell.
Runtime identity, diagnostics, and viewport requests cross the same typed engine
boundary. Feedback reads bypass the live connection and open the selected
project's retained queue directly.

## Extend the tool or wire surface

- Put parameter types next to the owning handler and derive schemas from those
  types. Add the routed method and any output-schema registration in the server.
- Extend shared Stage query/response types in `stage-protocol` when new engine
  data is needed. Add engine dispatch and collection at the appropriate Godot
  boundary; do not create private mirrored Rust shapes at each endpoint.
- Use the existing [MCP handler](../patterns/mcp-tool-handler.md),
  [Director dispatch](../patterns/director-tool-macro.md), and
  [error layering](../patterns/error-layering.md) references where relevant.
- Verify observable behavior through the right pure, transport, and real-engine
  boundaries. Acceptance criteria and implementation plans belong in the active
  Workbench item, not a foundation document.
- Reconcile affected foundation semantics and generated tool references without
  copying complete schemas into prose.

## Related skills

- Godot-facing Rust: [gdext](../gdext/SKILL.md).
- MCP definitions/handling: [rmcp](../rmcp/SKILL.md).
- JSON Schema generation: [schemars](../schemars/SKILL.md).
- GDScript addon lifecycle: [godot-addon](../godot-addon/SKILL.md).
- Operating Stage: [theatre-stage](../theatre-stage/SKILL.md).
- Operating Director: [theatre-director](../theatre-director/SKILL.md).
- Recurring implementation shapes: [patterns](../patterns/SKILL.md).
