use super::{Brush, Document, Layer, LinearStrokeId, ReliefKey, Specimen};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectionId {
    Label {
        layer: u64,
        id: u64,
    },
    Landmark {
        layer: u64,
        id: u64,
    },
    Course(LinearStrokeId),
    Relief {
        layer: u64,
        key: ReliefKey,
        seed: u64,
    },
}

impl SelectionId {
    pub(crate) fn layer(self) -> u64 {
        match self {
            Self::Label { layer, .. }
            | Self::Landmark { layer, .. }
            | Self::Relief { layer, .. } => layer,
            Self::Course(id) => id.layer,
        }
    }

    pub fn is_visible(self, document: &Document) -> bool {
        let Some(layer) = document
            .layer(self.layer())
            .filter(|l| l.visible && l.opacity > 0.)
        else {
            return false;
        };
        match self {
            Self::Label { id, .. } => layer
                .labels
                .iter()
                .any(|label| label.id == id && label.opacity > 0.),
            Self::Landmark { id, .. } => layer.landmarks.iter().any(|landmark| landmark.id == id),
            Self::Course(id) => editable_stroke(layer, id.index, id.seed, |brush| {
                matches!(brush, Brush::River | Brush::Path)
            }),
            Self::Relief { key, seed, .. } => editable_stroke(layer, key.stroke, seed, |brush| {
                brush
                    == match key.specimen {
                        Specimen::Tree { .. } => Brush::Forest,
                        Specimen::Peak(_) => Brush::Mountain,
                    }
            }),
        }
    }
}

fn editable_stroke(
    layer: &Layer,
    index: usize,
    seed: u64,
    accepts: impl FnOnce(Brush) -> bool,
) -> bool {
    layer
        .strokes
        .get(index)
        .is_some_and(|stroke| stroke.strength > 0. && stroke.seed == seed && accepts(stroke.brush))
}
