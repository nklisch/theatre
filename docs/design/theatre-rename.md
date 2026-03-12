# Design: Rename Project to Theatre

## Overview

Rename the umbrella project from "Spectator" to "Theatre" to reflect that it
now contains two tools: **Spectator** (runtime observer) and **Director**
(editor-time scene operator). This is a naming/branding change, not an
architectural change.

### What Changes

| Item | Before | After |
|---|---|---|
| Project name | Spectator | Theatre |
| GitHub org/repo | `theatre-godot/theatre` | `theatre-godot/theatre` |
| Cargo.toml repository URL | `theatre-godot/theatre` | `theatre-godot/theatre` |
| Env var for Spectator port | `THEATRE_PORT` | `THEATRE_PORT` (fallback: `THEATRE_PORT`) |
| Godot settings prefix | `spectator/connection/...` | `theatre/spectator/...` |
| Plugin author fields | "Spectator Contributors" | "Theatre Contributors" |
| README | Spectator-only intro | Theatre umbrella intro |
| CLAUDE.md | "Spectator" project header | "Theatre" project header |
| Skill files | Spectator-as-project references | Theatre-as-project references |
| `theatre-deploy` script | `theatre-deploy` | `theatre-deploy` |
| docs/ prose | "the Theatre project" | "the Theatre project" |

### What Does NOT Change

| Item | Reason |
|---|---|
| Crate names (`spectator-server`, `spectator-godot`, `spectator-protocol`, `spectator-core`, `director`) | Tool-specific, not project-level |
| Crate directory paths (`crates/spectator-server/`, etc.) | Follow crate names |
| Binary names (`spectator-server`, `director`) | Tool-specific |
| GDExtension binary (`libspectator_godot.so`) | Follows crate name |
| Addon directories (`addons/spectator/`, `addons/director/`) | Tool-specific |
| GDExtension manifest (`spectator.gdextension`) | Follows addon/crate name |
| Wire protocol identifiers (`spectator:status`, `spectator:command`) | Runtime protocol, tied to tool |
| GDExtension class names (`SpectatorTCPServer`, `SpectatorCollector`, etc.) | Godot API surface |
| MCP server name in `.mcp.json` (`"spectator"`) | Tool-specific MCP identity |
| Autoload name (`SpectatorRuntime`) | Godot API surface |
| `spectator_internal` group name | Runtime marker |

---

## Implementation Units

### Unit 1: GitHub Repo Rename

**Action**: Use `gh` CLI to rename the repo. GitHub automatically sets up
redirects from the old URL.

```bash
# Rename repo (GitHub sets up redirects automatically)
gh repo rename theatre

# Note: GitHub org rename (spectator-godot → theatre-godot) requires manual
# steps in GitHub settings. Document this for the human.
```

**Human-required steps** (document in a migration checklist):
- GitHub org rename: Settings → Rename organization → `theatre-godot`
- Update any external CI/CD that references the old repo URL
- Update any published MCP config examples pointing to the old repo

**Acceptance Criteria**:
- [ ] Repo accessible at new URL
- [ ] Old URL redirects

---

### Unit 2: Root Cargo.toml

**File**: `Cargo.toml`

```toml
# Before
repository = "https://github.com/theatre-godot/theatre"

# After
repository = "https://github.com/theatre-godot/theatre"
```

No other changes to Cargo.toml. Workspace member paths, crate names, and
dependency declarations all stay.

**Acceptance Criteria**:
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo clippy --workspace` clean

---

### Unit 3: Environment Variable Rename

**Files**:
- `crates/spectator-server/src/main.rs`
- `crates/spectator-server/tests/support/godot_process.rs`
- `.mcp.json`

#### main.rs (line ~24)

```rust
// Before
let env_port: u16 = std::env::var("THEATRE_PORT")

// After — try THEATRE_PORT first, fall back to THEATRE_PORT for backward compat
let env_port: u16 = std::env::var("THEATRE_PORT")
    .or_else(|_| std::env::var("THEATRE_PORT"))
```

The fallback ensures existing deployments don't break. Log a deprecation
warning when the old var is used:

```rust
let (env_port, deprecated) = match std::env::var("THEATRE_PORT") {
    Ok(v) => (v, false),
    Err(_) => match std::env::var("THEATRE_PORT") {
        Ok(v) => (v, true),
        Err(_) => // default handling
    },
};
if deprecated {
    tracing::warn!("THEATRE_PORT is deprecated, use THEATRE_PORT instead");
}
```

#### godot_process.rs (test support, line ~49)

```rust
// Before
.env("THEATRE_PORT", port.to_string())

