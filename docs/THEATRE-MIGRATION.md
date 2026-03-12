# Migrating to Theatre

This document covers breaking changes when upgrading from the Spectator
project to the Theatre umbrella release.

## For Users of the Spectator Addon

### Environment Variable

- `SPECTATOR_PORT` → `THEATRE_PORT`
- The old variable still works but logs a deprecation warning:
  `SPECTATOR_PORT is deprecated, use THEATRE_PORT instead`

Update your `.mcp.json`:
```json
{
  "mcpServers": {
    "spectator": {
      "type": "stdio",
      "command": "./target/release/spectator-server",
      "env": {
        "THEATRE_PORT": "9077"
      }
    }
  }
}
```

### Godot Project Settings

The settings prefix changed from `spectator/` to `theatre/spectator/`.

If you had custom settings in `project.godot`, update the keys:
- `spectator/connection/port` → `theatre/spectator/connection/port`
- `spectator/connection/auto_start` → `theatre/spectator/connection/auto_start`
- `spectator/connection/client_idle_timeout_secs` → `theatre/spectator/connection/client_idle_timeout_secs`
- `spectator/display/show_agent_notifications` → `theatre/spectator/display/show_agent_notifications`
- `spectator/shortcuts/marker_key` → `theatre/spectator/shortcuts/marker_key`
- `spectator/shortcuts/pause_key` → `theatre/spectator/shortcuts/pause_key`
- `spectator/tracking/default_static_patterns` → `theatre/spectator/tracking/default_static_patterns`
- `spectator/tracking/token_hard_cap` → `theatre/spectator/tracking/token_hard_cap`

Or delete the old keys and re-enable the plugin — defaults apply automatically.

### MCP Configuration

The `spectator` MCP server name is **unchanged**. Only the env var changes:
- `SPECTATOR_PORT` → `THEATRE_PORT`

### Deploy Script

- `spectator-deploy` → `theatre-deploy`

The new script lives at `scripts/theatre-deploy` in the repo. Symlink it:
```bash
ln -s /path/to/theatre/scripts/theatre-deploy ~/.local/bin/theatre-deploy
```

## For Contributors

### Git Remote

Update your remote URL after the GitHub repo rename:
```bash
git remote set-url origin https://github.com/theatre-godot/theatre.git
```

Old URLs redirect automatically (GitHub feature), so existing clones continue
to work, but updating is recommended.

### What Did NOT Change

These identifiers are unchanged — they are tool-specific, not project-level:

| Item | Value |
|---|---|
| Crate names | `spectator-server`, `spectator-godot`, `spectator-protocol`, `spectator-core`, `director` |
| Binary names | `spectator-server`, `director` |
| GDExtension binary | `libspectator_godot.so` |
| Addon directories | `addons/spectator/`, `addons/director/` |
| GDExtension manifest | `spectator.gdextension` |
| Wire protocol identifiers | `spectator:status`, `spectator:command`, `spectator:activity` |
| GDExtension class names | `SpectatorTCPServer`, `SpectatorCollector`, `SpectatorRecorder` |
| MCP server name in `.mcp.json` | `"spectator"` |
| Autoload name | `SpectatorRuntime` |
| `spectator_internal` group name | (runtime marker) |
