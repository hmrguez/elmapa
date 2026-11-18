//! Presentation-only, paced opening of the road in the direction it was brushed.
use super::Scene;
use crate::document::Point;
use std::collections::{HashMap, HashSet};

pub type PathId = (u64, usize, u64, usize);
#[derive(Clone, Debug, PartialEq)]
pub struct PathPart {
    pub id: PathId,
    pub start: Point,
    pub end: Point,
}
#[derive(Clone, Default)]
pub struct PathAnimation {
    present: HashSet<PathId>,
    starts: HashMap<PathId, f32>,
}
impl PathAnimation {
    pub fn reset(&mut self) {
        self.starts.clear();
    }
    pub fn sync(&mut self, scene: &Scene, animate: bool, now: f32) {
        let present: HashSet<_> = scene.path_parts().map(|p| p.id).collect();
        self.starts
            .retain(|id, start| present.contains(id) && now - *start < 0.36);
        if animate {
            let mut new: Vec<_> = present.difference(&self.present).copied().collect();
            new.sort_unstable();
            for (step, id) in new
                .into_iter()
                .take(4096_usize.saturating_sub(self.starts.len()))
                .enumerate()
            {
                // A bounded queue keeps even a very fast drag close to the pointer.
                self.starts.insert(
                    id,
                    now + (step as f32 * 0.024 + (step % 2) as f32 * 0.004).min(0.18),
                );
            }
        }
        self.present = present;
    }
    pub fn advance(&mut self, now: f32) {
        self.starts.retain(|_, start| now - *start < 0.36);
    }
    /// Forward reveal first, then the verges gently part as if worn by passage.
    pub fn pose(&self, part: &PathPart, now: f32) -> (f32, f32) {
        let Some(start) = self.starts.get(&part.id) else {
            return (1., 1.);
        };
        Self::entrance(now - start)
    }

    pub fn entrance(elapsed: f32) -> (f32, f32) {
        let t = (elapsed / 0.36).clamp(0., 1.);
        let forward = 1. - (1. - (t / 0.72).min(1.)).powi(3);
        let opening = ((t - 0.08) / 0.92).clamp(0., 1.);
        let across = opening * opening * (3. - 2. * opening);
        (forward, across)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{Brush, Document, Stroke},
        scene::{SceneCache, to_svg},
    };
    #[test]
    fn road_opens_in_order_survives_release_and_never_changes_export() {
        let mut doc = Document::blank();
        let mut cache = SceneCache::default();
        let mut motion = PathAnimation::default();
        motion.sync(&cache.render(&doc, None), false, 0.);
        let mut stroke = Stroke {
            creation_id: None,
            material: None,
            brush: Brush::Path,
            points: vec![Point::new(0., 0.), Point::new(80., 0.)].into(),
            size: 100.,
            strength: 0.85,
            seed: 8,
        };
        let live = cache.render(&doc, Some((1, &stroke)));
        motion.sync(&live, true, 1.);
        let mut parts: Vec<_> = live.path_parts().cloned().collect();
        parts.sort_by_key(|p| p.id);
        assert_eq!(parts.len(), 10);
        assert_eq!(motion.pose(&parts[0], 1.), (0., 0.));
        assert!(motion.pose(&parts[0], 1.12).0 > motion.pose(&parts[4], 1.12).0);
        let mut prior = (0., 0.);
        for i in 0..=360 {
            let pose = motion.pose(&parts[0], 1. + i as f32 / 1000.);
            assert!(pose.0 >= prior.0 && pose.1 >= prior.1 && pose.0 <= 1. && pose.1 <= 1.);
            prior = pose;
        }
        stroke.points.push_point(Point::new(120., 20.));
        let extended = cache.render(&doc, Some((1, &stroke)));
        motion.sync(&extended, true, 1.1);
        assert_eq!(motion.starts[&parts[0].id], 1.);
        doc.layers[0].strokes.push(stroke);
        let svg = to_svg(&doc);
        motion.sync(&cache.render(&doc, None), false, 1.2);
        assert!(motion.pose(&parts[4], 1.2).1 < 1.);
        motion.advance(2.);
        assert_eq!(motion.pose(&parts[4], 2.), (1., 1.));
        assert_eq!(to_svg(&doc), svg);
        motion.sync(&extended, true, 2.);
        assert!(motion.starts.is_empty(), "settled road must not replay");
        motion.reset();
        motion.sync(&extended, false, 3.);
        assert!(
            extended
                .path_parts()
                .all(|p| motion.pose(p, 3.) == (1., 1.))
        );
        doc.layers[0].strokes.clear();
        motion.sync(&cache.render(&doc, None), false, 3.);
        assert!(motion.present.is_empty());
        motion.sync(&extended, false, 4.); // open / redo / reduced motion
        assert!(motion.starts.is_empty());
    }
}
