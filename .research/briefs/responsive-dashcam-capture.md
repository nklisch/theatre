---
id: responsive-dashcam-capture
kind: research-brief
summary: Compatibility GPU downsampling is public; delayed native readback is feasible but its integration and performance remain unverified.
updated: 2026-09-05
source_handles: [godot-4-7-1-capture-interop, khronos-pixel-readback-context]
relationships: []
---

# Responsive dashcam capture

## Decision boundary

Support delayed visual evidence without delaying controls in Voxlar's requested
Godot 4.7.1 Compatibility/OpenGL workflow. Reuse the existing rendered viewport,
not a second scene render. This brief informs backend design; it does not approve
an implementation, change the consumer renderer or cover save persistence.
Godot evidence is pinned to `a13da4feb8d8aefc283c3763d33a2f170a18d541`.

## Findings

- **Public GPU downsampling is implemented on Compatibility.** A sized drawable
  texture can receive the existing viewport render texture through
  `RenderingServer.texture_drawable_blit_rect`. The GLES3 implementation binds
  source textures and draws a quad into the destination-sized framebuffer.
  Default blending is mixing, so exact replacement and color/filter behavior
  require deliberate selection. [godot-4-7-1-capture-interop]{2}
  [godot-4-7-1-capture-interop]{3}
- **Ordinary readback remains synchronous.** Desktop `texture_2d_get` reads into
  CPU memory with the pixel-pack buffer explicitly unbound. Downsizing first
  reduces bytes but does not remove this synchronization.
  [godot-4-7-1-capture-interop]{1}
- **RenderingDevice is not a Compatibility escape hatch.** Both device getters
  return null with OpenGL; the Compatibility RD texture accessor is empty. The
  RD asynchronous method belongs to a different renderer capability boundary.
  [godot-4-7-1-capture-interop]{4}
- **Native asynchronous readback is feasible, not yet verified integration.**
  Inference: combine Godot's native texture handle and render-thread dispatch
  with a bounded ring of OpenGL pixel-pack buffers and fences. Submit the small
  texture's transfer, check completion on later frames with zero timeout, then
  map/copy only completed data into owned bytes for JPEG encoding. Skip before
  GPU work when capacity is exhausted. Do not use unsynchronized read mapping.
  [godot-4-7-1-capture-interop]{3} [godot-4-7-1-capture-interop]{5}
  [khronos-pixel-readback-context]{1} [khronos-pixel-readback-context]{2}
  [khronos-pixel-readback-context]{3}

## Native integration constraints

These are **design implications**, not an existing Godot-managed capture API:

1. **Load functions for the actual graphics implementation.** A `GLuint` is not
   a GL function table or context. Do not assume Godot's private GLAD globals are
   exported, or hard-code a GLX/libGL loader merely because the host is Linux.
   Godot itself uses EGL resolution where available and distinguishes desktop
   GL and GLES. The extension needs a context-compatible loader with retained
   library lifetime and checked required entry points. This research has not
   selected or verified a Rust loader or the active window-system backend.
   [godot-4-7-1-capture-interop]{6} [khronos-pixel-readback-context]{5}
2. **Execute native GPU operations in Godot's rendering context.** Route work
   through render-thread dispatch; establish that the engine context is current
   there before using native handles. Do not steal/rebind the engine context
   onto the JPEG worker or create a speculative shared context. GLX explicitly
   restricts cross-thread current-context reuse. Dispatch may execute inline;
   it is not itself a background worker or GPU completion boundary.
   [godot-4-7-1-capture-interop]{5} [khronos-pixel-readback-context]{4}
3. **Keep ownership and state bounded.** Main-thread scene objects must not be
   accessed from the native callback. Transfer only owned bytes and metadata to
   workers. Keep buffer/fence operations and teardown on the owning context;
   reacquire source handles across resize/recreation. Save and restore every GL
   binding/pixel-pack state the native path changes, rather than assuming zero
   is the engine's prior state. These are integration obligations inferred from
   context-bound commands and buffer constraints, not a proven gdext callable
   implementation. [godot-4-7-1-capture-interop]{3}
   [khronos-pixel-readback-context]{1} [khronos-pixel-readback-context]{4}
4. **Never wait for visual evidence in the control path.** Bound admission,
   submit without an intentional completion wait, and ensure submitted commands
   progress without using `glFinish`. Poll fences without blocking; incomplete
   slots remain pending and new samples may be skipped. Preserve source-capture
   provenance separately from completion time. No cited API promises bounded
   driver-call duration or fast mapped-memory reads.
   [khronos-pixel-readback-context]{2} [khronos-pixel-readback-context]{3}

## Recommendation and alternatives

**Recommendation (inference):** use public GPU blitting plus one narrowly scoped
native GL readback path if genuinely delayed image delivery is required. Keep
spatial-only capture available when the graphics path cannot operate, and apply
skip-before-work independently. Do not silently substitute synchronous readback
under a nonblocking capture promise. A smaller synchronous readback is a useful
intermediate measurement, not fulfillment of asynchronous capture.

Changing Voxlar's renderer to obtain RD support or shipping a custom Godot build
would increase the qualification and distribution boundary. Neither is justified
by this source evidence. Public blitting also removes the need to add a separate
2D SubViewport solely for downsampling.

## Disconfirming evidence

The public drawable-blit implementation disproves the stronger claim that
Compatibility requires private engine APIs or a second scene render to downsample.
Conversely, native texture-handle exposure disproves an absolute claim that
asynchronous Compatibility capture is impossible. These qualify, rather than
contradict, the absence of a managed async readback method in the examined path.
[godot-4-7-1-capture-interop]{1} [godot-4-7-1-capture-interop]{2}
[godot-4-7-1-capture-interop]{3} [godot-4-7-1-capture-interop]{4}

Fence completion does not prove smooth gameplay: submission and memory access
still have costs. Khronos warns that mapped reads can be slow. No runtime,
loader, color fidelity or control-latency experiment was performed, and no
performance threshold is established. [khronos-pixel-readback-context]{3}

## Verification

Adaptive rigor resolves to **standard** for this backend decision. An inline
source-support pass checked the load-bearing claims against the pinned passages:
public downsampling and RD exclusion are supported; end-to-end native readback
is a supported composition with explicitly unverified integration; performance
is unverified. This pass was not independent.

On 2026-09-05, Workbench 0.19.0 `scripts/lint-research.py` against the project
returned `Research lint passed: 0 warning(s)`. A read-only comparison against
`build-knowledge-index.py`'s canonical bibliography formatter confirmed that
`bibliography.yaml` matches generated attestation metadata. The loaded plugin
and project Workbench versions both equal 0.19.0. Index generation/check and
final integration remain the parent's responsibility; no runtime or performance
verification is claimed.
