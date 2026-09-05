---
id: gdext-render-callables-with-lazy-tables
tags: [stage, gdext, rendering]
created: 2026-09-05
updated: 2026-09-05
---

# Revisit high-level render callables with lazy binding tables

Godot's public rendering-thread dispatch cannot currently use Theatre's gdext 0.5.5 high-level callable return conversion safely under the existing lazy-table configuration. This is a reproduced binding limitation, not evidence that Godot lacks rendering-thread dispatch or asynchronous native readback.

During responsive-dashcam development, a plain-data callback created with `Callable::from_sync_fn` was submitted through `RenderingServer.call_on_render_thread`. With Godot 4.7.1 Compatibility and `--render-thread separate`, the capture poll and retirement callbacks produced:

```text
[panic godot-ffi-0.5.5/src/binding/single_threaded.rs:140]
attempted to access binding from different thread than main thread; this is UB - use the "experimental-threads" feature.
in <Callable>::stage_readback_poll()
in <Callable>::stage_readback_retire()
```

Expected: a callback containing only native graphics/owned-data work can return nil without consulting main-thread-only binding tables. Observed: high-level callable return conversion reaches those tables. Enabling `experimental-threads` together with `lazy-function-tables` also failed to compile, including `BuiltinMethodTable`/`RefCell` and `StringCache` Send/Sync errors. No upstream defect disposition is claimed.

The current local workaround is the narrow public C-interface adapter in `crates/stage-godot/src/capture_render_call.rs`: create the callable and cache the nil initializer on the main thread; execute only owned native work and initialize local return storage in the render callback. Scene objects still do not cross threads. It does not disable gdext's general safety checks.

The regression journey is `native_readback_retires_on_separate_render_thread` in `crates/stage-server/tests/native_readback_engine.rs`. It now passes with the adapter. Revisit whether a later binding release permits removing that adapter without dropping lazy lookup or thread safety; a binding upgrade or upstream change is not part of the current capture repair.
