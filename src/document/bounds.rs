use super::{Brush, Document, Layer, Point, Stroke};

/// A finite frame around artwork, never an editing boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContentBounds {
    pub min: Point,
    pub max: Point,
}

impl ContentBounds {
    pub fn width(self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn padded(self, margin: f32) -> Self {
        Self {
            min: Point::new(self.min.x - margin, self.min.y - margin),
            max: Point::new(self.max.x + margin, self.max.y + margin),
        }
    }

    pub fn union(self, other: Self) -> Self {
        Self {
            min: Point::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: Point::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    pub fn intersects(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }
}

impl Stroke {
    /// Includes procedural roughness, mountain shading, and outline overhang.
    pub fn bounds(&self) -> Option<ContentBounds> {
        self.points.point_bounds().map(|bounds| {
            bounds.padded(
                self.size
                    * if self.brush == Brush::Mountain {
                        1.3
                    } else {
                        1.
                    }
                    + 32.0,
            )
        })
    }
}

impl Layer {
    /// Bounds of the visible artwork on this layer.
    pub fn content_bounds(&self) -> Option<ContentBounds> {
        if !self.visible || self.opacity <= 0. {
            return None;
        }
        self.strokes
            .iter()
            .filter(|stroke| stroke.brush != Brush::Erase && stroke.strength > 0.)
            .filter_map(Stroke::bounds)
            .chain(
                self.labels
                    .iter()
                    .filter(|label| label.opacity > 0.)
                    .map(super::Label::bounds),
            )
            .chain(self.landmarks.iter().map(super::Landmark::bounds))
            .reduce(ContentBounds::union)
    }
}

impl Document {
    /// Legacy dimensions provide an initial view for empty worlds only.
    pub fn content_bounds(&self) -> ContentBounds {
        self.layers
            .iter()
            .filter_map(Layer::content_bounds)
            .reduce(ContentBounds::union)
            .unwrap_or(ContentBounds {
                min: Point::new(0.0, 0.0),
                max: Point::new(self.width, self.height),
            })
    }
}
