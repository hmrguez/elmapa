//! Landmark placement, selection and one-checkpoint drag previews.
use super::{
    radial::Item,
    state::{Action, Studio, Tool},
};
use elmapa::{
    document::{
        Landmark, LandmarkKind, Point,
        limits::{MAX_LANDMARK_SIZE, MIN_LANDMARK_SIZE},
    },
    scene::landmark_catalog,
};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerTarget {
    Placement,
    Replacement(u64),
}

#[derive(Default)]
pub struct LandmarkTool {
    pub selected: Option<u64>,
    pub placement: Landmark,
    pub picker: Option<PickerTarget>,
    pub moving: Option<(Point, Landmark)>,
    pub preview: Option<Landmark>,
    /// Only explicit placement or changing a specimen requests motion.
    pub arrivals: HashMap<u64, Option<f32>>,
}

impl Studio {
    pub fn available_landmark(&self, kind: &LandmarkKind) -> Option<usize> {
        let catalog = landmark_catalog();
        catalog.find(kind).filter(|&index| {
            !self
                .disabled_landmark_packs
                .contains(&catalog.packs[catalog.items[index].pack].id)
        })
    }

    pub fn current_landmark(&self) -> Option<Landmark> {
        self.landmarks.preview.clone().or_else(|| {
            self.landmarks
                .selected
                .and_then(|id| self.editor.landmark(id).cloned())
        })
    }

    pub fn landmark_actions(&self) -> Vec<Item<Action>> {
        let mut change = Item::action("Change icon", "landmark", Action::LandmarkPicker);
        change.active = self.landmarks.picker.is_some();
        vec![
            change,
            Item::action("Smaller · [", "minus", Action::LandmarkSize(-16.)),
            Item::action("Larger · ]", "plus", Action::LandmarkSize(16.)),
            Item::action(
                super::input::shortcut_hint("Duplicate landmark", "D", false),
                "text-duplicate",
                Action::LandmarkDuplicate,
            ),
            Item::action(
                "Delete landmark · Backspace",
                "delete",
                Action::LandmarkDelete,
            ),
        ]
    }

