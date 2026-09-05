---
source_handle: godot-interaction-input
fetched: 2026-09-04
source_title: Godot Engine stable documentation — InputEventMouseButton and Node input propagation
source_url: https://docs.godotengine.org/en/stable/classes/class_inputeventmousebutton.html
---

Official Godot documentation fetched 2026-09-04. The stable class reference
and input propagation documentation were reviewed for the runtime observation
surface.

## Attested details

1. `InputEventMouseButton` represents mouse-button press and release events;
   it exposes `button_index`, `pressed`, `double_click`, `canceled`, and
   `factor`. The class directs users to `Node._input()` and the mouse/input
   coordinate tutorial. Source: `class_inputeventmousebutton.html#description`
   and `#property-descriptions`.
2. Node input propagation calls `_input()` for each input event before GUI and
   unhandled-input stages. An event can be marked handled, which stops later
   propagation. Source: `class_node.html#class-node-private-method-input`.
3. `_shortcut_input()` is an input-propagation hook intended for shortcuts and
   runs after GUI handling but before unhandled-key/input hooks. Source:
   `class_node.html#class-node-private-method-shortcut-input`.
4. The `latest` documentation describes the same input model but is explicitly
   unstable and may document APIs unavailable in released stable Godot.
   Source: `https://docs.godotengine.org/en/latest/classes/class_editordebuggerplugin.html`.
