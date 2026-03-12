---
name: patterns
description: "Project code patterns and conventions. Auto-loads when implementing,
  designing, verifying, or reviewing code. Provides detailed pattern definitions
  with code examples."
user-invocable: false
allowed-tools: Read, Glob, Grep
---

# Project Patterns Reference

This skill contains detailed pattern documentation for the Spectator project.
See individual pattern files for full details with code examples.

Available patterns:
- [mcp-tool-handler.md](mcp-tool-handler.md) — MCP Tool Handler (#[tool_router] + Parameters<T> + query_addon + log_activity)
- [tcp-length-prefix.md](tcp-length-prefix.md) — Length-Prefixed TCP Codec (sync + async variants)
- [arc-mutex-state.md](arc-mutex-state.md) — Arc<Mutex<SessionState>> with Background Task
- [gdext-class.md](gdext-class.md) — GDExtension Class Export (#[derive(GodotClass)] + #[godot_api])
- [serde-tagged-enum.md](serde-tagged-enum.md) — Serde Tagged Enum (#[serde(tag="type")] protocol dispatch)
- [error-layering.md](error-layering.md) — Three-Tier Error Layering (CodecError → anyhow → McpError)
- [inline-test-fixtures.md](inline-test-fixtures.md) — Inline Test Module with Builder Fixtures
- [activity-logging.md](activity-logging.md) — Activity Logging Tail (summary up front, log_activity at handler tail)
- [clip-session.md](clip-session.md) — ClipSession Resource (open + analyze + finalize for clip analysis)
- [godot-e2e-harness.md](godot-e2e-harness.md) — Godot E2E Test Harness (GodotFixture + TestHarness + E2EHarness + DirectorFixture + DaemonFixture)
- [parse-mcp-enum.md](parse-mcp-enum.md) — ParseMcpEnum Trait (standardized string→enum parsing with consistent invalid_params errors)
