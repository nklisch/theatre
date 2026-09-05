---
source_handle: temporal-current-theatre
fetched: 2026-09-04
source_title: Theatre repository at observed origin/HEAD commit 2486f021bad0c81efab5a04f982614b5bc81938e
source_url: https://github.com/nklisch/theatre/tree/2486f021bad0c81efab5a04f982614b5bc81938e
---

The Theatre `origin` remote's advertised `HEAD` matched local commit
`2486f021bad0c81efab5a04f982614b5bc81938e` when fetched. The inspected
Cargo and Stage visual-artifact files had no local diff from that commit, so
all links below refer to committed source rather than working-tree state.

## Attested details

1. `stage-server` declares a registry dependency on `temporal-vision = "0.1"` in its package manifest. [crates/stage-server/Cargo.toml#L11-L29](https://github.com/nklisch/theatre/blob/2486f021bad0c81efab5a04f982614b5bc81938e/crates/stage-server/Cargo.toml#L11-L29)
2. The resolved `Cargo.lock` package is `temporal-vision` version `0.1.1` from the crates.io registry, with checksum `44b8761eae45e4fffb25832fcb4b8cac7717b0c8934bdea2ac36777411f39993`. [Cargo.lock package entry](https://github.com/nklisch/theatre/blob/2486f021bad0c81efab5a04f982614b5bc81938e/Cargo.lock#L1717-L1728)
3. `clip_artifacts.rs` imports `temporal_vision` frame, normalization, storyboard, motion-history, difference-map, and tracked-filmstrip APIs; its artifact enum exposes `storyboard`, `motion_history`, `difference_map`, and `node_filmstrip`. [crates/stage-server/src/clip_artifacts.rs#L1-L52](https://github.com/nklisch/theatre/blob/2486f021bad0c81efab5a04f982614b5bc81938e/crates/stage-server/src/clip_artifacts.rs#L1-L52)
4. The generator decodes saved JPEG screenshots into RGBA frames, obtains markers and screenshot gaps from the opened clip database, and constructs a `temporal_vision::FrameSequence`; it is therefore an analysis path over retained clip data. [crates/stage-server/src/clip_artifacts.rs#L340-L448](https://github.com/nklisch/theatre/blob/2486f021bad0c81efab5a04f982614b5bc81938e/crates/stage-server/src/clip_artifacts.rs#L340-L448)
5. Theatre calls `generate_storyboard`, `build_motion_history_plan`/`generate_motion_history`, `generate_tracked_region_filmstrip`, and `render_difference_map` for the four supported artifact kinds. [crates/stage-server/src/clip_artifacts.rs#L459-L710](https://github.com/nklisch/theatre/blob/2486f021bad0c81efab5a04f982614b5bc81938e/crates/stage-server/src/clip_artifacts.rs#L459-L710)
6. The sole Stage clip action surface enumerates saving and querying clips, screenshots, and `VisualArtifact`; it has no action for a live visual-artifact capture or for consuming a human click stream. This is bounded to the declared `ClipAction` surface, not a claim about all future or internal capabilities. [crates/stage-server/src/mcp/clips.rs#L21-L55](https://github.com/nklisch/theatre/blob/2486f021bad0c81efab5a04f982614b5bc81938e/crates/stage-server/src/mcp/clips.rs#L21-L55)
7. `visual_artifact` requires a requested artifact, validates filmstrip-specific inputs and tile bounds, generates from the selected `clip_id`, and returns a text manifest plus an optional PNG image content block. [crates/stage-server/src/mcp/clips.rs#L537-L620](https://github.com/nklisch/theatre/blob/2486f021bad0c81efab5a04f982614b5bc81938e/crates/stage-server/src/mcp/clips.rs#L537-L620)
8. A Godot-dependent ignored journey test covers a saved clip's storyboard response, including both the no-screenshot degradation and rendered manifest/image path, and checks text-only motion-history output. [crates/stage-server/tests/e2e_journeys.rs#L1086-L1143](https://github.com/nklisch/theatre/blob/2486f021bad0c81efab5a04f982614b5bc81938e/crates/stage-server/tests/e2e_journeys.rs#L1086-L1143)
9. Ordinary fixture tests exercise node-filmstrip projection, cache storage/hits, deterministic PNG output, and selected degradation results. [crates/stage-server/tests/node_filmstrip.rs#L210-L300](https://github.com/nklisch/theatre/blob/2486f021bad0c81efab5a04f982614b5bc81938e/crates/stage-server/tests/node_filmstrip.rs#L210-L300)
