---
id: investigate-stage-output-schema-client-rejection
kind: feature
tags: [stage, mcp, interoperability]
parent: null
release: null
completed: 2026-09-05
---

# Investigate Stage output-schema rejection in Pi

Stage and Director normalize generated schemas in schema positions only, preserving literal booleans and expressing nullability as standard JSON Schema unions. Typed MCP results carry matching structured content and JSON text; snapshot/watch schemas cover their response variants. SDK transport regressions and actual Pi status/watch calls verify interoperability.
