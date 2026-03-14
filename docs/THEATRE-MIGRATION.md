# Migrating to Theatre

This document covers breaking changes when upgrading from the Stage
project to the Theatre umbrella release.

## For Users of the Stage Addon

### Environment Variable

- `SPECTATOR_PORT` → `THEATRE_PORT`
- The old variable still works but logs a deprecation warning:
  `SPECTATOR_PORT is deprecated, use THEATRE_PORT instead`

Update your `.mcp.json`:
```json
{
  "mcpServers": {
    "stage": {
      "type": "stdio",
      "command": "./target/release/stage-server",
      "env": {
        "THEATRE_PORT": "9077"
      }
    }
  }
}
```

### Godot Project Settings

The settings prefix changed from `stage/` to `theatre/stage/`.

If you had custom settings in `project.godot`, update the keys:
- `stage/connection/port` → `theatre/stage/connection/port`
- `stage/connection/auto_start` → `theatre/stage/connection/auto_start`
- `stage/connection/client_idle_timeout_secs` → `theatre/stage/connection/client_idle_timeout_secs`
- `stage/display/show_agent_notifications` → `theatre/stage/display/show_agent_notifications`
- `stage/shortcuts/marker_key` → `theatre/stage/shortcuts/marker_key`
- `stage/shortcuts/pause_key` → `theatre/stage/shortcuts/pause_key`
- `stage/tracking/default_static_patterns` → `theatre/stage/tracking/default_static_patterns`
- `stage/tracking/token_hard_cap` → `theatre/stage/tracking/token_hard_cap`

Or delete the old keys and re-enable the plugin — defaults apply automatically.

### MCP Configuration

The `stage` MCP server name is **unchanged**. Only the env var changes:
- `SPECTATOR_PORT` → `THEATRE_PORT`

### Deploy Script

- `stage-deploy` → `theatre-deploy`

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
| Crate names | `stage-server`, `stage-godot`, `stage-protocol`, `stage-core`, `director` |
| Binary names | `stage-server`, `director` |
| GDExtension binary | `libstage_godot.so` (formerly `libspectator_godot.so`) |
| Addon directories | `addons/stage/`, `addons/director/` |
| GDExtension manifest | `stage.gdextension` |
| Wire protocol identifiers | `stage:status`, `stage:command`, `stage:activity` |
| GDExtension class names | `StageTCPServer`, `StageCollector`, `StageRecorder` |
| MCP server name in `.mcp.json` | `"stage"` |
| Autoload name | `StageRuntime` |
| `stage_internal` group name | (runtime marker) |
