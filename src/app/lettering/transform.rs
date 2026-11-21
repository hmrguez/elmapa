//! Label movement, resize handles and rotation gestures.
use super::{Studio, Tool};
use bevy::prelude::{Rot2, Vec2};
use elmapa::document::{
    Label, Point,
    limits::{MAX_LABEL_SIZE, MAX_LABEL_SPACING, MIN_LABEL_SIZE, MIN_LABEL_SPACING},
};

/// Screen-sized affordances use the same geometry for drawing and hit testing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Handle {
    Corner(f32, f32),
    Rotate,
}
pub fn selection_size(label: &Label, zoom: f32) -> Vec2 {
    let mut flat = label.clone();
    flat.position = Point::new(0., 0.);
    flat.rotation = 0.;
    let bounds = flat.bounds();
    (Vec2::new(bounds.width(), bounds.height()) * zoom + Vec2::splat(12.)).max(Vec2::splat(20.))
}
pub(super) fn vector(p: Point) -> Vec2 {
    Vec2::new(p.x, p.y)
}
pub(super) fn map_point(p: Vec2) -> Point {
    Point::new(p.x, p.y)
}
#[derive(Clone)]
pub struct TransformDrag {
    pub handle: Handle,
    start: Point,
    original: Label,
}
impl TransformDrag {
    fn preview(&self, point: Point, zoom: f32, snap: bool) -> Label {
        let mut label = self.original.clone();
        let center = vector(label.position);
        let start = vector(self.start);
        let point = vector(point);
        match self.handle {
            Handle::Rotate => {
                let from = start - center;
                let to = point - center;
                let delta = (to.y.atan2(to.x) - from.y.atan2(from.x)).to_degrees();
                let angle = label.rotation + delta;
                let angle = if snap {
                    (angle / 15.).round() * 15.
                } else {
                    angle
                };
                label.rotation = (angle + 180.).rem_euclid(360.) - 180.;
            }
            Handle::Corner(x, y) => {
                let rotation = Rot2::degrees(label.rotation);
                // Geometry, rather than the padded screen handle, determines the
                // fixed opposite corner. Subtracting start also avoids a jump.
                let extent =
                    ((selection_size(&label, zoom) - Vec2::splat(12.)) / zoom).max(Vec2::splat(1.));
                let diagonal = rotation * (extent * Vec2::new(x, y));
                let ratio = (1. + (point - start).dot(diagonal) / diagonal.length_squared())
                    .clamp(MIN_LABEL_SIZE / label.size, MAX_LABEL_SIZE / label.size);
                let anchor = center - diagonal / 2.;
                label.position = map_point(anchor + diagonal * ratio / 2.);
                label.size *= ratio;
                label.spacing = (label.spacing * ratio).clamp(MIN_LABEL_SPACING, MAX_LABEL_SPACING);
            }
        }
        label
    }
}
impl Studio {
    pub(in crate::app) fn begin_text_transform(&mut self, point: Point) -> bool {
        let Some(handle) = self.text_handle(point) else {
            return false;
        };
        self.text.transforming = Some(TransformDrag {
            handle,
            start: point,
            original: self.current_label(),
        });
        true
    }

    pub fn text_move(&mut self, point: Point) {
        if let Some(drag) = &self.text.transforming {
            let label = drag.preview(point, self.viewport.zoom, self.text.snap);
            if label.validate().is_ok() {
                self.text.preview = Some(label);
                self.invalidate_preview();
            }
            return;
        }
        let Some((start, original)) = &self.text.moving else {
            return;
        };
        let mut label = original.clone();
        label.position.x += point.x - start.x;
        label.position.y += point.y - start.y;
        if label.validate().is_ok() {
            self.text.preview = Some(label);
            self.invalidate_preview();
        }
    }
    pub fn finish_text_move(&mut self) {
        let moving = self.text.moving.take().is_some();
        let transforming = self.text.transforming.take().is_some();
        if !moving && !transforming {
            return;
        }
        self.ui_dirty = true;
        if let Some(label) = self.text.preview.take()
            && self.update_selected_lettering(label)
        {
            self.changed();
        }
    }
    pub fn text_handle(&self, point: Point) -> Option<Handle> {
        if self.tool != Tool::Select || !self.has_text_selection() {
            return None;
        }
        let label = self.current_label();
        if label.text.trim().is_empty() {
            return None;
        }
        let zoom = self.viewport.transform(self.bounds).scale;
        let half = selection_size(&label, zoom) / 2.;
        let local =
            Rot2::degrees(-label.rotation) * (vector(point) - vector(label.position)) * zoom;
        let side = if self.name_above_landmark(&label) {
            -1.
        } else {
            1.
        };
        if local.distance(Vec2::new(0., side * (half.y + 28.))) <= 11. {
            return Some(Handle::Rotate);
        }
        for (x, y) in [(-1., -1.), (1., -1.), (1., 1.), (-1., 1.)] {
            if local.distance(half * Vec2::new(x, y)) <= 10. {
                return Some(Handle::Corner(x, y));
            }
        }
        None
    }
}
