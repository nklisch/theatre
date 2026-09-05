---
source_handle: theatre-client-feedback-claude-plugins
fetched: 2026-09-04
source_title: Create plugins - Claude Code Docs
source_url: https://docs.anthropic.com/en/docs/claude-code/plugins
---

Claude Code documents plugins as a shareable distribution unit for skills,
agents, hooks, MCP servers, and monitors.

## Attested details

1. [Plugin structure] A Claude Code plugin can contain `.claude-plugin/plugin.json`, `skills/`, `agents/`, `hooks/hooks.json`, `.mcp.json`, and optional monitors; plugin components live at the plugin root.
2. [Plugin hook distribution] Plugin hooks are enabled with the plugin and use the same hook behavior as hooks configured in settings.
3. [Background monitors] A plugin may include `monitors/monitors.json`. Claude Code starts configured monitors while the plugin is active, and each stdout line is delivered to Claude as a notification during the session.
4. [Installation and activation] Local development can use `--plugin-dir`; installed-plugin changes may require `/reload-plugins` or a restart. Plugin availability and behavior can be limited by managed settings.
5. [Packaging boundary] Plugins are for reusable sharing across projects and teams; project-local `.claude/` configuration remains the documented alternative for a single-project customization.
