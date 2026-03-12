use std::collections::HashMap;

use rstar::{AABB, PointDistance, RTree, RTreeObject};

use crate::types::{Position2, Position3};

/// 2D grid hash cell size (world units).
const GRID_CELL_SIZE: f64 = 64.0;

/// An indexed entity with its position and metadata (3D).
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

/// A 2D indexed entity.
#[derive(Debug, Clone)]
pub struct IndexedEntity2D {
    pub path: String,
    pub class: String,
    pub position: Position2,
    pub groups: Vec<String>,
}

/// R-tree spatial index for 3D (implementation detail of SpatialIndex enum).
pub struct SpatialIndex3D {
    tree: RTree<IndexedEntity>,
}

impl SpatialIndex3D {
    fn build(entities: Vec<IndexedEntity>) -> Self {
        Self {
            tree: RTree::bulk_load(entities),
        }
    }

    fn empty() -> Self {
        Self { tree: RTree::new() }
    }

    fn nearest(
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

    fn within_radius(
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

    fn matches_filters(entity: &IndexedEntity, groups: &[String], class_filter: &[String]) -> bool {
        let group_ok = groups.is_empty() || entity.groups.iter().any(|g| groups.contains(g));
        let class_ok = class_filter.is_empty() || class_filter.iter().any(|c| c == &entity.class);
        group_ok && class_ok
    }

    fn len(&self) -> usize {
        self.tree.size()
    }

    fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }
}

/// Grid hash for 2D spatial indexing (implementation detail of SpatialIndex enum).
pub struct GridHash2D {
    cells: HashMap<(i64, i64), Vec<IndexedEntity2D>>,
    all: Vec<IndexedEntity2D>,
    cell_size: f64,
}

impl GridHash2D {
    fn build(entities: Vec<IndexedEntity2D>, cell_size: f64) -> Self {
        let mut cells: HashMap<(i64, i64), Vec<IndexedEntity2D>> = HashMap::new();
        for entity in &entities {
            let key = Self::cell_key(entity.position, cell_size);
            cells.entry(key).or_default().push(entity.clone());
        }
        Self {
            cells,
            all: entities,
            cell_size,
        }
    }

    fn cell_key(pos: Position2, cell_size: f64) -> (i64, i64) {
        (
            (pos[0] / cell_size).floor() as i64,
            (pos[1] / cell_size).floor() as i64,
        )
    }

    fn nearest(
        &self,
        point: Position2,
        k: usize,
        groups: &[String],
        class_filter: &[String],
    ) -> Vec<NearestResult> {
        let mut results: Vec<NearestResult> = self
            .all
            .iter()
            .filter(|e| Self::matches_filters(e, groups, class_filter))
            .map(|e| NearestResult {
                path: e.path.clone(),
                class: e.class.clone(),
                position: [e.position[0], e.position[1], 0.0],
                distance: crate::bearing::distance_2d(point, e.position),
                groups: e.groups.clone(),
            })
            .collect();
        results.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(k);
        results
    }

    fn within_radius(
        &self,
        point: Position2,
        radius: f64,
        groups: &[String],
        class_filter: &[String],
    ) -> Vec<NearestResult> {
        let r2 = radius * radius;
        let min_cx = ((point[0] - radius) / self.cell_size).floor() as i64;
        let max_cx = ((point[0] + radius) / self.cell_size).floor() as i64;
        let min_cy = ((point[1] - radius) / self.cell_size).floor() as i64;
        let max_cy = ((point[1] + radius) / self.cell_size).floor() as i64;

        let mut results: Vec<NearestResult> = Vec::new();
        for cx in min_cx..=max_cx {
            for cy in min_cy..=max_cy {
                if let Some(entities) = self.cells.get(&(cx, cy)) {
                    for e in entities {
                        let dx = e.position[0] - point[0];
                        let dy = e.position[1] - point[1];
                        let d2 = dx * dx + dy * dy;
                        if d2 <= r2 && Self::matches_filters(e, groups, class_filter) {
                            results.push(NearestResult {
                                path: e.path.clone(),
                                class: e.class.clone(),
                                position: [e.position[0], e.position[1], 0.0],
                                distance: d2.sqrt(),
                                groups: e.groups.clone(),
                            });
                        }
                    }
                }
            }
        }
        results.sort_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    fn matches_filters(
        entity: &IndexedEntity2D,
        groups: &[String],
        class_filter: &[String],
    ) -> bool {
        let group_ok = groups.is_empty() || entity.groups.iter().any(|g| groups.contains(g));
        let class_ok = class_filter.is_empty() || class_filter.iter().any(|c| c == &entity.class);
        group_ok && class_ok
    }

    fn len(&self) -> usize {
        self.all.len()
    }

    fn is_empty(&self) -> bool {
        self.all.is_empty()
    }
}

/// Spatial index — either 3D R-tree or 2D grid hash.
pub enum SpatialIndex {
    /// 3D R-tree spatial index (rstar).
    Three(SpatialIndex3D),
    /// 2D grid hash spatial index.
    Two(GridHash2D),
}

impl SpatialIndex {
    /// Build a 3D R-tree index.
    pub fn build(entities: Vec<IndexedEntity>) -> Self {
        Self::Three(SpatialIndex3D::build(entities))
    }

