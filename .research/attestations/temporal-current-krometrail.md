---
source_handle: temporal-current-krometrail
fetched: 2026-09-04
source_title: Krometrail repository at observed origin/HEAD commit eb5b465618fcaeb232c2038d8c6cf960ff499b99
source_url: https://github.com/nklisch/krometrail/tree/eb5b465618fcaeb232c2038d8c6cf960ff499b99
---

Krometrail `origin`'s advertised `HEAD` matched local commit
`eb5b465618fcaeb232c2038d8c6cf960ff499b99` when fetched. The
`crates/temporal-vision` subtree had no local diff from that commit. These
attested details describe the current sibling source, not an assertion that
this commit is the exact crates.io release tarball.

## Attested details

1. The Krometrail workspace includes `crates/temporal-vision`, and its workspace repository is `https://github.com/nklisch/krometrail`. [Cargo.toml#L9-L42](https://github.com/nklisch/krometrail/blob/eb5b465618fcaeb232c2038d8c6cf960ff499b99/Cargo.toml#L9-L42)
2. The sibling crate is named `temporal-vision`, describes storyboards, difference maps, motion-history images, and region filmstrips, and declares version `0.1.1`. [crates/temporal-vision/Cargo.toml#L1-L18](https://github.com/nklisch/krometrail/blob/eb5b465618fcaeb232c2038d8c6cf960ff499b99/crates/temporal-vision/Cargo.toml#L1-L18)
3. Its public crate surface exports generators for storyboards, motion histories, difference maps, and tracked-region filmstrips, together with the frame-sequence and associated parameter types. [crates/temporal-vision/src/lib.rs#L126-L170](https://github.com/nklisch/krometrail/blob/eb5b465618fcaeb232c2038d8c6cf960ff499b99/crates/temporal-vision/src/lib.rs#L126-L170)
4. The crate README describes RGBA frame sequences with timestamps, optional markers, and declared gaps as inputs, and specifies storyboard, difference-map, motion-history, and tracked-region-filmstrip outputs. [crates/temporal-vision/README.md#L12-L35](https://github.com/nklisch/krometrail/blob/eb5b465618fcaeb232c2038d8c6cf960ff499b99/crates/temporal-vision/README.md#L12-L35)
5. The same README names both Krometrail and Theatre/Stage as current MCP consumers, characterizing Stage's use as clip storyboards, motion history, and node-following filmstrips. [crates/temporal-vision/README.md#L37-L66](https://github.com/nklisch/krometrail/blob/eb5b465618fcaeb232c2038d8c6cf960ff499b99/crates/temporal-vision/README.md#L37-L66)
6. The README states that the crate is pure Rust and synchronous, and records explicit bounded processing/render/output properties and plan/render separation. [crates/temporal-vision/README.md#L8-L10](https://github.com/nklisch/krometrail/blob/eb5b465618fcaeb232c2038d8c6cf960ff499b99/crates/temporal-vision/README.md#L8-L10) [crates/temporal-vision/README.md#L68-L79](https://github.com/nklisch/krometrail/blob/eb5b465618fcaeb232c2038d8c6cf960ff499b99/crates/temporal-vision/README.md#L68-L79)