    pub fn landmark_action(&mut self, action: Action) {
        match action {
            Action::LandmarkPicker => {
                self.landmarks.picker = match self.tool {
                    Tool::Landmarks => Some(PickerTarget::Placement),
                    Tool::Select => self.landmarks.selected.map(PickerTarget::Replacement),
                    _ => None,
                };
            }
            Action::CloseLandmarkPicker => self.landmarks.picker = None,
            Action::LandmarkKind(index) => {
                let Some(def) = landmark_catalog().items.get(index) else {
                    return;
                };
                if self.available_landmark(&def.kind).is_none() {
                    return;
                }
                match self.landmarks.picker.take() {
                    Some(PickerTarget::Placement) if self.tool == Tool::Landmarks => {
                        self.landmarks.placement.kind = def.kind.clone();
                    }
                    Some(PickerTarget::Replacement(id))
                        if self.tool == Tool::Select && self.landmarks.selected == Some(id) =>
                    {
                        if let Some(mut landmark) = self.editor.landmark(id).cloned() {
                            landmark.kind = def.kind.clone();
                            if self.editor.update_landmark(landmark) {
                                self.landmarks.arrivals.insert(id, None);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Action::LandmarkSize(delta) => {
                let size = |size: f32| (size + delta).clamp(MIN_LANDMARK_SIZE, MAX_LANDMARK_SIZE);
                if self.tool == Tool::Landmarks {
                    self.landmarks.placement.size = size(self.landmarks.placement.size);
                } else if let Some(mut landmark) = self.current_landmark() {
                    landmark.size = size(landmark.size);
                    self.editor.update_landmark(landmark);
                }
            }
            Action::LandmarkDuplicate => {
                if let Some(mut landmark) = self.current_landmark() {
                    landmark.position.x += 24.;
                    landmark.position.y += 24.;
                    if let Some(id) = self.editor.add_landmark(landmark) {
                        self.landmarks.selected = Some(id);
                        self.landmarks.arrivals.insert(id, None);
                    }
                }
            }
            Action::LandmarkDelete => {
                if let Some(id) = self.landmarks.selected.take() {
                    self.editor.delete_landmark(id);
                    self.landmarks.arrivals.remove(&id);
                }
            }
            _ => {}
        }
    }

    pub(super) fn place_landmark(&mut self, point: Point) {
        if self
            .available_landmark(&self.landmarks.placement.kind)
            .is_none()
        {
            self.landmark_action(Action::LandmarkPicker);
            return;
        }
        let landmark = Landmark {
            position: point,
            ..self.landmarks.placement.clone()
        };
        if let Some(id) = self.editor.add_landmark(landmark) {
            self.landmarks.arrivals.insert(id, None);
            self.changed();
            self.status.clear();
        } else {
            self.status =
                "Show this layer or choose a layer with room for another landmark.".into();
        }
    }

    pub(super) fn select_landmark(&mut self, point: Point, layer: u64, id: u64) {
        self.editor.select_layer(layer);
        self.landmarks.selected = Some(id);
        let landmark = self.editor.landmark(id).unwrap().clone();
        self.landmarks.moving = Some((point, landmark));
    }

    pub fn landmark_move(&mut self, point: Point) {
        let Some((start, original)) = &self.landmarks.moving else {
            return;
        };
        let mut p = original.clone();
        p.position.x += point.x - start.x;
        p.position.y += point.y - start.y;
        if p.validate().is_ok() && self.landmarks.preview.as_ref() != Some(&p) {
            self.landmarks.preview = Some(p);
            self.invalidate_preview();
        }
    }
    pub fn finish_landmark_move(&mut self) {
        if self.landmarks.moving.take().is_some() {
            if let Some(p) = self.landmarks.preview.take()
                && self.editor.update_landmark(p)
            {
                self.changed();
            }
            // Even an unchanged preview has to restore authoritative mesh identities.
            self.invalidate_preview();
            self.ui_dirty = true;
        }
    }
    pub fn cancel_landmark(&mut self) {
        self.landmarks.picker = None;
        self.landmarks.preview = None;
        self.landmarks.moving = None;
        self.landmarks.selected = None;
        self.invalidate_preview();
        self.ui_dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::files::Jobs;
    use bevy::prelude::*;

    fn studio() -> Studio {
        Studio {
            home: false,
            ..Studio::default()
        }
    }

    fn choose(studio: &mut Studio, jobs: &mut Jobs, kind: &str) {
        studio.act(Action::LandmarkPicker, jobs);
        let index = landmark_catalog().find(&LandmarkKind::from(kind)).unwrap();
        studio.act(Action::LandmarkKind(index), jobs);
    }

    #[test]
    fn ten_landmarks_reuse_placement_and_individually_undo() {
        let mut studio = studio();
        let mut jobs = Jobs::default();
        studio.act(Action::LandmarkTool, &mut jobs);
        choose(&mut studio, &mut jobs, "town");
        studio.act(Action::LandmarkSize(-16.), &mut jobs);
        let revision = studio.editor.revision();
        for i in 0..10 {
            let point = Vec2::new(305. + i as f32 * 80., 390.);
            studio.down(point);
            studio.up(point, false);
        }
        let landmarks = &studio.editor.document().layers[0].landmarks;
        assert_eq!(landmarks.len(), 10);
        assert!(
            landmarks
                .iter()
                .all(|mark| mark.kind == LandmarkKind::from("town") && mark.size == 128.)
        );
        assert_eq!(studio.editor.revision(), revision + 10);
        assert_eq!(studio.tool, Tool::Landmarks);
        assert!(studio.landmarks.selected.is_none());

        studio.act(Action::Undo, &mut jobs);
        assert_eq!(studio.editor.document().layers[0].landmarks.len(), 9);
        studio.act(Action::Redo, &mut jobs);
        let first = studio.editor.document().layers[0].landmarks[0].clone();
        studio.act(Action::SelectTool, &mut jobs);
        studio.annotation_press(first.position);
        assert_eq!(studio.landmarks.selected, Some(first.id));
        assert_eq!(studio.landmark_actions().len(), 5);
        choose(&mut studio, &mut jobs, "castle");
        let castle = studio.editor.landmark(first.id).unwrap();
        assert_eq!(castle.position, first.position);
        assert_eq!(castle.size, 128.);
        assert_eq!(castle.kind, LandmarkKind::from("castle"));
        studio.act(Action::LandmarkSize(16.), &mut jobs);
        assert_eq!(studio.editor.landmark(first.id).unwrap().size, 144.);
        studio.act(Action::LandmarkTool, &mut jobs);
        assert_eq!(studio.landmarks.placement.kind, LandmarkKind::from("town"));
        assert_eq!(studio.landmarks.placement.size, 128.);
    }

    #[test]
    fn landmark_drag_commits_once_and_escape_reverts_the_preview() {
        let mut studio = studio();
        let mut jobs = Jobs::default();
        let cursor = Vec2::new(500., 400.);
        studio.act(Action::LandmarkTool, &mut jobs);
        studio.down(cursor);
        studio.up(cursor, false);
        let original = studio.editor.document().layers[0].landmarks[0].clone();
        studio.act(Action::SelectTool, &mut jobs);
        studio.down(cursor - Vec2::new(0., 10.));
        assert_eq!(studio.landmarks.selected, Some(original.id));
        let revision = studio.editor.revision();
        studio.moved(cursor + Vec2::new(45., 20.), false);
        studio.moved(cursor + Vec2::new(50., 20.), false);
        assert_eq!(studio.editor.landmark(original.id), Some(&original));
        assert!(studio.landmarks.preview.is_some());
        studio.up(cursor + Vec2::new(50., 20.), false);
        assert_eq!(studio.editor.revision(), revision + 1);
        studio.act(Action::Undo, &mut jobs);
        assert_eq!(studio.editor.landmark(original.id), Some(&original));
        studio.down(cursor - Vec2::new(0., 10.));
        studio.moved(cursor + Vec2::new(70., 20.), false);
        studio.cancel_canvas_interaction();
        studio.finish();
        assert_eq!(studio.editor.landmark(original.id), Some(&original));
        assert!(studio.landmarks.preview.is_none());
    }

    #[test]
    fn disabled_packs_block_placement_but_allow_existing_edits() {
        let mut studio = studio();
        let mut jobs = Jobs::default();
        studio.act(Action::LandmarkTool, &mut jobs);
        let point = Point::new(500., 400.);
        studio.annotation_press(point);
        let id = studio.editor.document().layers[0].landmarks[0].id;
        studio.act(Action::SelectTool, &mut jobs);
        studio.annotation_press(point);
        for (index, pack) in landmark_catalog().packs.iter().enumerate() {
            if !studio.disabled_landmark_packs.contains(&pack.id) {
                studio.act(Action::LandmarkPack(index), &mut jobs);
            }
        }
        assert!(jobs.preferences);
        studio.act(Action::LandmarkSize(16.), &mut jobs);
        assert_eq!(studio.editor.landmark(id).unwrap().size, 160.);
        studio.act(Action::LandmarkTool, &mut jobs);
        studio.annotation_press(point);
        assert_eq!(studio.landmarks.picker, Some(PickerTarget::Placement));
        assert_eq!(studio.editor.document().layers[0].landmarks.len(), 1);
        studio.annotation_press(point);
        assert!(studio.landmarks.picker.is_none());
        assert_eq!(studio.editor.document().layers[0].landmarks.len(), 1);
    }
}
