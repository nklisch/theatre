# Design: `theatre` CLI

## Overview

A Rust binary (`theatre`) that unifies installation, project setup, and
deployment into a small set of commands. Replaces the current multi-step
manual workflow (build, copy, symlink, hand-edit JSON, enable plugins in UI)
with:

```bash
theatre install                  # build + copy everything to ~/.local/{bin,share}
theatre init ~/godot/my-game     # interactive project setup
theatre deploy ~/godot/my-game   # rebuild + redeploy everything
theatre enable ~/godot/my-game   # non-interactive plugin enable
```

### Design Goals

1. **One command from clone to installed** — `theatre install` builds and
   copies everything to a self-contained install location.
2. **One command to wire up a Godot project** — `theatre init` copies addons,
   generates `.mcp.json`, enables plugins, all interactively.
3. **Self-contained install** — all assets (addon templates, GDExtension
   binary, MCP server binaries) live under `~/.local/share/theatre/` and
   `~/.local/bin/`. No dependency on the source repo after install. This is
   the same layout a future bundled distribution (tarball, package manager)
   would produce.
4. **Minimal dependencies** — `clap` + `dialoguer` + `console` + `serde_json`.
   No tokio, no rmcp, no Godot deps.

### Install Layout

After `theatre install`, the filesystem looks like:

```
~/.local/bin/
  theatre                        # CLI binary
  stage                      # Stage MCP server + CLI (serve / <tool>)
  director                       # Director MCP server + CLI (serve / <tool>)

~/.local/share/theatre/
  addons/
    stage/
      plugin.gd
      plugin.cfg
      runtime.gd
      dock.gd
      dock.tscn
      debugger_plugin.gd
      stage.gdextension
      *.uid
      bin/
        linux/libstage_godot.so    # (or macos/*.dylib, windows/*.dll)
    director/
      plugin.gd
      plugin.cfg
      daemon.gd
      editor_ops.gd
      operations.gd
      message_codec.gd
      ops/
        *.gd
```

`theatre init` and `theatre deploy` always read from `~/.local/share/theatre/`.
They never reference the source repo. This means:

- **Source install**: `theatre install` builds from repo, copies into share dir.
- **Future bundled install**: tarball/package unpacks into the same two locations.
- **No migration needed** — same paths, same layout, same CLI commands.

## Implementation Units

### Unit 1: Crate Skeleton + Clap Subcommands

**File**: `crates/theatre-cli/Cargo.toml`

```toml
[package]
name = "theatre-cli"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[[bin]]
name = "theatre"
path = "src/main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
dialoguer = "0.11"
console = "0.15"
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = "1"

[dev-dependencies]
tempfile = "3"
```

**File**: `crates/theatre-cli/src/main.rs`

```rust
use anyhow::Result;
use clap::{Parser, Subcommand};

mod deploy;
mod enable;
mod init;
mod install;
mod paths;
mod project;

#[derive(Parser)]
#[command(name = "theatre", version, about = "Theatre — Godot AI agent toolkit")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build and install Theatre to ~/.local
    Install(install::InstallArgs),
    /// Set up a Godot project with Theatre addons and MCP config
    Init(init::InitArgs),
    /// Rebuild and redeploy Theatre to Godot projects
    Deploy(deploy::DeployArgs),
    /// Enable or disable Theatre plugins in a Godot project
    Enable(enable::EnableArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Install(args) => install::run(args),
        Command::Init(args) => init::run(args),
        Command::Deploy(args) => deploy::run(args),
        Command::Enable(args) => enable::run(args),
    }
}
```

**File**: Workspace `Cargo.toml` additions

Add `"crates/theatre-cli"` to both `members` and `default-members`.

**Implementation Notes**:
- Binary name is `theatre` (not `theatre-cli`). The crate name is
  `theatre-cli` to avoid collision with a potential future `theatre` library
  crate.
- No tokio — all operations are synchronous (cargo invocations, file copies,
  INI editing). Use `std::process::Command` for cargo builds.

**Acceptance Criteria**:
- [ ] `cargo build -p theatre-cli` produces `target/debug/theatre`
- [ ] `theatre --help` prints usage with all four subcommands
- [ ] `theatre --version` prints the workspace version

