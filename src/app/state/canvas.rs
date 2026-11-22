//! Convert canvas gestures into viewport movement and document edits.
use super::{Studio, Tool};
use bevy::prelude::Vec2;
use elmapa::document::{
    Brush, Point, Stroke,
    limits::{MAX_STROKE_POINTS, MAX_STROKES, MAX_WORLD_COORDINATE},
};

impl Studio {
    pub fn finish(&mut self) {
        self.finish_selection_move();
        self.finish_path_drag();
        self.finish_text_move();
        self.finish_landmark_move();
        self.drag = None;
        if let Some(stroke) = self.stroke.take() {
            let wet = self.shader_motion() && matches!(stroke.brush, Brush::Land | Brush::Water);
            let layer = self.editor.active_layer();
            let id = self.editor.document().next_id;
            if self.editor.commit_stroke(stroke) {
                self.changed();
                self.wet_release = wet.then_some((layer, id));
                self.status = "Painted your world.".into();
            } else {
                self.status = "This stroke exceeds the map’s geometry limit.".into();
                self.invalidate();
            }
        }
    }
    pub fn extend(&mut self, point: Point, spacing: f32) {
        let remaining = MAX_STROKE_POINTS.saturating_sub(self.editor.point_count());
        if point.is_in_world()
            && let Some(stroke) = &mut self.stroke
            && stroke.points.len() < remaining
            && stroke
                .points
                .last()
                .is_none_or(|p| (p.x - point.x).hypot(p.y - point.y) > spacing)
        {
            stroke.points.push_point(point);
            self.invalidate();
        }
    }
    pub fn down(&mut self, point: Vec2) {
        self.down_with_shift(point, false);
    }

    pub fn down_with_shift(&mut self, point: Vec2, shift: bool) {
        self.pointer = Some(point);
        if self.home || self.installing || self.opening.is_some() {
            return;
        }
        if self.stop_view_replay() {
            return;
        }
        if self.menu {
            self.menu = false;
            self.ui_dirty = true;
            return;
        }
        self.viewport.resolve(self.bounds);
        let point = self.map_point(point);
        if self.regions.active {
            self.region_down(point);
            return;
        }
        if self.annotation_press(point, shift) {
            return;
        }
        if !self
            .editor
            .document()
            .layer(self.editor.active_layer())
            .is_some_and(|l| l.visible && l.territories.is_none())
        {
            self.status = "Show this layer before painting on it.".into();
            self.ui_dirty = true;
            return;
        }
        if self.editor.stroke_count() >= MAX_STROKES
            || self.editor.point_count() >= MAX_STROKE_POINTS
        {
            return;
        }
        if point.x.abs() > MAX_WORLD_COORDINATE || point.y.abs() > MAX_WORLD_COORDINATE {
            return;
        }
        self.wet_release = None;
        self.stroke = Some(Stroke {
            creation_id: Some(self.editor.document().next_id),
            material: self.brush_orbit.stroke_material(self.brush),
            brush: self.brush,
            points: vec![point].into(),
            size: self.radius,
            strength: self.strength,
            seed: self
                .editor
                .document()
                .next_id
                .wrapping_mul(93241)
                .wrapping_add((self.editor.stroke_count() as u64).wrapping_mul(7817)),
        });
        self.animate_new = matches!(
            self.brush,
            Brush::Forest | Brush::Mountain | Brush::Path | Brush::River
        );
        self.invalidate();
    }

    pub(in crate::app) fn wet_paint(&self) -> Option<(u64, u64)> {
        if !self.shader_motion()
            || self.home
            || self.installing
            || self.opening.is_some()
            || self.replay_view.is_some()
        {
            return None;
        }
        if let Some(stroke) = &self.stroke {
            return matches!(stroke.brush, Brush::Land | Brush::Water)
                .then_some((self.editor.active_layer(), stroke.creation_id?));
        }
        self.wet_release
    }
    pub fn begin_pan(&mut self, point: Vec2) {
        if self.home || self.installing || self.opening.is_some() {
            return;
        }
        self.stop_view_replay();
        self.close_brush_orbit();
        self.finish();
        self.regions.cancel_gesture();
        self.cancel_selection_gesture();
        self.viewport.resolve(self.bounds);
        self.drag = Some(point);
    }
    pub fn moved(&mut self, point: Vec2, blocked: bool) {
        self.pointer = (!blocked).then_some(point);
        if let Some(last) = self.drag {
            if !blocked {
                self.viewport.pan.x += point.x - last.x;
                self.viewport.pan.y += point.y - last.y;
            }
            self.drag = Some(point);
            return;
        }
        if self.regions.active {
            if !blocked {
                self.region_move(self.map_point(point));
            }
            return;
        }
        if self.selection.dragging() {
            self.move_selection_gesture(self.map_point(point));
            return;
        }
        if self.path_dragging() {
            self.move_path_node(self.map_point(point));
            return;
        }
        if blocked && self.text.transforming.is_none() {
            return;
        }
        if self.landmarks.moving.is_some() {
            self.landmark_move(self.map_point(point));
            return;
        }
        if self.tool != Tool::Brush {
            self.text_move(self.map_point(point));
            return;
        }
        self.extend(self.map_point(point), 3.);
    }
    pub fn up(&mut self, point: Vec2, blocked: bool) {
        if self.regions.active {
            let revision = self.editor.revision();
            self.region_up(self.map_point(point), blocked);
            if self.editor.revision() != revision {
                self.changed();
            }
            return;
        }
        if self.selection.dragging() {
            self.move_selection_gesture(self.map_point(point));
            self.finish_marquee();
            self.finish();
            return;
        }
        if self.path_dragging() {
            self.move_path_node(self.map_point(point));
            self.finish();
            return;
        }
        if self.landmarks.moving.is_some() {
            if !blocked {
                self.landmark_move(self.map_point(point));
            }
            self.finish();
            return;
        }
        if self.tool != Tool::Brush {
            if !blocked || self.text.transforming.is_some() {
                self.text_move(self.map_point(point));
            }
            self.finish();
            return;
        }
        if !blocked {
            self.extend(self.map_point(point), 0.);
        }
        self.finish();
    }
}