// After
.env("THEATRE_PORT", port.to_string())
```

#### .mcp.json

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

Note: MCP server key stays `"spectator"` (tool name). Binary path stays
`spectator-server` (crate name).

**Acceptance Criteria**:
- [ ] `THEATRE_PORT=9077 cargo run -p spectator-server` starts on port 9077
- [ ] `THEATRE_PORT=9077 cargo run -p spectator-server` still works (with deprecation warning)
- [ ] Neither set → default 9077

---

### Unit 4: Godot Settings Prefix

**Files**:
- `addons/spectator/plugin.gd`
- `addons/spectator/runtime.gd`

Change the Godot ProjectSettings prefix from `spectator/` to
`theatre/spectator/` to namespace under the Theatre umbrella while keeping
tool-specific separation (Director would use `theatre/director/`).

#### plugin.gd — settings registration

```gdscript
# Before
_add_setting("spectator/connection/port", TYPE_INT, 9077, ...)
_add_setting("spectator/connection/auto_start", TYPE_BOOL, true)
_add_setting("spectator/connection/client_idle_timeout_secs", TYPE_INT, 10, ...)
_add_setting("spectator/display/show_agent_notifications", TYPE_BOOL, true)
_add_setting("spectator/tracking/default_static_patterns", ...)
_add_setting("spectator/tracking/token_hard_cap", TYPE_INT, 5000, ...)

# After
_add_setting("theatre/spectator/connection/port", TYPE_INT, 9077, ...)
_add_setting("theatre/spectator/connection/auto_start", TYPE_BOOL, true)
_add_setting("theatre/spectator/connection/client_idle_timeout_secs", TYPE_INT, 10, ...)
_add_setting("theatre/spectator/display/show_agent_notifications", TYPE_BOOL, true)
_add_setting("theatre/spectator/tracking/default_static_patterns", ...)
_add_setting("theatre/spectator/tracking/token_hard_cap", TYPE_INT, 5000, ...)
```

#### runtime.gd — settings reads

```gdscript
# Before
ProjectSettings.get_setting("spectator/connection/auto_start", true)
ProjectSettings.get_setting("spectator/connection/port", 9077)
ProjectSettings.get_setting("spectator/connection/client_idle_timeout_secs", 10)
ProjectSettings.get_setting("spectator/display/show_agent_notifications", true)

# After
ProjectSettings.get_setting("theatre/spectator/connection/auto_start", true)
ProjectSettings.get_setting("theatre/spectator/connection/port", 9077)
ProjectSettings.get_setting("theatre/spectator/connection/client_idle_timeout_secs", 10)
ProjectSettings.get_setting("theatre/spectator/display/show_agent_notifications", true)
```

#### plugin.gd — cleanup on disable

The `_disable_plugin()` function removes settings. Update all
`ProjectSettings.clear("spectator/...")` calls to use the new prefix.

**Breaking change**: Existing Godot projects with `spectator/connection/port`
in their `project.godot` will need to update the key prefix. No automatic
migration — the default values apply when the old keys aren't found.

**Acceptance Criteria**:
- [ ] New project: settings appear under `theatre/spectator/` in Project Settings
- [ ] Plugin disable: all `theatre/spectator/` settings removed
- [ ] Default values work when no settings are present

---

### Unit 5: Plugin Author Fields

**File**: `addons/spectator/plugin.cfg`

```ini
# Before
author="Spectator Contributors"

# After
author="Theatre Contributors"
```

**File**: `addons/director/plugin.cfg` — already uses `author="Theatre"`,
update to be consistent:

```ini
# Before
author="Theatre"

# After
author="Theatre Contributors"
```

**Acceptance Criteria**:
- [ ] Both plugin.cfg files show "Theatre Contributors"

---

### Unit 6: README.md Rewrite

**File**: `README.md`

Full rewrite. New structure:

```markdown
# Theatre

A toolkit giving AI agents the ability to build and debug Godot games.
Two MCP servers — **Director** (build) and **Spectator** (observe) — that
work together or independently.

## Tools

