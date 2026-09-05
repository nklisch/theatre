---
source_handle: theatre-client-feedback-codex-plugins
fetched: 2026-09-04
source_title: Package your plugin - OpenAI Developers
source_url: https://developers.openai.com/plugins/build/plugins
---

OpenAI documents a Codex/ChatGPT plugin packaging format that can bundle skills,
MCP configuration, and lifecycle hooks.

## Attested details

1. [Required manifest] Every plugin has `.codex-plugin/plugin.json`; optional root-level components include `skills/`, `hooks/hooks.json`, `.mcp.json`, `.app.json`, and assets.
2. [Manifest component links] `skills`, `mcpServers`, and `hooks` are relative plugin-root manifest paths. A plugin may rely on the default `hooks/hooks.json` without a manifest `hooks` field.
3. [MCP configuration] A bundled `.mcp.json` can define an MCP server map. After installation, users can enable or disable bundled servers and tune approval policy from Codex configuration.
4. [Hook opt-in] Plugin-bundled hooks are non-managed hooks. Installing or enabling a plugin does not trust them automatically; Codex skips them until the user reviews and trusts the hook definition.
5. [Local distribution] The documentation describes repo-local marketplaces at `.agents/plugins/marketplace.json`, local testing, and the CLI command `codex plugin marketplace add`. It states availability can differ by host surface.
