---
source_handle: godot-latest-unsaved-scenes-version
fetched: 2026-09-04
source_title: EditorInterface declarations across Godot 4.5, 4.6, and 4.7 stable tags
source_url: https://raw.githubusercontent.com/godotengine/godot/4.7-stable/editor/editor_interface.h
---

The fetched EditorInterface headers make a bounded version comparison for `get_unsaved_scenes()`.

## Attested details

1. The Godot `4.5-stable` `EditorInterface` declaration includes `get_open_scenes()`, `get_open_scene_roots()`, and `mark_scene_as_unsaved()`, but does not declare `get_unsaved_scenes()`. [EditorInterface object/resource/node editing](https://raw.githubusercontent.com/godotengine/godot/4.5-stable/editor/editor_interface.h)
2. The Godot `4.6-stable` `EditorInterface` declaration likewise does not declare `get_unsaved_scenes()`. [EditorInterface object/resource/node editing](https://raw.githubusercontent.com/godotengine/godot/4.6-stable/editor/editor_interface.h)
3. The Godot `4.7-stable` `EditorInterface` declaration includes `PackedStringArray get_unsaved_scenes() const;` alongside the open-scene methods. [EditorInterface object/resource/node editing](https://raw.githubusercontent.com/godotengine/godot/4.7-stable/editor/editor_interface.h)
4. The stable class reference documents `get_unsaved_scenes()` as returning file paths of currently unsaved scenes. [get_unsaved_scenes](https://docs.godotengine.org/en/stable/classes/class_editorinterface.html#class-editorinterface-method-get-unsaved-scenes)
