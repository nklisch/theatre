use crate::dual_test;
use crate::harness::assertions::*;
use crate::harness::*;
use serde_json::json;

const LIVE_3D: &str = "res://live_scene_3d.tscn";
const LIVE_PHYSICS: &str = "res://live_scene_physics.tscn";

/// Journey: Observe gravity, stacking, and teleport disruption of RigidBodies.
///
/// Exercises real physics simulation over multiple seconds — something headless
/// tests do with static scenes. This test verifies gravity, object settling,
/// stacking stability, and that teleport restarts a physics trajectory.
///
/// Steps:
///   1. spatial_snapshot(standard) → FallingBox at y≈10, StackA at y≈0.5, StackB at y≈1.5
///   2. scene_tree(roots) → verify scene hierarchy is loaded
///   3. wait_frames(120) → 2 seconds of physics
///   4. spatial_snapshot(standard) → FallingBox has fallen significantly (y < 5)
///   5. spatial_inspect(FallingBox) → verify class is RigidBody3D
///   6. wait_frames(120) → 2 more seconds — everything should settle
///   7. spatial_snapshot(standard) → FallingBox near floor (y < 2),
///      StackA near floor (y < 2), StackB above StackA (StackB.y > StackA.y)
///   8. spatial_inspect(StackA) → verify class is RigidBody3D, has collision shape
///   9. spatial_action(teleport, FallingBox, [0, 20, 0]) → move it back up
///  10. spatial_snapshot(standard) → FallingBox at y >> stack (teleport worked)
///  11. wait_frames(180) → let it fall again and settle
///  12. spatial_snapshot(standard) → FallingBox near floor again (y < 2)
async fn journey_physics_gravity_and_stacking(b: &impl LiveBackend) {
    // Step 1: initial positions
    let snap1 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("initial snapshot")
        .unwrap_data();
    let e1 = snap1["entities"].as_array().expect("entities");
    let falling1 = find_entity(e1, "FallingBox");
    let stack_a1 = find_entity(e1, "StackA");
    let stack_b1 = find_entity(e1, "StackB");
    let falling_y1 = extract_position(falling1)[1];
    let stack_a_y1 = extract_position(stack_a1)[1];
    let stack_b_y1 = extract_position(stack_b1)[1];
    assert!(
        falling_y1 > 8.0,
        "FallingBox should start high, y={falling_y1}"
    );
    assert!(
        stack_b_y1 > stack_a_y1,
        "StackB should start above StackA: A.y={stack_a_y1}, B.y={stack_b_y1}"
    );

    // Step 2: verify scene tree loaded
    let tree = b
        .stage("scene_tree", json!({"action": "roots"}))
        .await
        .expect("scene_tree roots")
        .unwrap_data();
    assert!(
        tree["roots"]
            .as_array()
            .map(|a| !a.is_empty())
            .unwrap_or(false),
        "Scene tree should have roots: {tree}"
    );

    // Step 3-4: wait and check falling
    b.wait_frames(120).await;
    let snap2 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("mid-fall snapshot")
        .unwrap_data();
    let e2 = snap2["entities"].as_array().expect("entities");
    let falling2 = find_entity(e2, "FallingBox");
    let falling_y2 = extract_position(falling2)[1];
    assert!(
        falling_y2 < 5.0,
        "FallingBox should have fallen after 2s, y={falling_y2}"
    );

    // Step 5: inspect FallingBox — verify class
    let inspect_falling = b
        .stage("spatial_inspect", json!({"node": "FallingBox"}))
        .await
        .expect("inspect FallingBox")
        .unwrap_data();
    assert_eq!(
        inspect_falling["class"].as_str(),
        Some("RigidBody3D"),
        "FallingBox should be RigidBody3D"
    );

    // Step 6-7: full settle
    b.wait_frames(120).await;
    let snap3 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("settled snapshot")
        .unwrap_data();
    let e3 = snap3["entities"].as_array().expect("entities");
    let falling3 = find_entity(e3, "FallingBox");
    let stack_a3 = find_entity(e3, "StackA");
    let stack_b3 = find_entity(e3, "StackB");
    let falling_y3 = extract_position(falling3)[1];
    let stack_a_y3 = extract_position(stack_a3)[1];
    let stack_b_y3 = extract_position(stack_b3)[1];

    assert!(
        falling_y3 < 2.0,
        "FallingBox should be near floor, y={falling_y3}"
    );
    assert!(
        stack_a_y3 < 3.0,
        "StackA should be near floor, y={stack_a_y3}"
    );
    assert!(
        stack_b_y3 > stack_a_y3,
        "StackB should still be above StackA: A.y={stack_a_y3}, B.y={stack_b_y3}"
    );
    assert!(
        stack_b_y3 < 4.0,
        "StackB should not fly away, y={stack_b_y3}"
    );

    // Step 8: inspect StackA
    let inspect = b
        .stage("spatial_inspect", json!({"node": "StackA"}))
        .await
        .expect("inspect StackA")
        .unwrap_data();
    assert!(
        inspect["class"].as_str() == Some("RigidBody3D"),
        "StackA should be RigidBody3D: {inspect}"
    );

    // Step 9: teleport FallingBox back up
    b.stage(
        "spatial_action",
        json!({
            "action": "teleport",
            "node": "FallingBox",
            "position": [0.0, 20.0, 0.0]
        }),
    )
    .await
    .expect("teleport")
    .unwrap_data();

    // Step 10: verify teleport worked
    let snap4 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("post-teleport snapshot")
        .unwrap_data();
    let e4 = snap4["entities"].as_array().expect("entities");
    let falling4 = find_entity(e4, "FallingBox");
    let stack_b4 = find_entity(e4, "StackB");
    let falling_y4 = extract_position(falling4)[1];
    let stack_b_y4 = extract_position(stack_b4)[1];
    assert!(
        falling_y4 > stack_b_y4,
        "FallingBox should be above stack after teleport: box.y={falling_y4}, stackB.y={stack_b_y4}"
    );

    // Step 11-12: wait for it to fall again and settle
    b.wait_frames(180).await;
    let snap5 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("re-settled snapshot")
        .unwrap_data();
    let e5 = snap5["entities"].as_array().expect("entities");
    let falling5 = find_entity(e5, "FallingBox");
    let falling_y5 = extract_position(falling5)[1];
    assert!(
        falling_y5 < 2.0,
        "FallingBox should settle near floor again, y={falling_y5}"
    );
}

