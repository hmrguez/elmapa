//! Select and move artwork across layers, with contextual single-item editing.
mod paths;
use super::state::{Studio, Tool};
use bevy::prelude::{Rect, Vec2};
use elmapa::{
    document::{ContentBounds, Document, Point, SelectionId, limits::MAX_WORLD_COORDINATE},
    scene::TerrainHit,
};
pub(super) use paths::PathSelection;
use std::collections::HashSet;

type Item = TerrainHit;

#[derive(Default)]
pub(super) struct MapSelection {
    pub items: Vec<Item>,
    marquee: Option<Marquee>,
    movement: Option<Movement>,
}

struct Marquee {
    start: Point,
    end: Point,
    extend: bool,
}

struct Movement {
    start: Point,
    delta: Point,
    previewed: bool,
    range: ContentBounds,
    targets: Vec<(SelectionId, Point)>,
}

fn vector(point: Point) -> Vec2 {
    Vec2::new(point.x, point.y)
}

fn frame(a: Point, b: Point) -> ContentBounds {
    ContentBounds {
        min: Point::new(a.x.min(b.x), a.y.min(b.y)),
        max: Point::new(a.x.max(b.x), a.y.max(b.y)),
    }
}

impl MapSelection {
    pub fn delta(&self) -> Point {
        self.movement
            .as_ref()
            .map_or(Point::new(0., 0.), |drag| drag.delta)
    }

    pub fn dragging(&self) -> bool {
        self.movement.is_some() || self.marquee.is_some()
    }

    pub fn frame(&self) -> Option<ContentBounds> {
        self.items
            .iter()
            .map(|item| item.bounds)
            .reduce(ContentBounds::union)
    }

    pub fn marquee(&self, scale: f32) -> Option<ContentBounds> {
        let drag = self.marquee.as_ref()?;
        ((vector(drag.start) - vector(drag.end)).length_squared() * scale * scale > 25.)
            .then(|| frame(drag.start, drag.end))
    }
}

impl Studio {
    pub(super) fn annotation_press(&mut self, point: Point, shift: bool) -> bool {
        if std::mem::take(&mut self.text.style_open) {
            self.ui_dirty = true;
            return true;
        }
        match self.tool {
            Tool::Brush => return false,
            Tool::Landmarks => self.place_landmark(point, shift),
            Tool::Text => self.place_text(point),
            Tool::Select => {
                if self.finish_label_draft() {
                    self.commit_text_field();
                    self.dismiss_text_control();
                    self.select_press(point, shift);
                    self.sound.cue = Some(super::sound::Effect::Select);
                }
            }
        }
        self.ui_dirty = true;
        true
    }

    fn select_press(&mut self, point: Point, shift: bool) {
        if !shift
            && (self.begin_path_drag(point)
                || self.begin_text_transform(point)
                || self.begin_landmark_name_move(point))
        {
            return;
        }
        if !shift
            && self.group_grip().is_some_and(|grip| {
                let view = self.viewport.transform(self.bounds);
                grip.distance_squared(view.origin + vector(point) * view.scale) <= 14_f32.powi(2)
            })
        {
            self.begin_selection_move(point);
            return;
        }
        let hit = self.pick_item(point);
        if shift {
            if let Some(item) = hit {
                let mut items = self.selected_items();
                if let Some(index) = items.iter().position(|selected| selected.id == item.id) {
                    items.remove(index);
                } else {
                    items.push(item);
                }
                self.set_map_selection(items, point, false);
            } else {
                self.start_marquee(point, true);
            }
        } else if let Some(item) = hit {
            if !self
                .selection
                .items
                .iter()
                .any(|selected| selected.id == item.id)
            {
                self.set_map_selection(vec![item], point, true);
            }
            self.begin_selection_move(point);
        } else {
            self.set_map_selection(Vec::new(), point, false);
            self.start_marquee(point, false);
        }
    }

    fn start_marquee(&mut self, point: Point, extend: bool) {
        self.selection.marquee = Some(Marquee {
            start: point,
            end: point,
            extend,
        });
    }

    pub fn group_grip(&self) -> Option<Vec2> {
        let view = self.viewport.transform(self.bounds);
        let anchor = if let Some(path) = &self.path_selection {
            if path.active_node().is_some() {
                return None;
            }
            let target = vector(path.anchor);
            path.nodes().map(vector).min_by(|a, b| {
                a.distance_squared(target)
                    .total_cmp(&b.distance_squared(target))
            })?
        } else {
            let bounds = self.selection.frame()?;
            Vec2::new((bounds.min.x + bounds.max.x) / 2., bounds.min.y)
        };
        let p = view.origin + (vector(self.selection.delta()) + anchor) * view.scale
            - Vec2::new(0., 22.);
        Rect::from_corners(Vec2::splat(15.), self.bounds.size - Vec2::splat(15.))
            .contains(p)
            .then_some(p)
    }