| Tool | Purpose | MCP Server | Godot Side |
|---|---|---|---|
| **Director** | Create/modify scenes, resources, tilemaps, animations | `director serve` | GDScript addon (`addons/director/`) |
| **Spectator** | Observe spatial state of a running game | `spectator-server` | GDExtension + GDScript (`addons/spectator/`) |

## Quick Start

### Spectator — Runtime Observation

[existing Spectator setup content, updated references]

### Director — Scene Manipulation

[Director setup content from director-spec.md]

## Prerequisites
[combined]

## Development
[combined build commands]

## Troubleshooting
[combined]
```

The README should be comprehensive but not duplicate the individual tool
docs. Link to `docs/director-spec.md` and `docs/SPEC.md` for details.

**Acceptance Criteria**:
- [ ] README introduces Theatre as the umbrella
- [ ] Both Spectator and Director have setup sections
- [ ] All command examples use correct binary/crate names
- [ ] No stale references to "Spectator" as the project name

---

### Unit 7: CLAUDE.md Update

**File**: `CLAUDE.md`

Changes:
1. Header: `# Spectator — Agent Instructions` → `# Theatre — Agent Instructions`
2. "What This Is" section: describe Theatre as the umbrella with two tools
3. Repository Layout: add Director entries (they may already be there)
4. Build Commands: ensure Director commands are listed
5. `THEATRE_PORT` references → `THEATRE_PORT`
6. `theatre-deploy` → `theatre-deploy`
7. Key Constraints: generalize where appropriate
8. Architecture Rules: add Director rules alongside Spectator rules

The project name in Agent Tracker section already says "theatre" — no change
needed there.

**Acceptance Criteria**:
- [ ] Header says "Theatre"
- [ ] Both tools described
- [ ] Env var references use `THEATRE_PORT`
- [ ] Build commands cover both tools
- [ ] Agent Tracker section unchanged

---

### Unit 8: Skill Files

#### `.agents/skills/spectator-dev/SKILL.md`

Update orientation text:
```markdown
# Before (line 6-8)
# Spectator — Developer Orientation
Spectator is a **Rust MCP server** ...

# After
# Theatre — Developer Orientation
Theatre is a Godot AI agent toolkit containing two tools:
- **Spectator**: Rust MCP server + GDExtension for runtime spatial observation
- **Director**: Rust MCP server + GDScript addon for editor-time scene manipulation

This skill covers Spectator development. See the Director crate and
`docs/director-spec.md` for Director specifics.
```

Update the crate map to include Director:
```
theatre/
├── crates/
│   ├── spectator-server/     # Spectator MCP binary
│   ├── spectator-godot/      # Spectator GDExtension cdylib
│   ├── spectator-protocol/   # Shared: TCP wire format, message types
│   ├── spectator-core/       # Shared: spatial math, bearing, indexing
│   └── director/             # Director MCP binary
├── addons/spectator/         # Spectator Godot addon
├── addons/director/          # Director Godot addon
├── docs/                     # All design docs
└── tests/
    ├── wire-tests/           # Spectator E2E tests
    └── director-tests/       # Director E2E tests
```

Update `THEATRE_PORT` references to `THEATRE_PORT`.

#### `.agents/skills/spectator/SKILL.md`

This is the end-user skill for using Spectator MCP tools. The tool name stays
"Spectator". Only update:
- Line 6: Add context that Spectator is part of the Theatre toolkit
- Any references to `THEATRE_PORT` → `THEATRE_PORT`

#### `.agents/skills/godot-addon/SKILL.md`

Read and update any "Spectator project" references to "Theatre project" while
keeping "Spectator addon" references.

#### `.claude/skills/patterns/godot-e2e-harness.md`

Update `THEATRE_PORT` references to `THEATRE_PORT`.

#### `.claude/skills/patterns/*.md`

Audit all pattern files. Most reference crate names (keep) but check for
project-name references to update.

**Acceptance Criteria**:
- [ ] spectator-dev skill introduces Theatre as the umbrella
- [ ] spectator skill mentions Theatre context
- [ ] All `THEATRE_PORT` references updated
- [ ] Crate name references unchanged

---

### Unit 9: Documentation Audit

**Files**: All `docs/*.md` and `docs/design/*.md` (~40 files)

Strategy: These are historical design documents. Apply a targeted find-replace
for project-level references only:

