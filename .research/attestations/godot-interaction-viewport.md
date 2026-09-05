---
source_handle: godot-interaction-viewport
fetched: 2026-09-04
source_title: Godot Engine latest documentation — Viewport and RenderingServer
source_url: https://docs.godotengine.org/en/latest/classes/class_viewport.html
---

Official Godot documentation fetched 2026-09-04. This source is `latest`,
which the documentation labels unstable; stable availability must be verified
against the project's minimum supported Godot release before implementation.

## Attested details

1. A `Viewport` encapsulates drawing and interaction with a game world. It can
   expose its viewport-local mouse position through `get_mouse_position()`,
   and exposes transforms between viewport, embedding, and screen coordinate
   systems through `get_final_transform()` and `get_screen_transform()`.
   Source: `class_viewport.html#description` and
   `#class-viewport-method-get-mouse-position` / `#class-viewport-method-get-final-transform` /
   `#class-viewport-method-get-screen-transform`.
2. `Viewport.get_texture()` returns its texture. The documentation warns the
   texture may be black or outdated when read too early and recommends waiting
   for `RenderingServer.frame_post_draw` before converting it to an image.
   Source: `class_viewport.html#class-viewport-method-get-texture`.
3. `RenderingServer.frame_post_draw` is emitted at the end of a frame after
   RenderingServer has updated all viewports. Source:
   `class_renderingserver.html#class-renderingserver-signal-frame-post-draw`.
4. `Viewport.push_input()` is an injection/replay mechanism: it sends an
   `InputEvent` through child-node input propagation in a defined order and
   can cause physics object picking if no handler consumes it. Source:
   `class_viewport.html#class-viewport-method-push-input`.
5. Built-in 2D physics object picking is disabled by default. When enabled,
   simultaneous pickable objects are limited to 64 and selection order can be
   non-deterministic; sorting may cost performance and still does not guarantee
   the highest-z object because the 64-object limit is applied first. Source:
   `class_viewport.html#class-viewport-property-physics-object-picking` and
   `#class-viewport-property-physics-object-picking-sort`.
6. `RenderingServer` returns dummy values in headless mode, because rendering
   and window management are disabled. Source:
   `class_renderingserver.html#description`.