---

### Unit 2: Path Resolution (`paths.rs`)

**File**: `crates/theatre-cli/src/paths.rs`

```rust
use std::path::{Path, PathBuf};
use anyhow::{Context, Result, bail};

/// Installed Theatre layout under ~/.local.
pub struct TheatrePaths {
    /// Where binaries live: ~/.local/bin (or override)
    pub bin_dir: PathBuf,
    /// Where addon templates + GDExtension live: ~/.local/share/theatre
    pub share_dir: PathBuf,
}

impl TheatrePaths {
    /// Resolve the installed Theatre paths.
    ///
    /// 1. `THEATRE_SHARE_DIR` env var (if set) → share_dir
    /// 2. Default: `~/.local/share/theatre`
    ///
    /// bin_dir resolved separately via `resolve_bin_dir()`.
    pub fn resolve() -> Result<Self> { .. }

    /// Path to the addon source directory within the share dir.
    /// `~/.local/share/theatre/addons`
    pub fn addon_source(&self) -> PathBuf {
        self.share_dir.join("addons")
    }

    /// Path to the GDExtension binary within the share dir.
    /// `~/.local/share/theatre/addons/stage/bin/<platform>/<filename>`
    pub fn gdext_binary(&self) -> PathBuf {
        self.addon_source()
            .join("stage")
            .join("bin")
            .join(platform_dir())
            .join(gdext_filename())
    }

    /// Verify the share dir has been populated (install was run).
    pub fn validate_installed(&self) -> Result<()> { .. }
}

/// Context needed only during `theatre install` — knows about the source repo.
pub struct SourcePaths {
    /// Root of the Theatre source tree (for cargo builds)
    pub repo_root: PathBuf,
}

impl SourcePaths {
    /// Discover the repo root for install/deploy-from-source.
    ///
    /// 1. `THEATRE_ROOT` env var (if set)
    /// 2. Walk up from current executable to find workspace Cargo.toml
    /// 3. Walk up from current working directory
    pub fn discover() -> Result<Self> { .. }

    /// Path to a built binary in the repo's target dir.
    pub fn built_binary(&self, name: &str, release: bool) -> PathBuf {
        let mode = if release { "release" } else { "debug" };
        self.repo_root.join("target").join(mode).join(name)
    }

    /// Path to the built GDExtension in the repo's target dir.
    pub fn built_gdext(&self, release: bool) -> PathBuf {
        self.built_binary(gdext_filename(), release)
    }

    /// Path to the addon source in the repo.
    pub fn addon_source(&self) -> PathBuf {
        self.repo_root.join("addons")
    }
}

/// Platform-specific GDExtension library filename.
pub fn gdext_filename() -> &'static str {
    #[cfg(target_os = "linux")]
    { "libstage_godot.so" }
    #[cfg(target_os = "macos")]
    { "libstage_godot.dylib" }
    #[cfg(target_os = "windows")]
    { "stage_godot.dll" }
}

/// Platform-specific subdirectory name under addons/stage/bin/.
pub fn platform_dir() -> &'static str {
    #[cfg(target_os = "linux")]
    { "linux" }
    #[cfg(target_os = "macos")]
    { "macos" }
    #[cfg(target_os = "windows")]
    { "windows" }
}

/// Default install directory for binaries: ~/.local/bin
pub fn default_bin_dir() -> Result<PathBuf> {
    let home = home_dir()?;
    Ok(home.join(".local").join("bin"))
}

/// Default share directory: ~/.local/share/theatre
pub fn default_share_dir() -> Result<PathBuf> {
    let home = home_dir()?;
    Ok(home.join(".local").join("share").join("theatre"))
}

/// Resolve bin_dir: THEATRE_BIN_DIR env → ~/.local/bin
pub fn resolve_bin_dir() -> Result<PathBuf> { .. }

/// Resolve share_dir: THEATRE_SHARE_DIR env → ~/.local/share/theatre
pub fn resolve_share_dir() -> Result<PathBuf> { .. }

fn home_dir() -> Result<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .context("Neither HOME nor USERPROFILE is set")
}

/// Recursively copy a directory tree, skipping entries matched by `skip`.
/// Used by install (repo addons → share dir) and init (share dir → project).
pub fn copy_dir_recursive(
    src: &Path,
    dst: &Path,
    skip: &dyn Fn(&Path) -> bool,
) -> Result<u64> { .. }
```