    pub(super) fn has_selection(&self) -> bool {
        !self.selection.items.is_empty()
            || self.text.selected.is_some()
            || self.landmarks.selected.is_some()
    }

    fn selected_items(&self) -> Vec<Item> {
        if !self.selection.items.is_empty() {
            return self.selection.items.clone();
        }
        self.editor
            .document()
            .layers
            .iter()
            .find_map(|layer| {
                if let Some(id) = self.text.selected
                    && layer.labels.iter().any(|label| label.id == id)
                {
                    return self.item(SelectionId::Label {
                        layer: layer.id,
                        id,
                    });
                }
                if let Some(id) = self.landmarks.selected
                    && layer.landmarks.iter().any(|landmark| landmark.id == id)
                {
                    return self.item(SelectionId::Landmark {
                        layer: layer.id,
                        id,
                    });
                }
                None
            })
            .into_iter()
            .collect()
    }

    fn item(&self, id: SelectionId) -> Option<Item> {
        let (anchor, bounds) = match id {
            SelectionId::Label { id, .. } => {
                let label = self.editor.label(id)?;
                (label.position, label.bounds())
            }
            SelectionId::Landmark { id, .. } => {
                let landmark = self.editor.landmark(id)?;
                (landmark.position, landmark.bounds())
            }
            SelectionId::Course(id) => {
                let stroke = self.editor.linear_stroke(id)?;
                (
                    stroke.points[0],
                    stroke.points.point_bounds()?.padded(stroke.size * 0.5),
                )
            }
            SelectionId::Relief { .. } => return None,
        };
        Some(Item { id, anchor, bounds })
    }

    fn pick_item(&self, point: Point) -> Option<Item> {
        let scale = self.viewport.transform(self.bounds).scale;
        let landmark = self.editor.landmark_at(point, 4. / scale);
        let label = if landmark.is_some() && self.landmarks.selected.is_some() {
            None
        } else {
            self.editor.label_at(point, 6. / scale)
        };
        let mut hit = label
            .and_then(|(layer, id)| self.item(SelectionId::Label { layer, id }))
            .or_else(|| {
                landmark.and_then(|(layer, id)| self.item(SelectionId::Landmark { layer, id }))
            });
        let scene = self.scene.snapshot();
        if hit.is_none() {
            hit = scene
                .relief_at(point, 2. / scale)
                .filter(|item| item.id.is_visible(self.editor.document()))
                .or_else(|| {
                    self.item(SelectionId::Course(
                        scene.linear_stroke_at(point, 6. / scale)?,
                    ))
                });
        }
        hit
    }

    fn set_map_selection(&mut self, mut items: Vec<Item>, point: Point, moving: bool) {
        self.clear_map_selection();
        match items.as_slice() {
            [
                Item {
                    id: SelectionId::Label { layer, id },
                    ..
                },
            ] => {
                self.cancel_landmark();
                self.select_label(point, *layer, *id);
                if !moving {
                    self.text.moving = None;
                }
            }
            [
                Item {
                    id: SelectionId::Landmark { layer, id },
                    ..
                },
            ] => {
                self.cancel_text();
                self.select_landmark(point, *layer, *id);
                if !moving || self.begin_landmark_name_move(point) {
                    self.landmarks.moving = None;
                }
            }
            _ => {
                self.cancel_text();
                self.cancel_landmark();
                if let [
                    Item {
                        id: SelectionId::Course(id),
                        ..
                    },
                ] = items.as_slice()
                {
                    self.select_path(*id, point);
                }
                self.selection.items = std::mem::take(&mut items);
            }
        }
        self.ui_dirty = true;
    }

    pub(in crate::app) fn clear_map_selection(&mut self) {
        self.cancel_selection_gesture();
        self.clear_path_selection();
        self.selection = MapSelection::default();
        self.ui_dirty = true;
    }

    pub(in crate::app) fn retain_visible_selection(&mut self) {
        if self
            .selection
            .items
            .iter()
            .all(|item| item.id.is_visible(self.editor.document()))
        {
            return;
        }
        let items = self
            .selection
            .items
            .iter()
            .copied()
            .filter(|item| item.id.is_visible(self.editor.document()))
            .collect();
        let point = self
            .pointer
            .map_or(Point::new(0., 0.), |point| self.map_point(point));
        self.set_map_selection(items, point, false);
    }

    pub(super) fn cancel_selection_gesture(&mut self) -> bool {
        let moving = self.selection.movement.take();
        let marquee = self.selection.marquee.take().is_some();
        if moving.as_ref().is_some_and(|drag| drag.previewed) {
            self.invalidate_preview();
        }
        moving.is_some() || marquee
    }