1. **Headers/intros** that say "the Theatre project" → "the Theatre project"
2. **Repository references** (`theatre-godot/theatre` URL) → new URL
3. **`THEATRE_PORT`** → `THEATRE_PORT`
4. **`theatre-deploy`** → `theatre-deploy`
5. **`spectator.toml`** (if referenced) → `theatre.toml`

Do NOT change:
- Crate name references (`spectator-server`, `spectator-godot`, etc.)
- Tool name references ("Spectator gives you 9 MCP tools...")
- Code examples referencing crate imports
- Wire protocol identifiers

Files requiring significant prose changes (not just find-replace):
- `docs/VISION.md` — project vision, likely says "Spectator" as project name
- `docs/SPEC.md` — may have project-level language
- `docs/TECH.md` — similar
- `docs/director-spec.md` — already uses "Theatre" terminology, verify consistency
- `docs/DIRECTOR-ROADMAP.md` — already uses "Theatre", verify consistency

Files that are purely design implementation docs (M0-M11, phases, refactors):
these reference crate names and tool names, not the project name. Likely need
minimal or no changes. Scan for `THEATRE_PORT` and `theatre-deploy` only.

**Acceptance Criteria**:
- [ ] No docs refer to "the Theatre project" (should say "the Theatre project")
- [ ] `THEATRE_PORT` not found in any doc (except in deprecation/migration notes)
- [ ] Crate name references preserved

---

### Unit 10: External Tooling — theatre-deploy

The `theatre-deploy` script at `~/.local/bin/theatre-deploy` is
referenced in CLAUDE.md and README but lives outside the repo.

**Design**: Create a new `scripts/theatre-deploy` in the repo that handles
both tools:

```bash
#!/usr/bin/env bash
# Build and deploy Theatre addons to a Godot project.
# Usage: theatre-deploy [--release] [project_path...]

set -euo pipefail

MODE="debug"
if [ "${1:-}" = "--release" ]; then
    MODE="release"
    shift
fi

TARGETS=("${@:-$HOME/godot/test-harness}")
THEATRE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Build GDExtension
cargo build -p spectator-godot ${MODE:+--$MODE}

SRC="$THEATRE_ROOT/target/$MODE/libspectator_godot.so"
for PROJECT in "${TARGETS[@]}"; do
    DST="$PROJECT/addons/spectator/bin/linux/"
    mkdir -p "$DST"
    cp "$SRC" "$DST"
    echo "Deployed spectator-godot → $PROJECT"
done
```

The user can symlink `~/.local/bin/theatre-deploy` → `scripts/theatre-deploy`.

Also update `scripts/copy-gdext.sh` comments to reference Theatre.

**Acceptance Criteria**:
- [ ] `scripts/theatre-deploy` exists and works
- [ ] CLAUDE.md references `theatre-deploy`
- [ ] Old `theatre-deploy` references noted as deprecated

---

### Unit 11: CI Workflow

**File**: `.github/workflows/ci.yml`

The CI file references crate names in build commands (`cargo build -p
spectator-server`), which stay. Update only:

```yaml
# Before (line 67)
- name: Build server (release)
  run: cargo build --release -p spectator-server

# After
- name: Build Spectator server (release)
  run: cargo build --release -p spectator-server

- name: Build Director server (release)
  run: cargo build --release -p director
```

Add Director to the release build if not already present.

**Acceptance Criteria**:
- [ ] CI builds both spectator-server and director
- [ ] Step names clarify which tool is being built
- [ ] CI passes

---

### Unit 12: Test Project Updates

**Files**:
- `tests/godot-project/project.godot`
- `examples/2d-platformer-demo/project.godot`

#### tests/godot-project/project.godot

```ini
# Before
config/name="SpectatorTests"

# After
config/name="TheatreTests"
```

Update Godot settings prefix references if present:
```ini
# Before (in [spectator] section if exists)
spectator/connection/port=9077

# After
theatre/spectator/connection/port=9077
```

#### examples/2d-platformer-demo/project.godot

```ini
# Before
config/name="Spectator 2D Demo"

# After
config/name="Theatre 2D Demo"
```

**Acceptance Criteria**:
- [ ] Test project name is "TheatreTests"
- [ ] Example project name is "Theatre 2D Demo"
- [ ] Settings prefix matches Unit 4 changes