**Implementation Notes**:
- Two path types: `TheatrePaths` (installed location — used by `init`,
  `deploy`, `enable`) and `SourcePaths` (repo location — used only by
  `install`). Clean separation: only `install` needs to know about the repo.
- `copy_dir_recursive` returns the number of files copied. Skips `.git`,
  `.godot`, and any paths matched by the `skip` closure. Creates destination
  directories as needed.
- `SourcePaths::discover()` walks up from `std::env::current_exe()` looking
  for a `Cargo.toml` containing `[workspace]`. Falls back to walking from
  `std::env::current_dir()`. This handles both running from `target/debug/`
  and running from the repo root.

**Acceptance Criteria**:
- [ ] `TheatrePaths::resolve()` returns correct default paths
- [ ] `TheatrePaths::resolve()` respects `THEATRE_SHARE_DIR` env override
- [ ] `TheatrePaths::validate_installed()` fails when share dir is empty
- [ ] `SourcePaths::discover()` finds repo root when run from within repo
- [ ] `SourcePaths::discover()` respects `THEATRE_ROOT` env override
- [ ] `SourcePaths::discover()` fails with clear error outside repo without env
- [ ] `gdext_filename()` returns platform-correct filename
- [ ] `platform_dir()` returns platform-correct directory name
- [ ] `copy_dir_recursive` copies files preserving tree structure
- [ ] `copy_dir_recursive` skips paths matched by skip closure

---

### Unit 3: Godot Project Helpers (`project.rs`)

**File**: `crates/theatre-cli/src/project.rs`

```rust
use std::path::{Path, PathBuf};
use anyhow::{Context, Result, bail};

/// Validate that a path is a Godot project (contains project.godot).
pub fn validate_project(path: &Path) -> Result<()> { .. }

/// Read project.godot and return its contents as a String.
pub fn read_project_godot(project: &Path) -> Result<String> { .. }

/// Check which Theatre plugins are currently enabled in project.godot.
/// Returns (stage_enabled, director_enabled).
pub fn check_plugins_enabled(project: &Path) -> Result<(bool, bool)> { .. }

/// Enable or disable a plugin in project.godot.
///
/// Parses the `[editor_plugins]` section and modifies the
/// `enabled=PackedStringArray(...)` value. Creates the section if missing.
///
/// `plugin_cfg_path` is the res:// path, e.g.
/// `"res://addons/stage/plugin.cfg"`.
pub fn set_plugin_enabled(
    project: &Path,
    plugin_cfg_path: &str,
    enabled: bool,
) -> Result<()> { .. }

/// Add an autoload entry to project.godot if not already present.
///
/// Entries follow Godot format: `Name="*res://path/to/script.gd"`
/// The `*` prefix means "enabled".
pub fn set_autoload(
    project: &Path,
    name: &str,
    script_path: &str,
) -> Result<()> { .. }

/// Remove an autoload entry from project.godot.
pub fn remove_autoload(
    project: &Path,
    name: &str,
) -> Result<()> { .. }

/// Generate .mcp.json content for a project.
///
/// `stage_bin` and `director_bin` are absolute paths to the installed
/// binaries.
pub fn generate_mcp_json(
    stage_bin: &Path,
    director_bin: &Path,
    include_stage: bool,
    include_director: bool,
    port: Option<u16>,
) -> serde_json::Value { .. }

/// Write .mcp.json to project root. Returns false without writing if file
/// exists and overwrite is false.
pub fn write_mcp_json(
    project: &Path,
    content: &serde_json::Value,
    overwrite: bool,
) -> Result<bool> { .. }
```

**Implementation Notes**:

