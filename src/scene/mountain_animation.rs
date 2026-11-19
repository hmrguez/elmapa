//! Presentation-only orogeny. Geometry, persistence and SVG stay fully raised.
use super::Scene;
use crate::document::Point;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub type MountainId = (u64, usize, u64, usize);
#[derive(Clone, Debug, PartialEq)]
pub struct MountainPart {
    pub id: MountainId,
    pub base: Point,
    pub side: f32,
    pub width: f32,
    pub bounds: crate::document::ContentBounds,
}
#[derive(Clone, Default)]
pub struct MountainAnimation {
    present: Arc<HashSet<MountainId>>,
    starts: HashMap<MountainId, f32>,
}
impl MountainAnimation {
    pub fn reset(&mut self) {
        self.starts.clear();
    }
    pub fn sync(&mut self, scene: &Scene, animate: bool, now: f32) {
        let present: HashSet<_> = scene.mountain_parts().map(|p| p.id).collect();
        self.starts
            .retain(|id, start| present.contains(id) && now - *start < 0.32);
        if animate {
            let mut new: Vec<_> = present.difference(&self.present).copied().collect();
            new.sort_unstable();
            for id in new
                .into_iter()
                .take(2048_usize.saturating_sub(self.starts.len()))
            {
                self.starts.insert(id, now);
            }
        }
        self.present = Arc::new(present);
    }
    pub fn advance(&mut self, now: f32) {
        self.starts.retain(|_, start| now - *start < 0.32);
    }
    /// Vertical uplift and opposing flank displacement, in world units.
    pub fn pose(&self, part: &MountainPart, now: f32) -> (f32, f32) {
        let Some(start) = self.starts.get(&part.id) else {
            return (1., 0.);
        };
        Self::entrance(part, now - start)
    }

    pub fn entrance(part: &MountainPart, elapsed: f32) -> (f32, f32) {
        let t = (elapsed / 0.32).clamp(0., 1.);
        let rise = ((t - 0.09) / 0.91).clamp(0., 1.);
        let height = 0.06 + 0.94 * (rise * rise * (3. - 2. * rise));
        let press = (t / 0.55).min(1.);
        let shift = part.side * (part.width * 0.045).min(2.5) * (1. - press).powi(2);
        (height, shift)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{Brush, Document, Stroke},
        scene::SceneCache,
    };
    #[test]
    fn uplift_is_grounded_subtle_and_survives_release_without_replaying() {
        let mut doc = Document::blank();
        let mut cache = SceneCache::default();
        let mut motion = MountainAnimation::default();
        motion.sync(&cache.render(&doc, None), false, 0.);
        let s = Stroke {
            creation_id: None,
            material: None,
            brush: Brush::Mountain,
            points: vec![Point::new(100., 100.)].into(),
            size: 100.,
            strength: 0.8,
            seed: 9,
        };
        let scene = cache.render(&doc, Some((1, &s)));
        motion.sync(&scene, true, 0.);
        let parts: Vec<_> = scene.mountain_parts().cloned().collect();
        assert_eq!(parts.len(), 2);
        let left = parts.iter().find(|p| p.side < 0.).unwrap();
        let right = parts.iter().find(|p| p.side > 0.).unwrap();
        assert_eq!(left.base, right.base);
        assert_eq!(motion.pose(left, 0.).0, 0.06);
        assert!(motion.pose(left, 0.).1 < 0. && motion.pose(right, 0.).1 > 0.);
        let mut previous = 0.;
        for i in 0..=320 {
            let (height, shift) = motion.pose(left, i as f32 / 1000.);
            assert!(height >= previous && height <= 1.);
            assert!(shift.abs() <= 2.5);
            previous = height;
        }
        doc.layers[0].strokes.push(s);
        motion.sync(&cache.render(&doc, None), false, 0.1);
        assert!(motion.pose(left, 0.16).0 < 1.);
        motion.advance(0.33);
        motion.sync(&scene, true, 0.4);
        assert_eq!(motion.pose(left, 0.4), (1., 0.));
        doc.layers[0].strokes.clear();
        motion.sync(&cache.render(&doc, None), false, 0.4);
        assert!(motion.starts.is_empty());
        motion.sync(&scene, false, 0.5); // opening / undo / reduced motion
        assert_eq!(motion.pose(left, 0.5), (1., 0.));
    }
}
