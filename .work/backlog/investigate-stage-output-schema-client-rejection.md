---
id: investigate-stage-output-schema-client-rejection
tags: [stage, mcp, interoperability]
created: 2026-09-05
updated: 2026-09-05
---

# Investigate Stage output-schema rejection in Pi

A connected Stage MCP server lists its tools, but Pi rejects a call to
`stage_runtime_status` before returning a runtime result:

```text
Failed to call tool: Tool 'runtime_status' has an invalid outputSchema:
nullable value must be ["boolean"]
```

Observed with Stage CLI 0.3.4 while inspecting the Voxlar combined starter on
Godot 4.7.1. Connecting the MCP server succeeded and listed 13 tools. The request
had empty arguments. No game was running at the time.

The equivalent project-selected CLI `stage runtime_status '{}'` returned valid
JSON with `connected:false`, `ready:false`, and a connection-refused diagnostic.
This separates the MCP schema/client path from actual runtime availability;
it does not establish whether the defect belongs to Stage's generated schema or
Pi's schema consumer. A running-game MCP call was not tested.

Workaround: use the Stage CLI for stateless inspection where appropriate. It
cannot replace persistent MCP sessions for watches, deltas, or session config.
No toolkit code was changed during the consumer task.

A related Director MCP symptom occurred on the same host:

```text
Failed to call tool: Tool editor_run has an output schema but did not return structured content
```

This was an observational `editor_run` status request with an explicit project
path. The equivalent Director CLI status succeeded and reported the editor's
saved scene was not running. A subsequent CLI start succeeded. This is additional
MCP output-handling evidence, not proof that the two symptoms share one cause.