    /// Build a 2D grid hash index.
    pub fn build_2d(entities: Vec<IndexedEntity2D>) -> Self {
        Self::Two(GridHash2D::build(entities, GRID_CELL_SIZE))
    }

    /// Create an empty 3D index (default).
    pub fn empty() -> Self {
        Self::Three(SpatialIndex3D::empty())
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
        match self {
            Self::Three(idx) => idx.nearest(point, k, groups, class_filter),
            Self::Two(idx) => idx.nearest([point[0], point[1]], k, groups, class_filter),
        }
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
        match self {
            Self::Three(idx) => idx.within_radius(point, radius, groups, class_filter),
            Self::Two(idx) => idx.within_radius([point[0], point[1]], radius, groups, class_filter),
        }
    }

    /// Return the number of indexed entities.
    pub fn len(&self) -> usize {
        match self {
            Self::Three(idx) => idx.len(),
            Self::Two(idx) => idx.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Three(idx) => idx.is_empty(),
            Self::Two(idx) => idx.is_empty(),
        }
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

    fn entity2d(path: &str, pos: [f64; 2]) -> IndexedEntity2D {
        IndexedEntity2D {
            path: path.into(),
            class: "Node2D".into(),
            position: pos,
            groups: vec![],
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

    #[test]
    fn grid_hash_nearest() {
        let entities = vec![
            entity2d("a", [0.0, 0.0]),
            entity2d("b", [50.0, 0.0]),
            entity2d("c", [100.0, 0.0]),
        ];
        let index = SpatialIndex::build_2d(entities);
        let results = index.nearest([0.0, 0.0, 0.0], 2, &[], &[]);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, "a");
        assert_eq!(results[1].path, "b");
    }

    #[test]
    fn grid_hash_within_radius() {
        let entities = vec![
            entity2d("close", [30.0, 0.0]),
            entity2d("far", [1000.0, 0.0]),
        ];
        let index = SpatialIndex::build_2d(entities);
        let results = index.within_radius([0.0, 0.0, 0.0], 100.0, &[], &[]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "close");
    }

    #[test]
    fn grid_hash_group_filter() {
        let mut enemy = entity2d("enemy", [10.0, 0.0]);
        enemy.class = "CharacterBody2D".into();
        enemy.groups = vec!["enemies".into()];
        let mut pickup = entity2d("pickup", [20.0, 0.0]);
        pickup.class = "Area2D".into();
        pickup.groups = vec!["pickups".into()];
        let index = SpatialIndex::build_2d(vec![enemy, pickup]);
        let results = index.nearest([0.0, 0.0, 0.0], 5, &["enemies".to_string()], &[]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "enemy");
    }

    #[test]
    fn spatial_index_3d_still_works() {
        let entities = vec![entity("a", [0.0, 0.0, 0.0]), entity("b", [5.0, 0.0, 0.0])];
        let index = SpatialIndex::build(entities);
        let results = index.nearest([0.0, 0.0, 0.0], 1, &[], &[]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "a");
    }
}