    fn begin_selection_move(&mut self, point: Point) {
        let mut range = None::<ContentBounds>;
        for item in &self.selection.items {
            let bounds = match item.id {
                SelectionId::Course(id) => self
                    .editor
                    .linear_stroke(id)
                    .and_then(|stroke| stroke.points.point_bounds()),
                SelectionId::Landmark { id, .. } => Some(frame(
                    item.anchor,
                    self.editor
                        .landmark(id)
                        .and_then(|mark| mark.name_label())
                        .map_or(item.anchor, |name| name.position),
                )),
                _ => Some(frame(item.anchor, item.anchor)),
            };
            if let Some(bounds) = bounds {
                range = Some(range.map_or(bounds, |r| r.union(bounds)));
            }
        }
        let Some(range) = range else { return };
        self.selection.movement = Some(Movement {
            start: point,
            range,
            delta: Point::new(0., 0.),
            previewed: false,
            targets: self
                .selection
                .items
                .iter()
                .map(|item| (item.id, item.anchor))
                .collect(),
        });
        self.animate_new = false;
    }

    pub(super) fn move_selection_gesture(&mut self, point: Point) {
        if let Some(marquee) = &mut self.selection.marquee {
            marquee.end = point;
            return;
        }
        let Some(drag) = &mut self.selection.movement else {
            return;
        };
        let scale = self.viewport.transform(self.bounds).scale;
        let mut d = vector(point) - vector(drag.start);
        if d.length_squared() * scale * scale < 4. {
            d = Vec2::ZERO;
        }
        let limit = Vec2::splat(MAX_WORLD_COORDINATE);
        let d = d.clamp(
            -limit - vector(drag.range.min),
            limit - vector(drag.range.max),
        );
        let delta = Point::new(d.x, d.y);
        if delta != drag.delta {
            drag.delta = delta;
            drag.previewed = true;
            self.invalidate();
        }
    }

    pub(super) fn finish_selection_move(&mut self) {
        let Some(drag) = self.selection.movement.take() else {
            return;
        };
        if self.editor.move_selection(&drag.targets, drag.delta) {
            let move_point = |point: &mut Point| {
                point.x += drag.delta.x;
                point.y += drag.delta.y;
            };
            for item in &mut self.selection.items {
                move_point(&mut item.anchor);
                move_point(&mut item.bounds.min);
                move_point(&mut item.bounds.max);
            }
            self.translate_path_selection(drag.delta);
            self.changed();
        } else if drag.previewed {
            self.invalidate_preview();
        }
    }

    pub(super) fn preview_selection(&self) -> Option<Document> {
        let drag = self.selection.movement.as_ref()?;
        (drag.delta != Point::new(0., 0.))
            .then(|| {
                self.editor
                    .preview_selection_move(&drag.targets, drag.delta)
            })
            .flatten()
    }

    pub(super) fn finish_marquee(&mut self) {
        let frame = self
            .selection
            .marquee(self.viewport.transform(self.bounds).scale);
        let Some(drag) = self.selection.marquee.take() else {
            return;
        };
        let Some(frame) = frame else { return };
        let mut items = if drag.extend {
            self.selected_items()
        } else {
            Vec::new()
        };
        let mut seen: HashSet<_> = items.iter().map(|item| item.id).collect();
        let document = self.editor.document();
        items.extend(
            self.scene
                .snapshot()
                .terrain_in(frame)
                .into_iter()
                .filter(|item| item.id.is_visible(document))
                .map(|item| self.item(item.id).unwrap_or(item))
                .filter(|item| seen.insert(item.id)),
        );
        for layer in document
            .layers
            .iter()
            .filter(|layer| layer.visible && layer.opacity > 0.)
        {
            let labels = layer
                .labels
                .iter()
                .filter(|label| label.opacity > 0.)
                .map(|label| Item {
                    id: SelectionId::Label {
                        layer: layer.id,
                        id: label.id,
                    },
                    anchor: label.position,
                    bounds: label.bounds(),
                });
            let landmarks = layer.landmarks.iter().map(|landmark| Item {
                id: SelectionId::Landmark {
                    layer: layer.id,
                    id: landmark.id,
                },
                anchor: landmark.position,
                bounds: landmark.bounds(),
            });
            items.extend(
                labels
                    .chain(landmarks)
                    .filter(|item| item.bounds.intersects(frame) && seen.insert(item.id)),
            );
        }
        self.set_map_selection(items, drag.end, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elmapa::document::{Label, Landmark};

    #[test]
    fn selecting_either_annotation_keeps_select_active() {
        let mut studio = Studio::default();
        let point = Point::new(0., 0.);
        let label = studio.editor.add_label(Label::default()).unwrap();
        let landmark = studio.editor.add_landmark(Landmark::default()).unwrap();

        studio.tool = Tool::Select;
        assert!(studio.annotation_press(point, false));
        assert_eq!(studio.text.selected, Some(label));
        assert_eq!(studio.tool, Tool::Select);
        studio.finish();

        studio.editor.delete_label(label);
        assert!(studio.annotation_press(point, false));
        assert_eq!(studio.landmarks.selected, Some(landmark));
        assert_eq!(studio.text.selected, None);
        assert_eq!(studio.tool, Tool::Select);
    }
}