project.godot is a Godot-flavored INI format. Key parsing rules:
- Sections are `[section_name]` on their own line
- Values are `key=value` (no spaces around `=` in Godot's output)
- `PackedStringArray(...)` contains comma-separated quoted strings
- Empty array: `PackedStringArray()`
- With entries: `PackedStringArray("res://addons/stage/plugin.cfg", "res://addons/director/plugin.cfg")`

`set_plugin_enabled` approach:
1. Read file as string
2. Find `[editor_plugins]` section (create if missing, append before next
   `[section]` or at EOF)
3. Find `enabled=PackedStringArray(...)` line within that section
4. Parse the array contents, add/remove the plugin path
5. Write the modified line back
6. Write file

Do NOT use a generic INI parser — Godot's format has quirks (no quoting rules
for section names, `PackedStringArray` syntax). Simple string manipulation is
more reliable and preserves formatting/comments.

`set_autoload` approach:
1. Find `[autoload]` section (create if missing)
2. Check if entry already exists (line starting with `Name=`)
3. If not, append `Name="*res://path"` after the section header
4. Blank line between the entry and the next section header

The `.mcp.json` structure:
```json
{
  "mcpServers": {
    "stage": {
      "type": "stdio",
      "command": "/home/user/.local/bin/stage",
      "args": ["serve"]
    },
    "director": {
      "type": "stdio",
      "command": "/home/user/.local/bin/director",
      "args": ["serve"]
    }
  }
}
```

When `port` is `Some(p)` and `p != 9077`, add `"env": { "THEATRE_PORT": "<p>" }`
to the stage entry.

