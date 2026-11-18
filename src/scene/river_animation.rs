//! One advancing stream front per painted stroke, independent of export.
use super::{RiverReach, Scene};
use std::collections::HashMap;

type StreamId = (u64, usize, u64);
const SETTLE: f32 = 0.58;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RiverFlow {
    pub front: f32,
    pub energy: f32,
    pub tip: f32,
}
#[derive(Clone, Copy)]
struct Stream {
    from: f32,
    target: f32,
    updated: f32,
    duration: f32,
}
impl Stream {
    fn front(self, now: f32) -> f32 {
        let t = ((now - self.updated) / self.duration).clamp(0., 1.);
        self.from + (self.target - self.from) * t * (2. - t)
    }
    fn energy(self, now: f32) -> f32 {
        let t = ((now - self.updated - 0.12) / (SETTLE - 0.12)).clamp(0., 1.);
        1. - t * t * (3. - 2. * t)
    }
}
#[derive(Clone, Default)]
pub struct RiverAnimation {
    present: HashMap<StreamId, f32>,
    streams: HashMap<StreamId, Stream>,
}
fn stream_id(reach: &RiverReach) -> StreamId {
    (reach.id.0, reach.id.1, reach.id.2)
}
impl RiverAnimation {
    pub fn reset(&mut self) {
        self.streams.clear();
    }
    pub fn sync(&mut self, scene: &Scene, animate: bool, now: f32) {
        let mut present = HashMap::<StreamId, f32>::new();
        for reach in scene.river_reaches() {
            present
                .entry(stream_id(reach))
                .and_modify(|end| *end = end.max(reach.distance + reach.length))
                .or_insert(reach.distance + reach.length);
        }
        self.streams
            .retain(|id, s| present.contains_key(id) && now - s.updated < SETTLE);
        if animate {
            let mut changed: Vec<_> = present
                .iter()
                .filter(|(id, end)| self.present.get(id).is_none_or(|old| **end > *old + 0.05))
                .collect();
            changed.sort_unstable_by_key(|(id, _)| **id);
            for (&id, &end) in changed {
                if self.streams.len() >= 256 && !self.streams.contains_key(&id) {
                    continue;
                }
                let from = self.streams.get(&id).map_or_else(
                    || self.present.get(&id).map_or(-5., |end| end + 3.),
                    |s| s.front(now),
                );
                let target = end + 3.;
                self.streams.insert(
                    id,
                    Stream {
                        from,
                        target,
                        updated: now,
                        duration: ((target - from).max(0.) / 900.).clamp(0.04, 0.12),
                    },
                );
            }
        }
        self.present = present;
    }
    /// Reaches behind the short wake immediately use the completed engraving.
    pub fn pose(&self, reach: &RiverReach, now: f32) -> Option<RiverFlow> {
        stream_pose(*self.streams.get(&stream_id(reach))?, reach, now)
    }

    pub fn replay_pose(
        reach: &RiverReach,
        first: f32,
        last: f32,
        elapsed: f32,
    ) -> Option<RiverFlow> {
        stream_pose(
            Stream {
                from: first - 5.,
                target: last + 3.,
                updated: 0.,
                duration: 0.44,
            },
            reach,
            elapsed,
        )
    }
    pub fn advance(&mut self, now: f32) {
        self.streams.retain(|_, s| now - s.updated < SETTLE);
    }
}

fn stream_pose(stream: Stream, reach: &RiverReach, now: f32) -> Option<RiverFlow> {
    if now - stream.updated >= SETTLE {
        return None;
    }
    let front = stream.front(now);
    (reach.distance + reach.length >= front - 24.).then_some(RiverFlow {
        front,
        energy: stream.energy(now),
        tip: stream.target - 3.,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{Brush, Document, Point, Stroke},
        scene::SceneCache,
    };
    #[test]
    fn one_front_crosses_reaches_retargets_continuously_and_settles_after_release() {
        let mut doc = Document::blank();
        let mut cache = SceneCache::default();
        let mut motion = RiverAnimation::default();
        motion.sync(&cache.render(&doc, None), false, 0.);
        let mut stroke = Stroke {
            creation_id: None,
            material: None,
            brush: Brush::River,
            points: vec![Point::new(100., 200.), Point::new(250., 200.)].into(),
            size: 110.,
            strength: 0.85,
            seed: 17,
        };
        let first = cache.render(&doc, Some((1, &stroke)));
        motion.sync(&first, true, 1.);
        let reaches: Vec<_> = first.river_reaches().collect();
        let head = reaches.iter().min_by_key(|r| r.id).unwrap();
        let tail = reaches.iter().max_by_key(|r| r.id).unwrap();
        assert_eq!(motion.pose(head, 1.).unwrap().front, -5.);
        let mut previous = -5.;
        for i in 0..10 {
            let now = 1. + i as f32 * 0.01;
            let front = motion.pose(tail, now).unwrap().front;
            assert!(front >= previous);
            for reach in &reaches {
                if let Some(pose) = motion.pose(reach, now) {
                    assert_eq!(pose.front, front);
                }
            }
            previous = front;
        }
        let visible = motion.pose(tail, 1.1).unwrap().front;
        stroke.points.push(Point::new(400., 300.));
        let extended = cache.render(&doc, Some((1, &stroke)));
        motion.sync(&extended, true, 1.1);
        let last = extended.river_reaches().max_by_key(|r| r.id).unwrap();
        assert_eq!(motion.pose(last, 1.1).unwrap().front, visible);
        doc.layers[0].strokes.push(stroke);
        let released = cache.render(&doc, None);
        let pose = motion.pose(last, 1.15);
        motion.sync(&released, false, 1.15);
        assert_eq!(motion.pose(last, 1.15), pose);
        assert!(motion.pose(last, 1.4).unwrap().front > last.distance + last.length);
        assert!(motion.pose(last, 1.4).unwrap().energy < 1.);
        motion.advance(2.);
        motion.sync(&released, true, 2.);
        assert!(motion.pose(last, 2.).is_none());
        motion.reset();
        assert!(motion.streams.is_empty());
        doc.layers[0].strokes.clear();
        motion.sync(&cache.render(&doc, None), false, 3.);
        assert!(motion.present.is_empty());
        motion.sync(&released, false, 4.);
        assert!(motion.streams.is_empty());
    }
}
