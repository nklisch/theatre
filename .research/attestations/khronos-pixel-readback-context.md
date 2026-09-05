---
source_handle: khronos-pixel-readback-context
fetched: 2026-09-05
source_title: Khronos OpenGL pixel readback, synchronization and GLX context reference pages
source_url: https://github.com/KhronosGroup/OpenGL-Refpages/tree/121f1891c688e0c75ad46ed83d9cb73712fbbbfc
---

# OpenGL readback and context semantics

Reference-page sources fetched at commit
`121f1891c688e0c75ad46ed83d9cb73712fbbbfc`. GLX details describe that window-system
interface; they do not establish the active Godot display backend or an EGL,
Windows or macOS implementation.

## Attested details

1. With a pixel-pack buffer bound, `glReadPixels` treats the data argument as a byte offset into that buffer instead of a CPU pointer. Buffer capacity, mapping state and pixel format impose validity constraints. [Readback destination](https://github.com/KhronosGroup/OpenGL-Refpages/blob/121f1891c688e0c75ad46ed83d9cb73712fbbbfc/gl4/glReadPixels.xml#L185-L190), [buffer constraints](https://github.com/KhronosGroup/OpenGL-Refpages/blob/121f1891c688e0c75ad46ed83d9cb73712fbbbfc/gl4/glReadPixels.xml#L1185-L1207).
2. A fence using `GL_SYNC_GPU_COMMANDS_COMPLETE` is signaled after preceding commands in the same stream have completed and their effects are realized. `glClientWaitSync` waits for up to the supplied nanosecond timeout and distinguishes signaled, timeout and failure outcomes. [Fence](https://github.com/KhronosGroup/OpenGL-Refpages/blob/121f1891c688e0c75ad46ed83d9cb73712fbbbfc/gl4/glFenceSync.xml#L67-L71), [wait semantics](https://github.com/KhronosGroup/OpenGL-Refpages/blob/121f1891c688e0c75ad46ed83d9cb73712fbbbfc/gl4/glClientWaitSync.xml#L58-L90).
3. Buffer mapping can synchronize pending operations. `GL_MAP_UNSYNCHRONIZED_BIT` suppresses synchronization but overlapping pending operations have undefined results; it cannot be combined with `GL_MAP_READ_BIT`. Mapped storage may have nonstandard performance, including slow reads. [Unsynchronized mapping](https://github.com/KhronosGroup/OpenGL-Refpages/blob/121f1891c688e0c75ad46ed83d9cb73712fbbbfc/gl4/glMapBufferRange.xml#L218-L229), [performance](https://github.com/KhronosGroup/OpenGL-Refpages/blob/121f1891c688e0c75ad46ed83d9cb73712fbbbfc/gl4/glMapBufferRange.xml#L253-L262), [invalid flag combinations](https://github.com/KhronosGroup/OpenGL-Refpages/blob/121f1891c688e0c75ad46ed83d9cb73712fbbbfc/gl4/glMapBufferRange.xml#L332-L341).
4. GLX makes a rendering context current to the calling thread; there is only one current context per thread. Making a context current while it is current on another thread causes `BadAccess`. [Current context](https://github.com/KhronosGroup/OpenGL-Refpages/blob/121f1891c688e0c75ad46ed83d9cb73712fbbbfc/gl2.1/glXMakeCurrent.xml#L60-L75), [other-thread restriction](https://github.com/KhronosGroup/OpenGL-Refpages/blob/121f1891c688e0c75ad46ed83d9cb73712fbbbfc/gl2.1/glXMakeCurrent.xml#L118-L120).
5. `glXGetProcAddress` returns null for a requested function unsupported by the queried implementation. Function lookup is distinct from making a context current. [Procedure lookup](https://github.com/KhronosGroup/OpenGL-Refpages/blob/121f1891c688e0c75ad46ed83d9cb73712fbbbfc/gl2.1/glXGetProcAddress.xml#L40-L55), [context operation](https://github.com/KhronosGroup/OpenGL-Refpages/blob/121f1891c688e0c75ad46ed83d9cb73712fbbbfc/gl2.1/glXMakeCurrent.xml#L60-L75).
