//! Presentation timing for growing freshly painted trees.
use super::{ForestTree, Scene, TreeId, geometry::distance};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

const GROW: f32 = 0.24;

#[derive(Clone, Default)]
pub struct ForestAnimation {
    present: Arc<HashSet<TreeId>>,
    starts: HashMap<TreeId, f32>,
}

impl ForestAnimation {
    pub fn reset(&mut self) {
        self.present = Arc::default();
        self.starts.clear();
    }

    /// Reconcile after geometry changes, including cancellation, erasing and undo.
    pub fn sync(&mut self, scene: &Scene, animate_new: bool, now: f32) {
        let present: HashSet<_> = scene.forest_trees().map(|tree| tree.id).collect();
        self.starts
            .retain(|id, start| present.contains(id) && now - *start < GROW);
        if animate_new {
            let mut new: Vec<_> = scene
                .forest_trees()
                .filter(|tree| !self.present.contains(&tree.id))
                .collect();
            new.sort_by_key(|tree| tree.id);
            let mut first = HashMap::<_, f32>::new();
            for tree in &new {
                first
                    .entry(dab(tree))
                    .and_modify(|v| *v = v.min(delay(tree)))
                    .or_insert(delay(tree));
            }
            for tree in new {
                if self.starts.len() >= 2048 {
                    break;
                }
                self.starts
                    .insert(tree.id, now + delay(tree) - first[&dab(tree)]);
            }
        }
        self.present = Arc::new(present);
    }

    pub fn advance(&mut self, now: f32) -> bool {
        self.starts.retain(|_, start| now - *start < GROW);
        !self.starts.is_empty()
    }

    pub fn visible_active(
        &self,
        scene: &Scene,
        bounds: crate::document::ContentBounds,
        now: f32,
    ) -> bool {
        scene.forest_trees().any(|tree| {
            self.starts
                .get(&tree.id)
                .is_some_and(|start| now - start < GROW)
                && crate::document::ContentBounds {
                    min: tree.center,
                    max: tree.center,
                }
                .padded(73. * tree.scale)
                .intersects(bounds)
        })
    }

    pub fn factor(&self, id: TreeId, now: f32) -> f32 {
        self.starts
            .get(&id)
            .map_or(1., |start| grow((now - start).max(0.)))
    }

    pub fn replay_factor(tree: &ForestTree, elapsed: f32) -> f32 {
        grow(elapsed - delay(tree))
    }
}

fn dab(tree: &ForestTree) -> (u64, usize, usize) {
    (tree.id.0, tree.id.1, tree.id.2)
}
fn delay(tree: &ForestTree) -> f32 {
    let id = tree.id.1 as u64 * 30_000 + tree.id.2 as u64 * 72 + tree.id.3 as u64;
    let mut tag = id ^ (id >> 32);
    tag = tag.wrapping_mul(0x9e3779b97f4a7c15);
    let jitter = (tag ^ (tag >> 32)) as u32 as f32 / u32::MAX as f32;
    (distance(tree.center, tree.origin) / tree.radius.max(1.)).clamp(0., 1.) * 0.07 + jitter * 0.012
}
fn grow(age: f32) -> f32 {
    let t = (age / GROW).clamp(0., 1.);
    if t < 0.68 {
        let q = t / 0.68;
        0.08 + 0.945 * (1. - (1. - q).powi(3))
    } else {
        let q = (t - 0.68) / 0.32;
        1.025 - 0.025 * q * q * (3. - 2. * q)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Brush, Document, Point, Stroke};
    use crate::scene::SceneCache;

    #[test]
    fn source_growth_curve_is_preserved() {
        assert_eq!(grow(0.), 0.08);
        assert!(grow(0.08) > 0.85);
        assert!(grow(0.14) > 1.);
        for n in 0..1000 {
            assert!((0.08..=1.02501).contains(&grow(n as f32 / 1000.)));
        }
        assert_eq!(grow(GROW), 1.);
    }

    #[test]
    fn stagger_survives_release_and_clears_on_cancel_without_replaying_old_trees() {
        let mut doc = Document::blank();
        let mut cache = SceneCache::default();
        let mut motion = ForestAnimation::default();
        motion.sync(&cache.render(&doc, None), false, 0.);
        let stroke = Stroke {
            creation_id: None,
            material: None,
            brush: Brush::Forest,
            points: vec![Point::new(100., 100.)].into(),
            size: 110.,
            strength: 0.58,
            seed: 9,
        };
        let scene = cache.render(&doc, Some((1, &stroke)));
        motion.sync(&scene, true, 0.);
        assert!(motion.advance(0.));
        let factors: Vec<_> = scene
            .forest_trees()
            .map(|t| motion.factor(t.id, 0.04))
            .collect();
        assert!(factors.iter().any(|f| *f > 0.08));
        assert!(factors.contains(&0.08));
        doc.layers[0].strokes.push(stroke);
        motion.sync(&cache.render(&doc, None), false, 0.04);
        assert!(motion.advance(0.1));
        assert!(!motion.advance(0.323));
        motion.sync(&scene, true, 1.);
        assert!(!motion.advance(1.));
        doc.layers[0].strokes.clear();
        motion.sync(&cache.render(&doc, None), false, 1.);
        assert!(!motion.advance(1.));
    }
    #[test]
    fn settled_controller_snapshot_reclaims_growth_capacity_before_new_gesture() {
        let mut doc = crate::performance_fixtures::fixture("dense-forest", 4).unwrap();
        let mut cache = SceneCache::default();
        let first = cache.render(&doc, None);
        let mut motion = ForestAnimation::default();
        motion.sync(&first, true, 0.);
        assert_eq!(motion.starts.len(), 2048);
        doc.layers[0].strokes.push(Stroke {
            creation_id: None,
            material: None,
            brush: Brush::Forest,
            points: vec![Point::new(-5000., -5000.)].into(),
            size: 110.,
            strength: 0.85,
            seed: 98765,
        });
        let old_ids: HashSet<_> = first.forest_trees().map(|tree| tree.id).collect();
        let next = cache.render(&doc, None);
        // Only the surface's cloned snapshot advances between publications.
        // The controller must prune its expired starts when synchronizing again.
        motion.sync(&next, true, 1.);
        assert!(
            next.forest_trees()
                .filter(|tree| !old_ids.contains(&tree.id))
                .any(|tree| motion.factor(tree.id, 1.) < 1.),
            "old settled trees exhausted the animation budget for the next gesture"
        );
        assert!(motion.starts.len() < 64);
    }
}
