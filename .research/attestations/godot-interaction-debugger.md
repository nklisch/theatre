---
source_handle: godot-interaction-debugger
fetched: 2026-09-04
source_title: Godot Engine latest documentation — EditorDebuggerPlugin
source_url: https://docs.godotengine.org/en/latest/classes/class_editordebuggerplugin.html
---

Official Godot documentation fetched 2026-09-04. This page explicitly labels
itself `latest` and unstable, so it is evidence for the editor/runtime message
boundary rather than a version-unqualified compatibility promise.

## Attested details

1. `EditorDebuggerPlugin` is an editor-side debugger API. It must be added via
   `EditorPlugin.add_debugger_plugin()`, after which `_setup_session()` runs
   for debugger sessions. Source: `class_editordebuggerplugin.html#description`.
2. The official example connects the runtime side with `EngineDebugger`:
   runtime code registers a message capture and sends messages, while the
   editor plugin captures prefixed messages and can send a response through an
   `EditorDebuggerSession`. Source: `class_editordebuggerplugin.html#description`.
3. `_has_capture()` determines whether messages with a matching prefix are
   delivered to `_capture()`, which receives the message, data array, and
   session ID. Source:
   `class_editordebuggerplugin.html#class-editordebuggerplugin-private-method-has-capture`
   and `#class-editordebuggerplugin-private-method-capture`.
4. The documentation directs the reader to check stable documentation because
   this `latest` page may not be compatible with released Godot versions.
   Source: page warning before the class description.
