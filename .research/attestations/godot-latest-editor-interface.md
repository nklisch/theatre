---
source_handle: godot-latest-editor-interface
fetched: 2026-09-04
source_title: EditorInterface — Godot Engine 4.7 documentation
source_url: https://docs.godotengine.org/en/stable/classes/class_editorinterface.html
---

The fetched stable EditorInterface class reference describes editor-scene, undo, save, and playback methods. Its footer identifies the stable documentation as Godot Engine 4.7.

## Attested details

1. `#class-editorinterface-method-get-open-scenes` says `get_open_scenes()` returns file paths of the currently opened scenes; `#class-editorinterface-method-get-open-scene-roots` says `get_open_scene_roots()` returns references to their root nodes; and `#class-editorinterface-method-get-edited-scene-root` returns the current edited scene root. [Method descriptions: get_open_scenes, get_open_scene_roots, get_edited_scene_root]
2. `#class-editorinterface-method-get-unsaved-scenes` says `get_unsaved_scenes()` returns file paths of currently unsaved scenes. `#class-editorinterface-method-mark-scene-as-unsaved` marks the current scene tab unsaved. [Method descriptions: get_unsaved_scenes, mark_scene_as_unsaved]
3. `#class-editorinterface-method-get-editor-undo-redo` returns the editor's `EditorUndoRedoManager`. [Method descriptions: get_editor_undo_redo]
4. `#class-editorinterface-method-save-scene` saves the active scene and returns `OK` or `ERR_CANT_CREATE`; `#class-editorinterface-method-save-scene-as` saves it at a supplied path; and `#class-editorinterface-method-save-all-scenes` saves all open editor scenes. [Method descriptions: save_scene, save_scene_as, save_all_scenes]
5. `#class-editorinterface-method-play-main-scene`, `#class-editorinterface-method-play-current-scene`, and `#class-editorinterface-method-play-custom-scene` play the main, active, and supplied-path scenes respectively. `#class-editorinterface-method-stop-playing-scene` stops the playing scene; `#class-editorinterface-method-is-playing-scene` reports whether one is playing, including paused scenes; and `#class-editorinterface-method-get-playing-scene` returns an empty string when none is playing. [Method descriptions: play_*, stop_playing_scene, is_playing_scene, get_playing_scene]
6. `#class-editorinterface-method-reload-scene-from-path` says reloading fails when the specified scene is not open. [Method descriptions: reload_scene_from_path]
