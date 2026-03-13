# Test Quality Gap Analysis: Director

## Contract Sources
- `docs/director-spec.md` — Main Director specification (844 lines, 33 MCP tools)
- `docs/design/director-*.md` — Phase design documents (phases 4-10)
- `crates/director/src/mcp/*.rs` — MCP parameter structs (JsonSchema derives)
- `.claude/rules/contracts.md` — Wire format field naming rules

## Coverage Summary

| Category | Spec-Defined Scenarios | Tests Existed | Gaps Found |
|----------|----------------------|---------------|------------|
| Happy path | 45+ | 70+ | 3 |
| Invalid input | 20+ | 23 | 4 |
| Boundary values | 12+ | 12 | 4 |
| Error cases | 15+ | 8 | 5 |
| State transitions | 5 | 3 | 0 |
| Business rules | 10+ | 7 | 8 |

## Gaps

### Critical
1. **signal_connect with binds parameter**
   - Spec: director-spec.md — "binds = extra arguments to pass to the method"
   - Missing: No test verified binds are persisted in .tscn and returned by signal_list
   - Status: test added (`signal_connect_with_binds`)

2. **signal_connect with flags parameter**
   - Spec: director-spec.md — "CONNECT_DEFERRED=1, CONNECT_PERSIST=2 (auto-added), CONNECT_ONE_SHOT=4"
   - Missing: No test for flags bitmask, including auto-addition of CONNECT_PERSIST
   - Status: test added (`signal_connect_with_flags_deferred`)

3. **resource_duplicate with deep_copy=true**
   - Spec: resource.rs — "Deep copy sub-resources (making them independent)"
   - Missing: Only shallow copy tested; deep_copy path untested
   - Status: test added (`resource_duplicate_deep_copy`)

4. **shader_material_set_params tool**
   - Spec: director-spec.md — entire tool with no dedicated test
   - Missing: No test for setting shader uniforms on nodes
   - Status: test added (`shader_material_set_params_basic`) — verifies error case when no ShaderMaterial assigned

5. **node_find with property/property_value filter**
   - Spec: node.rs — "property: Filter by property existence", "property_value: property must equal this value"
   - Missing: These two params are accepted but never tested
   - Status: tests added (`node_find_by_property_exists`, `node_find_by_property_value`)

6. **scene_read with properties=false**
   - Spec: director-spec.md — "properties=false omits property data"
   - Missing: No test verified this flag works
   - Status: test added (`scene_read_properties_false_omits_properties`)

7. **scene_diff detects moved nodes**
   - Spec: director-spec.md — response includes "moved: [{node_path, old_parent, new_parent}]"
   - Missing: No test checked the `moved` array; only added/removed/changed tested
   - Status: test added (`scene_diff_detects_moved_node`)

8. **shape_create with both save_path AND scene attachment**
   - Spec: director-spec.md — "Can both save AND attach"
   - Missing: Individual outputs tested, but simultaneous save+attach not tested
   - Status: test added (`shape_create_save_and_attach`)

### High
1. **animation_create with negative length**
   - Spec: "length must be positive" — zero tested, negative not
   - Status: test added (`animation_create_rejects_negative_length`)

2. **signal_disconnect nonexistent connection**
   - Spec: "Requires exact match on source, signal, target, method"
   - Status: test added (`signal_disconnect_nonexistent_connection`)

3. **node_add to nonexistent parent path**
   - Spec: parent_path references a node in the scene tree
   - Status: test added (`node_add_nonexistent_parent_returns_error`)

4. **node_set_script with nonexistent script file**
   - Spec: "Script must exist on disk"
   - Status: test added (`node_set_script_nonexistent_script_returns_error`)

5. **visual_shader with duplicate node_ids in same function**
   - Spec: "IDs must be unique within a shader"
   - Status: test added (`visual_shader_rejects_duplicate_node_ids`)

6. **batch with unknown operation name**
   - Spec: operation names are validated against known Director tools
   - Status: test added (`batch_unknown_operation_errors`)

7. **scene_create overwriting existing file**
   - Spec: "Creates new .tscn" — overwrite behavior unspecified but important
   - Status: test added (`scene_create_overwrites_existing`)

8. **resource_read with depth=0**
   - Spec: "At depth 0, nested resources are returned as path strings"
   - Status: test added (`resource_read_depth_zero`)

9. **animation_add_track with interpolation=nearest**
   - Spec: "nearest/linear/cubic" — cubic tested, nearest boundary not
   - Status: test added (`animation_add_track_interpolation_nearest`)

10. **material_create with #hex color notation**
    - Spec: "Color from '#ff0000'" type conversion
    - Status: test added (`material_create_hex_color`)

11. **visual_shader for canvas_item mode**
    - Spec: "shader_mode: spatial/canvas_item/particles/sky/fog" — only spatial tested
    - Status: test added (`visual_shader_canvas_item_mode`)

12. **visual_shader for particles mode with start/process functions**
    - Spec: "particles shader_function: start/process/collide"
    - Status: test added (`visual_shader_particles_mode`)

13. **animation_create with pingpong loop mode**
    - Spec: "loop_mode: none/linear/pingpong" — pingpong untested
    - Status: test added (`animation_create_pingpong_loop`)

14. **node_find with ? wildcard**
    - Spec: "name_pattern supports * and ? wildcards" — * tested, ? not
    - Status: test added (`node_find_name_pattern_question_mark_wildcard`)

15. **scene_list with directory filter**
    - Spec: "directory — optional subdir filter"
    - Status: test added (`scene_list_with_directory_filter`)

### Medium (deferred)
1. **animation_add_track with scale_3d type** — position_3d and rotation_3d tested, scale_3d not
2. **animation_add_track with blend_shape type** — specialized track type untested
3. **visual_shader for sky and fog modes** — spatial/canvas_item/particles covered, sky/fog not
4. **export_mesh_library with specific items filter matching nodes** — exists but minimal assertion
5. **node_find with limit=1** — boundary: verifying limit truncation
6. **gridmap_set_cells with invalid orientation (>23)** — boundary: orientation range 0-23
7. **signal_connect with both binds AND flags simultaneously** — combined optional params

## Spec Violations Found
None confirmed — all tests compile cleanly. Actual pass/fail requires running with Godot binary (`cargo test --workspace` with `#[ignore]` tests enabled).

## Tests Added
- `tests/director-tests/src/test_gaps.rs` — 24 tests covering 8 Critical and 15 High priority gaps
- Tests registered in `tests/director-tests/src/lib.rs`
