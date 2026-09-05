# Theatre — Vision

Theatre helps coding agents build and debug Godot projects using the engine's
own view of scenes, resources, and running games. It connects ordinary source
editing to structured engine operations and observable results.

## The problem

Source code describes what a game should do. It does not show where an enemy
actually moved, which collision shape blocked it, or how a menu rendered.
Without engine feedback, an agent depends on a developer to reproduce a problem,
describe it, and test each proposed fix. Scene and resource authoring also needs
care with Godot's ownership, references, and serialization rules.

Theatre gives the agent direct access to those engine capabilities while
keeping the developer in charge of the goal and the acceptable result.

## Two complementary tools

**Director** authors scenes and resources through Godot APIs. It handles node
composition, properties, signals, materials, animation, tilemaps, and project
settings. It can work through an editor plugin or a headless Godot process.
Scripts and shaders remain source files edited with normal coding tools.

**Stage** observes and interacts with a running game. It exposes scene structure,
spatial relationships, node state, physics queries, signals, and explicit debug
actions. An on-demand viewport image shows the latest completed render without
recording. Its recorder can retain spatial frames and viewport images for later
clip analysis, including storyboards and node-following filmstrips.

The **Theatre CLI** installs and deploys these tools and configures Godot
projects. The tools support the Model Context Protocol (MCP) for agent clients
and command-line invocation for shell-based use. Session-dependent behavior is
not interchangeable between persistent MCP and one-shot CLI calls.

## The working relationship

The agent should be able to observe, act, advance the game, and verify the
result without requiring a human to narrate every step. A developer can also
play the game, mark an interesting moment, or share editor and runtime context
for the agent to investigate. Shared feedback can include the relevant viewport,
selection or pointer, and a note. Human controls complement agent operation;
they are not its prerequisite.

Structured state and images serve different purposes. Exact positions,
velocities, and collision properties explain engine behavior; rendered pixels
show layout, lighting, occlusion, and appearance. Useful feedback combines the
two without overwhelming the agent with an entire scene dump.

The desired working loop is source inspection, authoring, validation, running,
interaction, observation, and explicit human feedback. The
[journeys](JOURNEYS.md) describe the supported paths and their limitations. This
vision does not promise that every step is a single Theatre tool call.

## Boundaries

- Theatre extends Godot; it does not replace the engine or simulate game logic.
- Director's authored project content and Stage's temporary runtime actions are
  distinct. Debugging a live object does not automatically save a scene change.
- Theatre complements source editing and a code debugger. Breakpoints, stack
  frames, and source-level stepping are not Stage's spatial-debugging role.
- The target is a developer's local Godot project and scene tree, not a hosted
  multi-tenant control service or a multiplayer replication debugger.
- Observation should not change game logic. Explicit agent actions are allowed
  interventions; capture overhead and debugging overlays still need to be
  considered when interpreting a run.
- Theatre supplies evidence and controls, not an assertion framework or an
  automatic verdict that a game is correct.
- Feedback is project-local evidence, not asynchronous agent steering. Optional
  client hooks can announce pending evidence at a later tool boundary.
- Ordinary GDScript remains the escape hatch for unusual procedural authoring.
  Theatre does not expose a general arbitrary-code execution tool.

See [architecture](ARCHITECTURE.md) for ownership and execution boundaries,
[contracts](CONTRACT.md) for observable semantics, and
[principles](PRINCIPLES.md) for the engineering trade-offs behind them.
