use super::{Editor, Stroke};
use crate::document::{
    Brush, LinearStrokeId, Point, SharedVec,
    limits::{MAX_STROKE_POINTS, MAX_STROKES},
};

impl Editor {
    pub fn linear_stroke(&self, id: LinearStrokeId) -> Option<&Stroke> {
        let layer = self.document.layer(id.layer)?;
        let stroke = layer.strokes.get(id.index)?;
        (layer.visible
            && layer.opacity > 0.
            && stroke.strength > 0.
            && stroke.seed == id.seed
            && matches!(stroke.brush, Brush::River | Brush::Path))
        .then_some(stroke)
    }

    pub fn update_linear_stroke_points(
        &mut self,
        id: LinearStrokeId,
        points: SharedVec<Point>,
    ) -> bool {
        let Some(stroke) = self.linear_stroke(id) else {
            return false;
        };
        let count = self.point_count - stroke.points.len() + points.len();
        if count > MAX_STROKE_POINTS
            || points.is_empty()
            || points == stroke.points
            || points.iter().any(|point| !point.is_in_world())
        {
            return false;
        }
        self.checkpoint();
        self.document
            .layer_mut(id.layer)
            .expect("checked layer")
            .strokes[id.index]
            .points = points;
        self.point_count = count;
        true
    }

    pub fn commit_stroke(&mut self, mut stroke: Stroke) -> bool {
        if stroke.validate().is_err()
            || self
                .document
                .layer(self.active_layer)
                .is_none_or(|layer| !layer.visible || layer.territories.is_some())
        {
            return false;
        }
        if self.point_count.saturating_add(stroke.points.len()) > MAX_STROKE_POINTS
            || self.stroke_count >= MAX_STROKES
        {
            return false;
        }
        let Some(id) = self.checkpoint_with_id() else {
            return false;
        };
        stroke.creation_id = Some(id);
        self.point_count += stroke.points.len();
        self.stroke_count += 1;
        self.document
            .layer_mut(self.active_layer)
            .expect("checked layer")
            .strokes
            .push(stroke);
        true
    }
}
