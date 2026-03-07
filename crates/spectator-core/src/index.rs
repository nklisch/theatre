use rstar::{AABB, PointDistance, RTree, RTreeObject};

use crate::types::Position3;

/// An indexed entity with its position and metadata.
#[derive(Debug, Clone)]
pub struct IndexedEntity {
    pub path: String,
    pub class: String,
    pub position: Position3,
    pub groups: Vec<String>,
}

impl RTreeObject for IndexedEntity {
    type Envelope = AABB<[f64; 3]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_point(self.position)
    }
}

impl PointDistance for IndexedEntity {
    fn distance_2(&self, point: &[f64; 3]) -> f64 {
        let dx = self.position[0] - point[0];
        let dy = self.position[1] - point[1];
        let dz = self.position[2] - point[2];
        dx * dx + dy * dy + dz * dz
    }
}

/// R-tree spatial index for efficient spatial queries.
pub struct SpatialIndex {
    tree: RTree<IndexedEntity>,
}

impl SpatialIndex {
    /// Build a new index from a set of entities.
    pub fn build(entities: Vec<IndexedEntity>) -> Self {
        Self {
            tree: RTree::bulk_load(entities),
        }
    }

    /// Create an empty index.
    pub fn empty() -> Self {
        Self {
            tree: RTree::new(),
        }
    }

    /// Find the K nearest entities to a point.
    /// Results are sorted by distance (nearest first).
    /// Applies optional group and class filters.
    pub fn nearest(
        &self,
        point: Position3,
        k: usize,
        groups: &[String],
        class_filter: &[String],
    ) -> Vec<NearestResult> {
        self.tree
            .nearest_neighbor_iter(&point)
            .filter(|e| Self::matches_filters(e, groups, class_filter))
            .take(k)
            .map(|e| NearestResult {
                path: e.path.clone(),
                class: e.class.clone(),
                position: e.position,
                distance: crate::bearing::distance(point, e.position),
                groups: e.groups.clone(),
            })
            .collect()
    }

    /// Find all entities within a radius of a point.
    /// Results are sorted by distance (nearest first).
    pub fn within_radius(
        &self,
        point: Position3,
        radius: f64,
        groups: &[String],
        class_filter: &[String],
    ) -> Vec<NearestResult> {
        let r2 = radius * radius;
        let envelope = AABB::from_corners(
            [point[0] - radius, point[1] - radius, point[2] - radius],
            [point[0] + radius, point[1] + radius, point[2] + radius],
        );
        let mut results: Vec<_> = self
            .tree
            .locate_in_envelope(&envelope)
            .filter(|e| e.distance_2(&point) <= r2)
            .filter(|e| Self::matches_filters(e, groups, class_filter))
            .map(|e| NearestResult {
                path: e.path.clone(),
                class: e.class.clone(),
                position: e.position,
                distance: crate::bearing::distance(point, e.position),
                groups: e.groups.clone(),
            })
            .collect();
        results.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    /// Check if an entity matches group and class filters.
    fn matches_filters(
        entity: &IndexedEntity,
        groups: &[String],
        class_filter: &[String],
    ) -> bool {
        let group_ok =
            groups.is_empty() || entity.groups.iter().any(|g| groups.contains(g));
        let class_ok =
            class_filter.is_empty() || class_filter.iter().any(|c| c == &entity.class);
        group_ok && class_ok
    }

    /// Return the number of indexed entities.
    pub fn len(&self) -> usize {
        self.tree.size()
    }

    pub fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }
}

/// Result of a nearest/radius query.
#[derive(Debug, Clone)]
pub struct NearestResult {
    pub path: String,
    pub class: String,
    pub position: Position3,
    pub distance: f64,
    pub groups: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(path: &str, pos: [f64; 3]) -> IndexedEntity {
        IndexedEntity {
            path: path.into(),
            class: "Node3D".into(),
            position: pos,
            groups: vec![],
        }
    }

    fn entity_with_group(path: &str, pos: [f64; 3], group: &str) -> IndexedEntity {
        IndexedEntity {
            path: path.into(),
            class: "Node3D".into(),
            position: pos,
            groups: vec![group.into()],
        }
    }

    #[test]
    fn nearest_returns_k_closest() {
        let entities = vec![
            entity("a", [0.0, 0.0, 0.0]),
            entity("b", [5.0, 0.0, 0.0]),
            entity("c", [10.0, 0.0, 0.0]),
        ];
        let index = SpatialIndex::build(entities);
        let results = index.nearest([0.0, 0.0, 0.0], 2, &[], &[]);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, "a");
        assert_eq!(results[1].path, "b");
    }

    #[test]
    fn within_radius_filters_by_distance() {
        let entities = vec![
            entity("close", [3.0, 0.0, 0.0]),
            entity("far", [100.0, 0.0, 0.0]),
        ];
        let index = SpatialIndex::build(entities);
        let results = index.within_radius([0.0, 0.0, 0.0], 10.0, &[], &[]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "close");
    }

    #[test]
    fn group_filter_applies() {
        let entities = vec![
            entity_with_group("enemy", [1.0, 0.0, 0.0], "enemies"),
            entity_with_group("pickup", [2.0, 0.0, 0.0], "pickups"),
        ];
        let index = SpatialIndex::build(entities);
        let results = index.nearest([0.0, 0.0, 0.0], 5, &["enemies".to_string()], &[]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "enemy");
    }

    #[test]
    fn class_filter_applies() {
        let mut e1 = entity("n1", [1.0, 0.0, 0.0]);
        e1.class = "CharacterBody3D".into();
        let mut e2 = entity("n2", [2.0, 0.0, 0.0]);
        e2.class = "Area3D".into();
        let index = SpatialIndex::build(vec![e1, e2]);
        let results = index.nearest([0.0, 0.0, 0.0], 5, &[], &["Area3D".to_string()]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "n2");
    }

    #[test]
    fn empty_index_returns_empty() {
        let index = SpatialIndex::empty();
        let results = index.nearest([0.0, 0.0, 0.0], 5, &[], &[]);
        assert!(results.is_empty());
        let results2 = index.within_radius([0.0, 0.0, 0.0], 100.0, &[], &[]);
        assert!(results2.is_empty());
    }

    #[test]
    fn within_radius_sorted_by_distance() {
        let entities = vec![
            entity("far_ish", [8.0, 0.0, 0.0]),
            entity("near", [2.0, 0.0, 0.0]),
            entity("mid", [5.0, 0.0, 0.0]),
        ];
        let index = SpatialIndex::build(entities);
        let results = index.within_radius([0.0, 0.0, 0.0], 10.0, &[], &[]);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].path, "near");
        assert_eq!(results[1].path, "mid");
        assert_eq!(results[2].path, "far_ish");
    }
}