---

### Unit 13: Migration Checklist Document

**File**: `docs/THEATRE-MIGRATION.md`

Create a migration guide for existing users:

```markdown
# Migrating to Theatre

## For Users of Spectator Addon

### Environment Variable
- `THEATRE_PORT` → `THEATRE_PORT`
- The old variable still works but logs a deprecation warning

### Godot Project Settings
- Settings prefix changed from `spectator/` to `theatre/spectator/`
- If you had custom settings in project.godot, update the keys:
  - `spectator/connection/port` → `theatre/spectator/connection/port`
  - `spectator/connection/auto_start` → `theatre/spectator/connection/auto_start`
  - etc.
- Or delete the old keys and re-enable the plugin (defaults apply)

### MCP Configuration
- Update `.mcp.json` env block: `THEATRE_PORT` → `THEATRE_PORT`
- The `spectator` MCP server name is unchanged

### Deploy Script
- `theatre-deploy` → `theatre-deploy`

## For Contributors

### Git Remote
- Update remote URL:
  `git remote set-url origin https://github.com/theatre-godot/theatre.git`
- Old URLs redirect automatically (GitHub feature)
```

**Acceptance Criteria**:
- [ ] Migration doc covers all breaking changes
- [ ] Each change has a clear before/after

---

## Implementation Order

```
1. Unit 1:  GitHub Repo Rename (gh repo rename)
2. Unit 2:  Root Cargo.toml (repository URL)
3. Unit 3:  Environment Variable Rename (Rust code + .mcp.json)
4. Unit 4:  Godot Settings Prefix (plugin.gd + runtime.gd)
5. Unit 5:  Plugin Author Fields (both plugin.cfg files)
6. Unit 6:  README.md Rewrite
7. Unit 7:  CLAUDE.md Update
8. Unit 8:  Skill Files
9. Unit 9:  Documentation Audit (bulk — all docs/)
10. Unit 10: External Tooling (theatre-deploy script)
11. Unit 11: CI Workflow
12. Unit 12: Test Project Updates
13. Unit 13: Migration Checklist Document
```

Dependencies:
- Units 2-13 are independent of each other (all file edits)
- Unit 1 (GitHub rename) should go first so the new URL is live before
  updating references
- Units 3+4 should be committed together (env var + settings prefix are
  a coordinated change)
- Unit 13 should be written last (references all other changes)

Suggested commit grouping:
1. `rename: GitHub repo spectator → theatre` (Unit 1, if done via gh)
2. `rename: env var THEATRE_PORT → THEATRE_PORT with fallback` (Units 2, 3, 4, 5)
3. `rename: rewrite README and CLAUDE.md for Theatre umbrella` (Units 6, 7)
4. `rename: update skill files and docs for Theatre` (Units 8, 9)
5. `rename: add theatre-deploy, update CI, test projects` (Units 10, 11, 12)
6. `docs: add Theatre migration guide` (Unit 13)

## Testing

### Build Verification
```bash
cargo build --workspace
cargo clippy --workspace
cargo fmt --check
cargo test --workspace
```

### Manual Verification
```bash
# Env var works
THEATRE_PORT=9077 cargo run -p spectator-server
# Backward compat
THEATRE_PORT=9077 cargo run -p spectator-server  # expect deprecation warning
# theatre-deploy script
./scripts/theatre-deploy ~/godot/test-harness
```

### Grep Verification
```bash
# Should find zero results (project-name references cleaned up):
grep -r "the Theatre project" docs/ CLAUDE.md README.md
grep -r "THEATRE_PORT" --include="*.rs" --include="*.json" --include="*.md" \
  | grep -v "deprecated\|fallback\|migration\|THEATRE_PORT"

# Should still find results (tool-name references preserved):
grep -r "spectator-server" crates/
grep -r "SpectatorTCPServer" crates/
grep -r "addons/spectator" addons/
```

## Verification Checklist

- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` clean
- [ ] No doc references "the Theatre project" (should say "Theatre")
- [ ] `THEATRE_PORT` only appears in backward-compat code and migration docs
- [ ] All crate names, binary names, and addon paths unchanged
- [ ] GitHub repo accessible at new URL
- [ ] README introduces Theatre with both tools
- [ ] CLAUDE.md header says "Theatre"
- [ ] Migration guide covers all breaking changes
