mod support;

use serde_json::{Value, json};
use support::e2e_harness::E2EHarness;

fn props<'a>(inspection: &'a Value, name: &str) -> &'a Value {
    &inspection["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|child| child["name"] == name)
        .unwrap()["props"]
}

#[tokio::test]
#[ignore = "requires Godot and deployed GDExtension"]
async fn disabled_area_inspection_is_observational_and_diagnostic_free() {
    let mut h = E2EHarness::start_3d().await.unwrap();
    h.expect(1, "spatial_action", json!({"action":"call_method", "node":"/root/TestScene3D", "method":"create_area_monitoring_fixture"})).await;
    h.wait_frames(5).await;
    let root = "/root/TestScene3D/MonitoringFixture";
    let children = h
        .expect(
            2,
            "spatial_inspect",
            json!({"node":root,"include":["transform","children"]}),
        )
        .await;
    let nested = h
        .expect(
            3,
            "spatial_inspect",
            json!({"node":format!("{root}/Disabled3D"),"include":["children"]}),
        )
        .await;
    let context = h
        .expect(
            4,
            "spatial_inspect",
            json!({"node":format!("{root}/Body3D"),"include":["spatial_context"]}),
        )
        .await;
    let diagnostics = h.expect(5, "runtime_diagnostics", json!({})).await;
    assert!(
        !diagnostics.to_string().contains("monitoring is off"),
        "{diagnostics}"
    );
    assert!(
        !h.godot.stderr_output().contains("monitoring is off"),
        "{}",
        h.godot.stderr_output()
    );
    for dimension in ["2D", "3D"] {
        let disabled = props(&children, &format!("Disabled{dimension}"));
        assert_eq!(disabled["monitoring"], false);
        assert!(disabled.get("overlapping_bodies").unwrap().is_null());
        let empty = props(&children, &format!("Empty{dimension}"));
        assert_eq!(empty["monitoring"], true);
        assert_eq!(empty["overlapping_bodies"], json!([]));
    }
    assert_eq!(
        props(&children, "Enabled2D")["overlapping_bodies"],
        json!(["Body2D"])
    );
    assert_eq!(
        props(&nested, "Enabled3D")["overlapping_bodies"],
        json!(["Body3D"])
    );
    assert!(
        context["spatial_context"]["in_areas"]
            .as_array()
            .unwrap()
            .iter()
            .any(|area| area.as_str().unwrap().ends_with("Disabled3D/Enabled3D")),
        "{context}"
    );
    // Read the actual engine properties after inspection, not only Stage's summary.
    for dimension in ["2D", "3D"] {
        let result = h.expect(6, "spatial_action", json!({"action":"call_method","node":format!("{root}/Disabled{dimension}"),"method":"is_monitoring"})).await;
        assert_eq!(result["details"]["return_value"], false, "{result}");
    }
}
