use crate::types::{Cardinal, RawEntityData, RelativePosition};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A cluster of entities for summary-tier output.
#[derive(Debug, Clone, Serialize)]
pub struct Cluster {
    pub label: String,
    pub count: usize,
    pub nearest: Option<ClusterNearest>,
    pub farthest_dist: f64,
    /// Natural-language summary of cluster members' states.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Note for static clusters (e.g., "unchanged").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Nearest entity in a cluster.
#[derive(Debug, Clone, Serialize)]
pub struct ClusterNearest {
    pub node: String,
    pub distance: f64,
    pub bearing: Cardinal,
}

/// Clustering strategy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusterStrategy {
    #[default]
    Group,
    Class,
    Proximity,
    None,
}

/// Cluster entities by group membership (default strategy).
///
/// Each entity is assigned to its first group. Entities with no groups
/// go into an "other" cluster. Static entities go into a "static_geometry"
/// cluster.
///
/// `entities` must already have `rel` data computed (distances, bearings).
pub(crate) fn cluster_by_group(entities: &[(RawEntityData, RelativePosition)]) -> Vec<Cluster> {
    let mut group_map: HashMap<String, Vec<(&RawEntityData, &RelativePosition)>> = HashMap::new();
    let mut static_count = 0usize;

    for (entity, rel) in entities {
        if entity.is_static {
            static_count += 1;
        } else {
            let label = entity
                .groups
                .first()
                .cloned()
                .unwrap_or_else(|| "other".to_string());
            group_map.entry(label).or_default().push((entity, rel));
        }
    }

    let mut clusters = Vec::new();

    // Build a cluster for each dynamic group
    for (label, members) in &group_map {
        let cluster = build_cluster(label.clone(), members, None);
        clusters.push(cluster);
    }

    // Sort clusters by label for deterministic output
    clusters.sort_by(|a, b| a.label.cmp(&b.label));

    // Add static cluster if any static entities
    if static_count > 0 {
        clusters.push(static_cluster(static_count));
    }

    clusters
}

