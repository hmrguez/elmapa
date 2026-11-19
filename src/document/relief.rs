//! Sparse placements for individual specimens from a seeded brush stroke.
use super::{Layer, Point};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Specimen {
    Tree { dab: usize, candidate: usize },
    Peak(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReliefKey {
    pub stroke: usize,
    pub specimen: Specimen,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReliefMove {
    pub key: ReliefKey,
    pub position: Point,
    /// Erasers drawn after this placement apply at the destination.
    pub after_strokes: usize,
}

impl Layer {
    pub fn relief_move(&self, key: ReliefKey) -> Option<ReliefMove> {
        let index = self
            .relief_moves
            .binary_search_by_key(&key, |item| item.key)
            .ok()?;
        Some(self.relief_moves[index])
    }

    pub(crate) fn move_relief(&mut self, placements: impl IntoIterator<Item = (ReliefKey, Point)>) {
        let mut moves = self
            .relief_moves
            .iter()
            .map(|item| (item.key, *item))
            .collect::<std::collections::BTreeMap<_, _>>();
        for (key, position) in placements {
            moves.insert(
                key,
                ReliefMove {
                    key,
                    position,
                    after_strokes: self.strokes.len(),
                },
            );
        }
        self.relief_moves = std::sync::Arc::new(moves.into_values().collect());
    }
}
