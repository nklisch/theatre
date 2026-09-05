---
source_handle: godot-latest-lazy-function-tables
fetched: 2026-09-04
source_title: godot crate feature documentation
source_url: https://docs.rs/godot/latest/godot/index.html
---

The fetched `godot` crate documentation describes the `lazy-function-tables` feature.

## Attested details

1. The `lazy-function-tables` feature loads engine function pointers on first use rather than at startup; the documentation says this reduces startup time and RAM use but adds overhead to each FFI call. [Cargo features → lazy-function-tables]
2. The same feature documentation says lazy loading removes the guarantee that all function pointers are available once the library has booted, so calls can panic at runtime; it also says the feature is not thread-safe and cannot be combined with `experimental-threads`. [Cargo features → lazy-function-tables]