fn build_cluster(
    label: String,
    members: &[(&RawEntityData, &RelativePosition)],
    note: Option<String>,
) -> Cluster {
    let count = members.len();

    let nearest = members
        .iter()
        .min_by(|a, b| {
            a.1.distance
                .partial_cmp(&b.1.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, rel)| ClusterNearest {
            node: entity.path.clone(),
            distance: rel.distance,
            bearing: rel.bearing,
        });

    let farthest_dist = members
        .iter()
        .map(|(_, rel)| rel.distance)
        .fold(0.0_f64, f64::max);

    let entity_refs: Vec<&RawEntityData> = members.iter().map(|(e, _)| *e).collect();
    let summary = generate_cluster_summary(&entity_refs);

    Cluster {
        label,
        count,
        nearest,
        farthest_dist,
        summary,
        note,
    }
}

/// Build a static_geometry cluster with the given entity count.
fn static_cluster(count: usize) -> Cluster {
    Cluster {
        label: "static_geometry".to_string(),
        count,
        nearest: None,
        farthest_dist: 0.0,
        summary: None,
        note: Some("unchanged".to_string()),
    }
}

/// Dispatch clustering based on strategy.
pub fn cluster_entities(
    entities: &[(RawEntityData, RelativePosition)],
    strategy: ClusterStrategy,
) -> Vec<Cluster> {
    match strategy {
        ClusterStrategy::Group => cluster_by_group(entities),
        ClusterStrategy::Class => cluster_by_class(entities),
        ClusterStrategy::Proximity => cluster_by_proximity(entities),
        ClusterStrategy::None => cluster_none(entities),
    }
}

/// Cluster by Godot class name.
pub(crate) fn cluster_by_class(entities: &[(RawEntityData, RelativePosition)]) -> Vec<Cluster> {
    let mut class_map: HashMap<String, Vec<(&RawEntityData, &RelativePosition)>> = HashMap::new();
    let mut static_count = 0usize;

    for (entity, rel) in entities {
        if entity.is_static {
            static_count += 1;
        } else {
            class_map
                .entry(entity.class.clone())
                .or_default()
                .push((entity, rel));
        }
    }

    let mut clusters: Vec<Cluster> = class_map
        .into_iter()
        .map(|(label, members)| build_cluster(label, &members, None))
        .collect();

    clusters.sort_by(|a, b| a.label.cmp(&b.label));

    if static_count > 0 {
        clusters.push(static_cluster(static_count));
    }

    clusters
}

/// Cluster by spatial proximity (simple nearest-seed algorithm).
pub(crate) fn cluster_by_proximity(entities: &[(RawEntityData, RelativePosition)]) -> Vec<Cluster> {
    let dynamic: Vec<(&RawEntityData, &RelativePosition)> = entities
        .iter()
        .filter(|(e, _)| !e.is_static)
        .map(|(e, r)| (e, r))
        .collect();
    let static_count = entities.len() - dynamic.len();

    if dynamic.is_empty() {
        let mut clusters = Vec::new();
        if static_count > 0 {
            clusters.push(static_cluster(static_count));
        }
        return clusters;
    }

    // Simple proximity clustering: merge threshold 10 units
    const MERGE_THRESHOLD: f64 = 10.0;
    let mut cluster_centers: Vec<[f64; 3]> = Vec::new();
    let mut cluster_members: Vec<Vec<(&RawEntityData, &RelativePosition)>> = Vec::new();

    for (entity, rel) in &dynamic {
        let pos = entity.position;
        let mut assigned = false;
        for (i, center) in cluster_centers.iter().enumerate() {
            let dx = pos[0] - center[0];
            let dy = pos[1] - center[1];
            let dz = pos[2] - center[2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist < MERGE_THRESHOLD {
                cluster_members[i].push((entity, rel));
                assigned = true;
                break;
            }
        }
        if !assigned {
            cluster_centers.push(pos);
            cluster_members.push(vec![(entity, rel)]);
        }
    }

    let mut clusters: Vec<Cluster> = cluster_members
        .iter()
        .enumerate()
        .map(|(i, members)| {
            let label = format!("cluster_{}", i + 1);
            build_cluster(label, members, None)
        })
        .collect();

    if static_count > 0 {
        clusters.push(static_cluster(static_count));
    }

    clusters
}

/// No clustering — each entity is its own cluster (except static).
fn cluster_none(entities: &[(RawEntityData, RelativePosition)]) -> Vec<Cluster> {
    let mut clusters = Vec::new();
    let mut static_count = 0usize;

    for (entity, rel) in entities {
        if entity.is_static {
            static_count += 1;
            continue;
        }
        clusters.push(Cluster {
            label: entity.path.clone(),
            count: 1,
            nearest: Some(ClusterNearest {
                node: entity.path.clone(),
                distance: rel.distance,
                bearing: rel.bearing,
            }),
            farthest_dist: rel.distance,
            summary: None,
            note: None,
        });
    }

    if static_count > 0 {
        clusters.push(static_cluster(static_count));
    }

    clusters
}

/// Generate a natural-language summary for a cluster.
///
/// Examines common state properties (e.g., "state", "alert_level")
/// and counts distinct values. Example output: "2 idle, 1 patrol".
pub(crate) fn generate_cluster_summary(entities: &[&RawEntityData]) -> Option<String> {
    if entities.is_empty() {
        return None;
    }

    const STATE_KEYS: &[&str] = &["state", "alert_level", "status", "mode"];

    // Find the first key that any entity has
    for key in STATE_KEYS {
        let mut value_counts: HashMap<String, usize> = HashMap::new();
        let mut found_any = false;

        for entity in entities {
            if let Some(val) = entity.state.get(*key) {
                found_any = true;
                let val_str = match val {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                *value_counts.entry(val_str).or_insert(0) += 1;
            }
        }

        if found_any {
            // Sort by count descending, then by value for determinism
            let mut pairs: Vec<(String, usize)> = value_counts.into_iter().collect();
            pairs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

            let parts: Vec<String> = pairs
                .iter()
                .map(|(val, count)| format!("{count} {val}"))
                .collect();
            return Some(parts.join(", "));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Cardinal, RelativePosition};

    fn make_entity(path: &str, groups: &[&str], is_static: bool) -> RawEntityData {
        RawEntityData {
            path: path.to_string(),
            class: "Node3D".to_string(),
            position: [0.0, 0.0, 0.0],
            rotation_deg: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            groups: groups.iter().map(|g| g.to_string()).collect(),
            state: serde_json::Map::new(),
            visible: true,
            is_static,
            children: Vec::new(),
            script: None,
            signals_recent: Vec::new(),
            signals_connected: Vec::new(),
            physics: None,
            transform: None,
        }
    }

    fn make_rel(dist: f64) -> RelativePosition {
        RelativePosition {
            distance: dist,
            bearing: Cardinal::Ahead,
            bearing_deg: 0.0,
            elevation: None,
            occluded: false,
        }
    }

    #[test]
    fn cluster_by_group_basic() {
        let entities = vec![
            (
                make_entity("enemies/e1", &["enemies"], false),
                make_rel(5.0),
            ),
            (
                make_entity("enemies/e2", &["enemies"], false),
                make_rel(10.0),
            ),
            (
                make_entity("enemies/e3", &["enemies"], false),
                make_rel(3.0),
            ),
        ];
        let clusters = cluster_by_group(&entities);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].label, "enemies");
        assert_eq!(clusters[0].count, 3);
    }

    #[test]
    fn ungrouped_entities_in_other() {
        let entities = vec![(make_entity("node1", &[], false), make_rel(5.0))];
        let clusters = cluster_by_group(&entities);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].label, "other");
        assert_eq!(clusters[0].count, 1);
    }

    #[test]
    fn static_entities_separate_cluster() {
        let entities = vec![
            (make_entity("wall", &[], true), make_rel(1.0)),
            (make_entity("floor", &[], true), make_rel(2.0)),
            (make_entity("enemy", &["enemies"], false), make_rel(5.0)),
        ];
        let clusters = cluster_by_group(&entities);

        let static_cluster = clusters.iter().find(|c| c.label == "static_geometry");
        assert!(static_cluster.is_some(), "Expected static_geometry cluster");
        let sc = static_cluster.unwrap();
        assert_eq!(sc.count, 2);
        assert_eq!(sc.note.as_deref(), Some("unchanged"));

        let enemy_cluster = clusters.iter().find(|c| c.label == "enemies");
        assert!(enemy_cluster.is_some());
        assert_eq!(enemy_cluster.unwrap().count, 1);
    }

    #[test]
    fn nearest_and_farthest_dist() {
        let entities = vec![
            (
                make_entity("enemies/e1", &["enemies"], false),
                make_rel(5.0),
            ),
            (
                make_entity("enemies/e2", &["enemies"], false),
                make_rel(10.0),
            ),
            (
                make_entity("enemies/e3", &["enemies"], false),
                make_rel(3.0),
            ),
        ];
        let clusters = cluster_by_group(&entities);
        let c = &clusters[0];

        let nearest = c.nearest.as_ref().unwrap();
        assert_eq!(nearest.node, "enemies/e3");
        assert!((nearest.distance - 3.0).abs() < 1e-10);
        assert!((c.farthest_dist - 10.0).abs() < 1e-10);
    }

    #[test]
    fn cluster_summary_generation() {
        let mut e1 = make_entity("e1", &["enemies"], false);
        e1.state
            .insert("state".to_string(), serde_json::json!("idle"));
        let mut e2 = make_entity("e2", &["enemies"], false);
        e2.state
            .insert("state".to_string(), serde_json::json!("idle"));
        let mut e3 = make_entity("e3", &["enemies"], false);
        e3.state
            .insert("state".to_string(), serde_json::json!("patrol"));

        let refs: Vec<&RawEntityData> = [&e1, &e2, &e3].to_vec();
        let summary = generate_cluster_summary(&refs);
        assert!(summary.is_some());
        let s = summary.unwrap();
        assert!(s.contains("2 idle"), "Expected '2 idle' in '{s}'");
        assert!(s.contains("1 patrol"), "Expected '1 patrol' in '{s}'");
    }

    #[test]
    fn cluster_summary_none_when_no_state_keys() {
        let e1 = make_entity("e1", &["enemies"], false);
        let refs: Vec<&RawEntityData> = [&e1].to_vec();
        let summary = generate_cluster_summary(&refs);
        assert!(summary.is_none());
    }

    #[test]
    fn cluster_by_class_basic() {
        let entities = vec![
            (
                make_entity("enemies/e1", &["enemies"], false),
                make_rel(5.0),
            ),
            (
                make_entity("pickups/p1", &["pickups"], false),
                make_rel(3.0),
            ),
        ];
        // Both entities have class "Node3D" (from make_entity)
        let clusters = cluster_by_class(&entities);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].label, "Node3D");
        assert_eq!(clusters[0].count, 2);
    }

    #[test]
    fn cluster_none_each_entity() {
        let entities = vec![
            (
                make_entity("enemies/e1", &["enemies"], false),
                make_rel(5.0),
            ),
            (
                make_entity("enemies/e2", &["enemies"], false),
                make_rel(10.0),
            ),
        ];
        let clusters = cluster_none(&entities);
        assert_eq!(clusters.len(), 2);
    }

    #[test]
    fn cluster_dispatch_group() {
        let entities = vec![(
            make_entity("enemies/e1", &["enemies"], false),
            make_rel(5.0),
        )];
        let clusters = cluster_entities(&entities, ClusterStrategy::Group);
        assert_eq!(clusters[0].label, "enemies");
    }

    #[test]
    fn cluster_dispatch_none() {
        let entities = vec![(
            make_entity("enemies/e1", &["enemies"], false),
            make_rel(5.0),
        )];
        let clusters = cluster_entities(&entities, ClusterStrategy::None);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].label, "enemies/e1");
    }

    #[test]
    fn cluster_proximity_groups_nearby() {
        // Two entities very close together → should be in 1 cluster
        let mut e1 = make_entity("a/e1", &[], false);
        e1.position = [0.0, 0.0, 0.0];
        let mut e2 = make_entity("a/e2", &[], false);
        e2.position = [1.0, 0.0, 0.0]; // within 10 units
        let entities = vec![(e1, make_rel(1.0)), (e2, make_rel(2.0))];
        let clusters = cluster_by_proximity(&entities);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].count, 2);
    }
}
