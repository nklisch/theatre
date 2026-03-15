use crate::dual_test;
use crate::harness::*;
use crate::harness::assertions::*;
use serde_json::json;

const LIVE_3D: &str = "res://live_scene_3d.tscn";
const LIVE_PHYSICS: &str = "res://live_scene_physics.tscn";

async fn gravity_pulls_rigidbody_down(b: &impl LiveBackend) {
    // Step 1: initial snapshot — FallingBox should be at y≈10
    let snap1 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("snapshot 1")
        .unwrap_data();
    let entities = snap1["entities"].as_array().expect("entities");
    let falling = find_entity(entities, "FallingBox");
    let pos1 = extract_position(falling);
    assert!(pos1[1] > 8.0, "FallingBox should start high, got y={}", pos1[1]);

    // Step 2: wait 2 seconds
    b.wait_frames(120).await;

    // Step 3: should have fallen significantly
    let snap2 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("snapshot 2")
        .unwrap_data();
    let entities2 = snap2["entities"].as_array().expect("entities");
    let falling2 = find_entity(entities2, "FallingBox");
    let pos2 = extract_position(falling2);
    assert!(pos2[1] < 5.0, "FallingBox should have fallen, y={}", pos2[1]);

    // Step 4: wait 2 more seconds — should be resting near floor
    b.wait_frames(120).await;

    let snap3 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("snapshot 3")
        .unwrap_data();
    let entities3 = snap3["entities"].as_array().expect("entities");
    let falling3 = find_entity(entities3, "FallingBox");
    let pos3 = extract_position(falling3);
    assert!(pos3[1] < 2.0, "FallingBox should be near floor, y={}", pos3[1]);
}

async fn patrol_enemy_moves_over_time(b: &impl LiveBackend) {
    let snap1 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("snapshot 1")
        .unwrap_data();
    let entities1 = snap1["entities"].as_array().expect("entities");
    let patrol1 = find_entity(entities1, "Patrol");
    let pos1 = extract_position(patrol1);

    b.wait_frames(60).await;

    let snap2 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("snapshot 2")
        .unwrap_data();
    let entities2 = snap2["entities"].as_array().expect("entities");
    let patrol2 = find_entity(entities2, "Patrol");
    let pos2 = extract_position(patrol2);

    let dx = (pos2[0] - pos1[0]).abs();
    assert!(dx > 0.5, "Patrol should have moved measurably, dx={dx}");
}

async fn stacked_rigidbodies_settle(b: &impl LiveBackend) {
    // Wait for physics to settle
    b.wait_frames(180).await;

    let snap = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("snapshot")
        .unwrap_data();
    let entities = snap["entities"].as_array().expect("entities");
    let stack_a = find_entity(entities, "StackA");
    let stack_b = find_entity(entities, "StackB");
    let pos_a = extract_position(stack_a);
    let pos_b = extract_position(stack_b);

    // B should be above A
    assert!(
        pos_b[1] > pos_a[1],
        "StackB should be above StackA: A.y={}, B.y={}",
        pos_a[1],
        pos_b[1]
    );

    // Both should be near the floor (not flying away)
    assert!(pos_a[1] < 3.0, "StackA should be near floor, y={}", pos_a[1]);
    assert!(pos_b[1] < 4.0, "StackB should be near floor, y={}", pos_b[1]);
}

async fn teleport_interrupts_physics(b: &impl LiveBackend) {
    // Wait for box to fall and settle near floor
    b.wait_frames(180).await;

    // Verify box is near floor
    let snap1 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("snapshot before teleport")
        .unwrap_data();
    let entities1 = snap1["entities"].as_array().expect("entities");
    let falling1 = find_entity(entities1, "FallingBox");
    let pos1 = extract_position(falling1);
    assert!(
        pos1[1] < 3.0,
        "FallingBox should be near floor before teleport, y={}",
        pos1[1]
    );

    // Teleport it back up
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

    // Immediately snapshot — box should be near y=20 (may have fallen slightly)
    let snap2 = b
        .stage("spatial_snapshot", json!({"detail": "standard"}))
        .await
        .expect("snapshot after teleport")
        .unwrap_data();
    let entities2 = snap2["entities"].as_array().expect("entities");
    let falling2 = find_entity(entities2, "FallingBox");
    let pos2 = extract_position(falling2);

    // Should be significantly above where it was (teleport worked)
    assert!(
        pos2[1] > pos1[1] + 5.0,
        "FallingBox should be much higher after teleport: before y={}, after y={}",
        pos1[1],
        pos2[1]
    );
}

dual_test!(gravity_pulls_rigidbody_down, LIVE_PHYSICS, gravity_pulls_rigidbody_down);
dual_test!(patrol_enemy_moves_over_time, LIVE_3D, patrol_enemy_moves_over_time);
dual_test!(stacked_rigidbodies_settle, LIVE_PHYSICS, stacked_rigidbodies_settle);
dual_test!(teleport_interrupts_physics, LIVE_PHYSICS, teleport_interrupts_physics);
