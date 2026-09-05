---
source_handle: godot-local-api-probe
fetched: 2026-09-04
source_title: Installed Godot version and temporary native editor probes
---

The source was the installed Godot executable and its ClassDB output during this
engagement, not a public document. Temporary projects used isolated configuration,
data and cache directories and were removed after the probes. No project source
or user scene was changed by these probes.

## Attested details

1. Running `godot --version` reported `4.7.1.stable.official.a13da4feb`.
2. A temporary headless GDScript ClassDB query reported EditorInterface methods
   `get_open_scene_roots`, `get_unsaved_scenes`, `get_editor_undo_redo`, `save_scene`,
   `mark_scene_as_unsaved`, `play_custom_scene`, `stop_playing_scene`,
   `is_playing_scene`, and `get_playing_scene`.
3. The ClassDB query reported `class_get_property_default_value`, property and
   method lists, signal lists, enum lists/constants, and integer-constant lookup.
4. A temporary real EditorPlugin renamed an open root without saving, marked it
   unsaved, disabled `run/auto_save/save_before_running`, synchronously invoked
   `play_custom_scene`, and restored the original setting. Subsequent output
   confirmed the game was running, the original saved root name remained on disk,
   and the changed root name remained in the editor. The probe stopped the game
   and exited successfully. This establishes the observed installed-engine
   behavior, not compatibility with every engine release or third-party plugin.