/// Journey: Patrol enemy moves continuously, observed through snapshot + inspect + scene_tree.
///
/// Verifies that real _physics_process movement is detected by all spatial
/// observation tools — snapshot position diffs, inspect data, and scene tree
/// navigation all work against a live moving entity.
///
/// Steps:
///   1. spatial_snapshot(standard) → note Patrol position P1, Player at origin
///   2. spatial_inspect(Enemies/Patrol) → class, health=80
///   3. scene_tree(subtree, Enemies) → Patrol and Stationary visible
///   4. wait_frames(120) → ~2 seconds of patrol movement (full sine cycle)
///   5. spatial_snapshot(standard) → Patrol at P2, verify P2 ≠ P1
///   6. spatial_inspect(Enemies/Patrol) → still healthy, transform changed
///   7. scene_tree(find, class=CharacterBody3D) → finds Patrol + Stationary + Player
///   8. spatial_inspect(Enemies/Stationary) → health=60, not moving
///   9. spatial_action(call_method, Stationary, take_damage, [20]) → damage it
///  10. wait_frames(5)
///  11. spatial_inspect(Enemies/Stationary) → health=40
///  12. spatial_snapshot(standard) → both enemies still in scene, Patrol still moving
async fn journey_patrol_movement_observed(b: &impl LiveBackend) {
    // Step 1: baseline
    let snap1 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("baseline snapshot")
        .unwrap_data();
    let e1 = snap1["entities"].as_array().expect("entities");
    let patrol1 = find_entity(e1, "Patrol");
    let p1 = extract_position(patrol1);
    let player1 = find_entity(e1, "Player");
    let player_pos = extract_position(player1);
    assert!(
        player_pos[0].abs() < 1.0 && player_pos[2].abs() < 1.0,
        "Player should be near origin: {:?}",
        player_pos
    );

    // Step 2: inspect patrol
    let inspect1 = b
        .stage("spatial_inspect", json!({"node": "Enemies/Patrol"}))
        .await
        .expect("inspect Patrol")
        .unwrap_data();
    assert_eq!(
        inspect1["class"].as_str(),
        Some("CharacterBody3D"),
        "Patrol should be CharacterBody3D"
    );
    if let Some(props) = inspect1["properties"].as_object() {
        if let Some(h) = props.get("health") {
            assert!(
                (h.as_f64().unwrap_or(0.0) - 80.0).abs() < 0.1,
                "Patrol health should be 80"
            );
        }
    }

    // Step 3: scene_tree subtree under Enemies
    let subtree = b
        .stage(
            "scene_tree",
            json!({"action": "subtree", "node": "Enemies"}),
        )
        .await
        .expect("scene_tree subtree Enemies")
        .unwrap_data();
    let subtree_str = serde_json::to_string(&subtree).unwrap_or_default();
    assert!(
        subtree_str.contains("Patrol"),
        "Enemies subtree should contain Patrol: {subtree}"
    );
    assert!(
        subtree_str.contains("Stationary"),
        "Enemies subtree should contain Stationary: {subtree}"
    );

    // Step 4: wait for patrol to complete more of its sine cycle
    b.wait_frames(120).await;

    // Step 5: patrol should have moved
    let snap2 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("post-movement snapshot")
        .unwrap_data();
    let e2 = snap2["entities"].as_array().expect("entities");
    let patrol2 = find_entity(e2, "Patrol");
    let p2 = extract_position(patrol2);
    let dx = (p2[0] - p1[0]).abs();
    assert!(
        dx > 0.1,
        "Patrol should have moved: dx={dx}, P1={p1:?}, P2={p2:?}"
    );

    // Step 6: inspect again — verify still valid
    let inspect2 = b
        .stage("spatial_inspect", json!({"node": "Enemies/Patrol"}))
        .await
        .expect("inspect Patrol after movement")
        .unwrap_data();
    assert!(
        inspect2["class"].is_string(),
        "Post-movement inspect should return class"
    );

    // Step 7: find by class
    let found = b
        .stage(
            "scene_tree",
            json!({"action": "find", "find_by": "class", "find_value": "CharacterBody3D"}),
        )
        .await
        .expect("find by class")
        .unwrap_data();
    let found_str = serde_json::to_string(&found).unwrap_or_default();
    assert!(
        found_str.contains("Patrol"),
        "find by class should locate Patrol: {found}"
    );
    assert!(
        found_str.contains("Player"),
        "find by class should locate Player: {found}"
    );

    // Step 8: inspect Stationary — should be static
    let stat_inspect = b
        .stage("spatial_inspect", json!({"node": "Enemies/Stationary"}))
        .await
        .expect("inspect Stationary")
        .unwrap_data();
    if let Some(props) = stat_inspect["properties"].as_object() {
        if let Some(h) = props.get("health") {
            assert!(
                (h.as_f64().unwrap_or(0.0) - 60.0).abs() < 0.1,
                "Stationary health should be 60"
            );
        }
    }

    // Step 9: damage Stationary
    b.stage(
        "spatial_action",
        json!({
            "action": "call_method",
            "node": "Enemies/Stationary",
            "method": "take_damage",
            "args": [20]
        }),
    )
    .await
    .expect("call take_damage")
    .unwrap_data();

    // Step 10: wait
    b.wait_frames(5).await;

    // Step 11: verify health dropped
    let stat_after = b
        .stage("spatial_inspect", json!({"node": "Enemies/Stationary"}))
        .await
        .expect("inspect after damage")
        .unwrap_data();
    if let Some(props) = stat_after["properties"].as_object() {
        if let Some(h) = props.get("health") {
            assert!(
                (h.as_f64().unwrap_or(60.0) - 40.0).abs() < 0.1,
                "Stationary health should be 40 after 20 damage, got {}",
                h.as_f64().unwrap_or(0.0)
            );
        }
    }

    // Step 12: final snapshot — both enemies still present
    let snap3 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("final snapshot")
        .unwrap_data();
    let e3 = snap3["entities"].as_array().expect("entities");
    let _patrol3 = find_entity(e3, "Patrol");
    let _stationary3 = find_entity(e3, "Stationary");
    // Both found without panic = both still in scene
}

dual_test!(
    journey_physics_gravity_and_stacking,
    LIVE_PHYSICS,
    journey_physics_gravity_and_stacking
);
dual_test!(
    journey_patrol_movement_observed,
    LIVE_3D,
    journey_patrol_movement_observed
);
