use super::{Editor, Stroke};
use crate::document::limits::{MAX_STROKE_POINTS, MAX_STROKES};

impl Editor {
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
