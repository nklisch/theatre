# Director Tool Reference

Director's Rust parameter structs and generated schemas own the current tool
catalog. Do not maintain a second parameter list in this skill.

Use the MCP client's tool discovery when Director is connected. In a source
checkout, consult:

- `crates/director/src/mcp/` for parameter and response types.
- `site/api/director.md` for the generated public reference.
- `director --help` for the installed CLI surface.

Every Director operation identifies a Godot project with `project_path`.
Structural scene and resource mutations use Godot serialization. Open-scene
mutations use native undo and remain unsaved until `scene_save`; detached
headless mutations persist their target files. Batch operations run sequentially
and preserve partial results without rollback.

Use `engine_api` for focused installed-engine metadata rather than guessing a
Godot member shape. Use `editor_run` only with a verified open editor, then use
Stage `runtime_status` to establish runtime readiness. The `feedback` family
reads the project-local queue without launching Godot, and retrieval never
handles or deletes evidence.
