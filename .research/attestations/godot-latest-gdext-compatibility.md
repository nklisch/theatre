---
source_handle: godot-latest-gdext-compatibility
fetched: 2026-09-04
source_title: godot-rust compatibility, migration, and v0.4.5 changelog
source_url: https://godot-rust.github.io/book/toolchain/compatibility.html
---

The fetched godot-rust book and tagged v0.4.5 changelog describe API/runtime compatibility and releases relevant to the current binding line. Each detail identifies the exact fetched source rather than treating this attestation's frontmatter URL as support for every claim.

## Attested details

1. The compatibility guide distinguishes a GDExtension API version (compile target) from the Godot runtime version, and says extensions from Godot 4.2 onward can load when runtime version is greater than or equal to API version. [Compatibility with Godot → Current guarantees](https://godot-rust.github.io/book/toolchain/compatibility.html#current-guarantees)
2. The compatibility guide's matrix says godot-rust `0.4+` has minimum Godot `4.2`; its out-of-scope section excludes non-stable Godot releases from maintained compatibility. [Compatibility matrix](https://godot-rust.github.io/book/toolchain/compatibility.html#compatibility-matrix); [Out of scope](https://godot-rust.github.io/book/toolchain/compatibility.html#out-of-scope)
3. The v0.5 migration guide says v0.5 makes API level 4.6 the default but supports an older API target through features such as `api-4-5`; it also says API-level features are minor-version-only. [Godot versions](https://godot-rust.github.io/book/migrate/v0.5.html#godot-versions)
4. The tagged v0.4.5 changelog records v0.4.5 as a 12 December 2025 hotfix for a Rust compiler breaking change. [v0.4.5](https://github.com/godot-rust/gdext/blob/v0.4.5/Changelog.md#v045)
5. The tagged v0.4.5 changelog's v0.4.0 section lists Godot 4.5 API level support. [v0.4.0 → Features](https://github.com/godot-rust/gdext/blob/v0.4.5/Changelog.md#v040)
6. The migration guide says moving from godot-rust 0.4 to 0.5 has breaking changes, including a Rust 2024 edition requirement, a 1.94 MSRV, and API/type changes such as required objects becoming non-optional in many engine APIs. [Migrating to v0.5](https://godot-rust.github.io/book/migrate/v0.5.html#migrating-to-v05); [Rust 2024 edition](https://godot-rust.github.io/book/migrate/v0.5.html#rust-2024-edition); [Required objects in engine APIs](https://godot-rust.github.io/book/migrate/v0.5.html#required-objects-in-engine-apis)