**Acceptance Criteria**:
- [ ] `validate_project` returns Ok for a dir containing `project.godot`
- [ ] `validate_project` returns descriptive error for missing file
- [ ] `check_plugins_enabled` correctly parses empty `PackedStringArray()`
- [ ] `check_plugins_enabled` correctly detects enabled plugins
- [ ] `set_plugin_enabled` adds a plugin to an empty array
- [ ] `set_plugin_enabled` adds a plugin alongside existing entries
- [ ] `set_plugin_enabled` removes a plugin when `enabled=false`
- [ ] `set_plugin_enabled` is idempotent (adding twice doesn't duplicate)
- [ ] `set_plugin_enabled` creates `[editor_plugins]` section if missing
- [ ] `set_autoload` adds entry without duplicating
- [ ] `set_autoload` creates `[autoload]` section if missing
- [ ] `remove_autoload` removes an existing entry
- [ ] `remove_autoload` is a no-op if entry doesn't exist
- [ ] `generate_mcp_json` produces valid JSON with correct structure
- [ ] `generate_mcp_json` includes THEATRE_PORT env only for non-default port
- [ ] `write_mcp_json` with `overwrite=false` does not clobber existing file

---

### Unit 4: Install Command (`install.rs`)

**File**: `crates/theatre-cli/src/install.rs`

```rust
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct InstallArgs {
    /// Installation directory for binaries (default: ~/.local/bin)
    #[arg(long)]
    bin_dir: Option<PathBuf>,

    /// Installation directory for shared data (default: ~/.local/share/theatre)
    #[arg(long)]
    share_dir: Option<PathBuf>,
}

pub fn run(args: InstallArgs) -> Result<()> { .. }
```

**Workflow**:

1. Resolve `SourcePaths::discover()` to find the repo root.
2. Resolve `bin_dir` from args or `resolve_bin_dir()`.
3. Resolve `share_dir` from args or `resolve_share_dir()`.
4. Create both directories if they don't exist.
5. Run `cargo build --release -p stage-server -p stage-godot -p director -p theatre-cli`
   from `repo_root` using `std::process::Command`. Stream output to
   stderr (inherit stderr). Fail if cargo returns non-zero.
6. Copy three binaries from `target/release/` to `bin_dir`:
   - `stage`
   - `director`
   - `theatre`
7. Copy addon templates from `repo_root/addons/` to `share_dir/addons/`:
   - Copy `addons/stage/` (all `.gd`, `.tscn`, `.cfg`, `.gdextension`,
     `.uid` files — skip `bin/` subdirectory)
   - Copy `addons/director/` (all files including `ops/` subdirectory)
8. Copy the built GDExtension binary:
   - From `target/release/<gdext_filename>`
   - To `share_dir/addons/stage/bin/<platform>/<gdext_filename>`
9. Check if `bin_dir` is in `$PATH`. If not, print a warning.
10. Print summary.

**Console output** (using `console` crate for colors):

```
Theatre Install

  Building release binaries...
  ✓ stage
  ✓ director
  ✓ stage-godot
  ✓ theatre

  Installing to ~/.local/bin/:
  ✓ stage
  ✓ director
  ✓ theatre

  Installing to ~/.local/share/theatre/:
  ✓ addons/stage/ (23 files)
  ✓ addons/stage/bin/linux/libstage_godot.so
  ✓ addons/director/ (15 files)

  ⚠ ~/.local/bin is not in your PATH. Add it:
    export PATH="$HOME/.local/bin:$PATH"

Install complete. Run `theatre init <project>` to set up a Godot project.
```

**Implementation Notes**:
- `cargo build` is invoked once with all four `-p` flags for efficiency.
  The `--release` flag is hardcoded — install always builds release.
- Addon copy from repo uses `copy_dir_recursive` with a skip closure that
  excludes `bin/` under `stage/` (we copy the built binary separately
  in step 8 to ensure we get the freshly-built one, not whatever may have
  been in the repo's `bin/` from a previous local build).
- The `theatre` binary copies itself. This is fine — on Linux, the running
  binary is mmapped and the copy creates a new inode.
- `install` is the only command that touches `SourcePaths`. All other
  commands operate purely on `TheatrePaths` (the installed layout).

**Acceptance Criteria**:
- [ ] `theatre install` builds all four crates in release mode
- [ ] Three binaries are copied to `~/.local/bin/` (or `--bin-dir`)
- [ ] Addon templates are copied to `~/.local/share/theatre/addons/`
- [ ] GDExtension binary is placed in correct platform subdir
- [ ] Warning printed when bin_dir is not in PATH
- [ ] Re-running install overwrites everything cleanly (idempotent)
- [ ] Non-zero exit if cargo build fails
- [ ] `--bin-dir` and `--share-dir` overrides work

---

### Unit 5: Deploy Command (`deploy.rs`)

**File**: `crates/theatre-cli/src/deploy.rs`

```rust
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct DeployArgs {
    /// Godot project paths to deploy to
    #[arg(required = true)]
    projects: Vec<PathBuf>,

    /// Build in release mode (default: debug)
    #[arg(long)]
    release: bool,
}

pub fn run(args: DeployArgs) -> Result<()> { .. }
```

**Workflow**:

1. Resolve `SourcePaths::discover()` (deploy rebuilds from source).
2. Resolve `TheatrePaths::resolve()` (to update the share dir too).
3. Validate all project paths (each must contain `project.godot`).
4. Run `cargo build -p stage-godot -p stage-server -p director [--release]` (binary output: `stage`).
5. Update the share dir:
   a. Copy fresh GDExtension binary to `share_dir/addons/stage/bin/<platform>/`.
   b. Sync addon GDScript files from repo to share dir.
   c. Copy fresh server binaries to `bin_dir`.
6. For each project:
   a. Copy addon files from `share_dir` to project (full `addons/stage/`
      including `bin/` with fresh GDExtension, plus `addons/director/`).
   b. Skip copy if project's addon dir is a symlink (dev setup).
7. Print summary per project.

**Implementation Notes**:
- Deploy rebuilds GDExtension + both MCP servers, then updates both the
  share dir and all target projects. This ensures everything stays in sync.
- The share dir update means `theatre init` on a new project after a deploy
  will get the latest build too.
- Symlink detection: `std::fs::symlink_metadata(path).map(|m| m.is_symlink())`.
  If the project's `addons/stage` is a symlink, print a note and skip
  the copy (it already points to the repo source).
- Director addon files are also synced to target projects (they might have
  changed since `init`).

**Acceptance Criteria**:
- [ ] `theatre deploy ~/godot/game` builds and copies to project
- [ ] `theatre deploy --release` uses release mode
- [ ] Multiple projects in one command all receive updates
- [ ] Share dir is updated alongside project deployments
- [ ] MCP server binaries are updated in bin_dir
- [ ] Invalid project paths produce clear errors before building
- [ ] Symlinked addon dirs are detected and copy is skipped with a note

---

### Unit 6: Enable Command (`enable.rs`)

**File**: `crates/theatre-cli/src/enable.rs`

```rust
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct EnableArgs {
    /// Godot project path
    project: PathBuf,

    /// Enable only Stage (default: both)
    #[arg(long)]
    stage: bool,

    /// Enable only Director (default: both)
    #[arg(long)]
    director: bool,

    /// Disable instead of enable
    #[arg(long)]
    disable: bool,
}

pub fn run(args: EnableArgs) -> Result<()> { .. }
```

**Workflow**:

1. Validate project path.
2. Determine which plugins to act on:
   - If neither `--stage` nor `--director` is set, act on both.
   - Otherwise, act only on the ones specified.
3. For each selected plugin:
   a. Call `set_plugin_enabled(project, plugin_cfg_path, !args.disable)`.
   b. If enabling Stage, also call `set_autoload(project, "StageRuntime", "res://addons/stage/runtime.gd")`.
   c. If disabling Stage, call `remove_autoload(project, "StageRuntime")`.
4. Print result.

**Console output**:

```
Theatre Enable

  ✓ Stage enabled in project.godot
  ✓ StageRuntime autoload added
  ✓ Director enabled in project.godot
```

**Implementation Notes**:
- Stage requires both a plugin entry AND an autoload entry. Director
  only requires the plugin entry (it manages its own lifecycle).
- This command is intentionally simple — no interactive prompts. It's the
  scriptable counterpart to `init`'s interactive mode.
- Check if addon files actually exist in the project before enabling. If
  `addons/stage/plugin.cfg` is missing, warn that the plugin is enabled
  in `project.godot` but won't load until files are copied (suggest
  `theatre init` or `theatre deploy`).

**Acceptance Criteria**:
- [ ] `theatre enable ~/game` enables both plugins + autoload
- [ ] `theatre enable ~/game --stage` enables only Stage
- [ ] `theatre enable ~/game --director` enables only Director
- [ ] `theatre enable ~/game --disable` disables both + removes autoload
- [ ] Already-enabled plugins produce no error (idempotent)
- [ ] Missing addon files produce a warning (not an error)

---

### Unit 7: Init Command — Interactive TUI (`init.rs`)

**File**: `crates/theatre-cli/src/init.rs`

```rust
use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Args)]
pub struct InitArgs {
    /// Godot project path
    project: PathBuf,

    /// Skip interactive prompts, use defaults
    /// (both addons, both plugins, generate .mcp.json)
    #[arg(long, short = 'y')]
    yes: bool,
}

pub fn run(args: InitArgs) -> Result<()> { .. }
```

**Workflow**:

1. Resolve `TheatrePaths` and validate installed (`validate_installed()`).
   If not installed, print error: "Theatre is not installed. Run `theatre install` first."
2. Validate project path.
3. Check current state:
   - Do `addons/stage/` and `addons/director/` already exist in project?
   - Does `.mcp.json` already exist?
   - What plugins are enabled in `project.godot`?
4. If `--yes`, use defaults (all addons, all plugins, generate `.mcp.json`,
   port 9077, overwrite existing).
5. Otherwise, run interactive prompts:

```
Theatre — Project Setup

Which addons to install?
> [x] Stage — spatial awareness for AI agents
  [x] Director — scene and resource authoring

Generate .mcp.json for AI agent configuration? [Y/n]
Port [9077]:

Enable plugins in project.godot?
> [x] Stage
  [x] Director

Proceed with setup? [Y/n]
```

6. Execute selections:
   a. Copy selected addon directories from `share_dir/addons/` to
      project's `addons/`.
      - For Stage: copies everything including `bin/<platform>/` with
        the GDExtension binary.
      - For Director: copies all GDScript files including `ops/` subdir.
      - If addon dir already exists in project, prompt to overwrite (or
        overwrite silently with `--yes`).
   b. Generate and write `.mcp.json` if selected.
      - Binary paths resolve to `bin_dir / "stage"` and
        `bin_dir / "director"`.
      - Verify binaries exist at those paths. If not, warn but still
        generate (user may install later).
      - If `.mcp.json` exists, prompt to overwrite (or overwrite with `--yes`).
   c. Enable selected plugins via `set_plugin_enabled` + `set_autoload`.
7. Print summary.

**Interactive prompt details** (using `dialoguer`):

- **Addon selection**: `MultiSelect` with items
  `["Stage — spatial awareness for AI agents", "Director — scene and resource authoring"]`,
  both checked by default.
- **MCP config**: `Confirm` default yes. If yes, `Input` for port with
  default `"9077"` and u16 validation.
- **Plugin enable**: `MultiSelect` showing only addons selected in step 1.
  All checked by default.
- **Final confirm**: `Confirm` default yes.

**Console output after execution**:

```
Theatre — Project Setup

  ✓ Copied addons/stage/ (with GDExtension)
  ✓ Copied addons/director/
  ✓ Generated .mcp.json
  ✓ Enabled Stage in project.godot
  ✓ Enabled Director in project.godot

Done. Open your project in Godot — plugins are active.
```

**Implementation Notes**:
- Init reads exclusively from `TheatrePaths` (the installed share dir).
  It has zero knowledge of the source repo.
- Addon copy uses `copy_dir_recursive` from `paths.rs`. No skip closure
  needed here — everything in the share dir is intentionally curated by
  `install`.
- Port validation: parse as u16, reject < 1024 with a message about
  privileged ports.
- If the user deselects all addons AND .mcp.json AND all plugins, the
  confirm prompt should say "Nothing selected" and exit gracefully.

**Acceptance Criteria**:
- [ ] `theatre init ~/game` launches interactive prompts
- [ ] `theatre init ~/game --yes` skips prompts, uses all defaults
- [ ] Stage addon is copied with GDExtension binary
- [ ] Director addon is copied (GDScript only, with ops/ subdir)
- [ ] `.mcp.json` is generated with correct absolute paths to bin_dir
- [ ] `.mcp.json` includes THEATRE_PORT env only for non-default port
- [ ] Plugins are enabled in `project.godot`
- [ ] Autoload entry added for StageRuntime
- [ ] Existing addons trigger overwrite prompt (not silent clobber)
- [ ] Existing `.mcp.json` triggers overwrite prompt
- [ ] Missing share dir triggers clear "run theatre install first" error
- [ ] `--yes` overwrites existing files without prompting

---

## Implementation Order

1. **Unit 2: paths.rs** — No dependencies, foundational for all commands.
2. **Unit 3: project.rs** — No dependencies, foundational for init/enable.
3. **Unit 1: main.rs + Cargo.toml** — Crate skeleton wiring up module stubs.
4. **Unit 4: install.rs** — Uses `SourcePaths` + `TheatrePaths` + `copy_dir_recursive`.
5. **Unit 6: enable.rs** — Uses project.rs only, simplest command.
6. **Unit 5: deploy.rs** — Uses `SourcePaths` + `TheatrePaths` + project.rs.
7. **Unit 7: init.rs** — Uses `TheatrePaths` + project.rs + dialoguer. Most complex, last.

## Testing

### Unit Tests: `crates/theatre-cli/src/*.rs` (inline `#[cfg(test)]`)

Each module gets inline tests following the project's `inline-test-fixtures`
pattern. All tests use `tempfile::TempDir` for filesystem operations.

**paths.rs tests**:
- `test_gdext_filename` — verify platform-correct name
- `test_platform_dir` — verify platform-correct dir
- `test_default_bin_dir` — verify HOME-based resolution
- `test_default_share_dir` — verify HOME-based resolution
- `test_resolve_share_dir_from_env` — set `THEATRE_SHARE_DIR`, verify
- `test_source_discover_from_env` — set `THEATRE_ROOT`, verify
- `test_copy_dir_recursive` — copy a temp tree, verify structure
- `test_copy_dir_recursive_with_skip` — skip closure filters correctly

**project.rs tests** (using `tempfile::TempDir`):
- `test_validate_project_ok` — dir with project.godot
- `test_validate_project_missing` — dir without project.godot
- `test_check_plugins_empty` — empty PackedStringArray
- `test_check_plugins_one_enabled` — one plugin in array
- `test_check_plugins_both_enabled` — both plugins
- `test_set_plugin_enabled_add_to_empty` — add first plugin
- `test_set_plugin_enabled_add_second` — add alongside existing
- `test_set_plugin_enabled_remove` — remove a plugin
- `test_set_plugin_enabled_idempotent` — add twice, only one entry
- `test_set_plugin_enabled_creates_section` — missing [editor_plugins]
- `test_set_autoload_add` — add autoload entry
- `test_set_autoload_idempotent` — add twice, one entry
- `test_set_autoload_creates_section` — missing [autoload]
- `test_remove_autoload` — remove existing entry
- `test_remove_autoload_noop` — remove non-existent entry
- `test_generate_mcp_json_default_port` — no env entry
- `test_generate_mcp_json_custom_port` — includes THEATRE_PORT env
- `test_generate_mcp_json_stage_only` — director omitted
- `test_write_mcp_json_no_overwrite` — existing file, overwrite=false

**install.rs, deploy.rs, enable.rs**:
- These primarily orchestrate calls to paths.rs and project.rs, which are
  thoroughly tested. Integration coverage via the CLI integration test below.

### Integration Test: `crates/theatre-cli/tests/cli_integration.rs`

```rust
use std::process::Command;

#[test]
fn help_prints_subcommands() {
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .arg("--help")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("install"));
    assert!(stdout.contains("init"));
    assert!(stdout.contains("deploy"));
    assert!(stdout.contains("enable"));
}

#[test]
fn version_prints_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .arg("--version")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn enable_on_valid_project() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("project.godot"),
        "[editor_plugins]\nenabled=PackedStringArray()\n",
    ).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["enable", dir.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(dir.path().join("project.godot")).unwrap();
    assert!(content.contains("stage/plugin.cfg"));
    assert!(content.contains("director/plugin.cfg"));
}

#[test]
fn enable_on_missing_project() {
    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["enable", "/tmp/nonexistent-project-12345"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn init_fails_without_install() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("project.godot"), "").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_theatre"))
        .args(["init", dir.path().to_str().unwrap(), "--yes"])
        .env("THEATRE_SHARE_DIR", "/tmp/nonexistent-share-12345")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("install") || stderr.contains("not installed"));
}
```

## Verification Checklist

```bash
# Build the CLI
cargo build -p theatre-cli

# Run unit + integration tests
cargo test -p theatre-cli

# Verify help output
./target/debug/theatre --help
./target/debug/theatre install --help
./target/debug/theatre init --help
./target/debug/theatre deploy --help
./target/debug/theatre enable --help

# Full install (manual verification)
./target/debug/theatre install
theatre --version
ls ~/.local/bin/{theatre,stage,director}
ls ~/.local/share/theatre/addons/stage/plugin.cfg
ls ~/.local/share/theatre/addons/stage/bin/linux/libstage_godot.so
ls ~/.local/share/theatre/addons/director/plugin.cfg

# Init a test project (manual verification)
theatre init ~/godot/test-harness --yes
cat ~/godot/test-harness/.mcp.json
grep -A2 'editor_plugins' ~/godot/test-harness/project.godot
grep 'StageRuntime' ~/godot/test-harness/project.godot

# Enable/disable (manual verification)
theatre enable ~/godot/test-harness --disable --stage
grep -A2 'editor_plugins' ~/godot/test-harness/project.godot
theatre enable ~/godot/test-harness --stage
grep -A2 'editor_plugins' ~/godot/test-harness/project.godot

# Deploy after code change (manual verification)
theatre deploy ~/godot/test-harness --release
```

## Migration / Deprecation

| Old | New | Action |
|-----|-----|--------|
| `scripts/theatre-deploy` | `theatre deploy` | Add deprecation notice to script pointing to `theatre deploy` |
| `scripts/copy-gdext.sh` | Folded into `theatre install` / `theatre deploy` | Add deprecation notice |
| Manual `cp -r addons/` | `theatre init` | Update docs |
| Manual `.mcp.json` editing | `theatre init` generates it | Update docs |
| Manual plugin enabling in Godot UI | `theatre init` / `theatre enable` | Update docs |

Old scripts remain functional but print a one-line deprecation warning to
stderr when invoked. Remove them in a future release.
