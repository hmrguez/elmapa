//! Landmark placement, selection and one-checkpoint drag previews.
use super::{
    radial::{Item, Navigation},
    state::{Action, Studio, Tool},
};
use elmapa::{
    biome_packs,
    document::{
        Landmark, LandmarkKind, Point,
        limits::{MAX_LANDMARK_SIZE, MIN_LANDMARK_SIZE},
    },
    scene::{LandmarkCatalog, landmark_catalog},
};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct LandmarkTool {
    pub selected: Option<u64>,
    pub placement: Landmark,
    pub radial: Navigation,
    pub last_placed: Option<u64>,
    pub moving: Option<(Point, Landmark)>,
    pub preview: Option<Landmark>,
    /// Only explicit placement or changing a specimen requests motion.
    pub arrivals: HashMap<u64, Option<f32>>,
}

fn landmark_choice(
    catalog: &LandmarkCatalog,
    index: usize,
    current: Option<&LandmarkKind>,
) -> Item<Action> {
    let entry = &catalog.items[index];
    let mut item = Item::action(
        entry.name.clone(),
        entry.kind.key(),
        Action::LandmarkKind(index),
    );
    item.active = current == Some(&entry.kind);
    item
}

fn landmark_picker(
    catalog: &LandmarkCatalog,
    disabled: &HashSet<String>,
    current: Option<&LandmarkKind>,
    pack: Option<usize>,
) -> Vec<Item<Action>> {
    if let Some(pack) = pack {
        return biome_packs::landmark_items(pack, catalog, disabled)
            .map(|index| landmark_choice(catalog, index, current))
            .collect();
    }
    let packs: Vec<_> = catalog
        .packs
        .iter()
        .enumerate()
        .filter(|(_, p)| !disabled.contains(&p.id))
        .collect();
    let children = |pack: usize| {
        catalog
            .items
            .iter()
            .enumerate()
            .filter(|(_, d)| d.pack == pack)
            .map(|(index, _)| landmark_choice(catalog, index, current))
            .collect::<Vec<_>>()
    };
    if packs.len() == 1 {
        return children(packs[0].0);
    }
    packs
        .into_iter()
        .map(|(i, p)| {
            let mut item = Item::branch(p.name.clone(), p.icon_key.clone(), children(i));
            item.active = item.children.iter().any(|c| c.active);
            item
        })
        .collect()
}

impl Studio {
    pub(super) fn reset_landmark_picker(&mut self) {
        self.landmarks.radial.reset();
        self.landmarks.last_placed = None;
        self.landmarks.placement.kind = self
            .biome_pack
            .and_then(|index| biome_packs::catalog()[index].landmarks.first())
            .map(|id| LandmarkKind::from(id.as_str()))
            .unwrap_or_default();
    }

    fn landmark_choices(&self) -> Vec<usize> {
        let catalog = landmark_catalog();
        match self.biome_pack {
            Some(pack) => {
                biome_packs::landmark_items(pack, catalog, &self.disabled_landmark_packs).collect()
            }
            None => catalog.enabled_items(&self.disabled_landmark_packs),
        }
    }

    pub fn placement_landmark(&self) -> Option<usize> {
        let catalog = landmark_catalog();
        let enabled = self.landmark_choices();
        catalog
            .find(&self.landmarks.placement.kind)
            .filter(|index| enabled.contains(index))
            .or_else(|| enabled.first().copied())
    }

    pub fn current_landmark(&self) -> Option<Landmark> {
        self.landmarks.preview.clone().or_else(|| {
            self.landmarks
                .selected
                .and_then(|id| self.editor.landmark(id).cloned())
        })
    }

    pub fn landmark_actions(&self) -> Vec<Item<Action>> {
        let current = self.current_landmark();
        let mut items = landmark_picker(
            landmark_catalog(),
            &self.disabled_landmark_packs,
            current.as_ref().map(|mark| &mark.kind),
            self.biome_pack,
        );
        items.extend([
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
            Item::action(
                if current.is_some_and(|mark| mark.name.is_some()) {
                    "Edit name · type"
                } else {
                    "Name landmark · type"
                },
                "text",
                Action::LandmarkName,
            ),
        ]);
        items
    }

    pub fn landmark_action(&mut self, action: Action) {
        match action {
            Action::LandmarkEnter(depth, index) if self.current_landmark().is_some() => {
                let items = self.landmark_actions();
                self.landmarks.radial.enter_at(&items, depth, index);
            }
            Action::LandmarkBack => self.landmarks.radial.back(),
            Action::LandmarkKind(index) => {
                if !self.landmark_choices().contains(&index) {
                    return;
                }
                let catalog = landmark_catalog();
                let def = &catalog.items[index];
                if self.tool == Tool::Landmarks {
                    // Native replay scripts can set the placement icon before clicking.
                    self.landmarks.placement.kind = def.kind.clone();
                } else if self.tool == Tool::Select
                    && let Some(mut landmark) = self.current_landmark()
                {
                    let id = landmark.id;
                    landmark.kind = def.kind.clone();
                    if self.editor.update_landmark(landmark) {
                        self.landmarks.arrivals.insert(id, None);
                        if self.landmarks.last_placed == Some(id) {
                            self.landmarks.placement.kind = def.kind.clone();
                        }
                    }
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
                        self.landmarks.radial.reset();
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

    pub(super) fn place_landmark(&mut self, point: Point, repeat: bool) {
        let Some(index) = self.placement_landmark() else {
            self.status = if self.biome_pack.is_some() {
                "Enable a landmark pack in Settings or choose another biome pack."
            } else {
                "Enable a landmark pack in Settings to place landmarks."
            }
            .into();
            return;
        };
        let kind = &landmark_catalog().items[index].kind;
        let landmark = Landmark {
            position: point,
            kind: kind.clone(),
            ..self.landmarks.placement.clone()
        };
        if let Some(id) = self.editor.add_landmark(landmark) {
            self.landmarks.last_placed = Some(id);
            self.landmarks.placement.kind = kind.clone();
            self.landmarks.arrivals.insert(id, None);
            if !repeat {
                self.tool = Tool::Select;
                self.landmarks.selected = Some(id);
                self.landmarks.radial.reset();
            }
            self.changed();
            self.status.clear();
        } else {
            self.status =
                "Show this layer or choose a layer with room for another landmark.".into();
        }
    }

    pub(super) fn select_landmark(&mut self, point: Point, layer: u64, id: u64) {
        if self.text.landmark_name {
            self.cancel_text();
        }
        self.editor.select_layer(layer);
        if self.landmarks.selected != Some(id) {
            self.landmarks.radial.reset();
        }
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
        if p.validate().is_ok() {
            // The pointer takes over from the placement animation.
            if p.position != original.position {
                self.landmarks.arrivals.remove(&p.id);
                self.landmarks.preview = Some(p);
            } else {
                self.landmarks.preview = None;
            }
        }
    }
    pub fn finish_landmark_move(&mut self) {
        if self.landmarks.moving.take().is_some() {
            if let Some(p) = self.landmarks.preview.take()
                && self.editor.update_landmark(p)
            {
                self.changed();
            }
            self.ui_dirty = true;
        }
    }
    pub fn cancel_landmark(&mut self) {
        if self.text.landmark_name {
            self.cancel_text();
        }
        self.landmarks.radial.reset();
        self.landmarks.preview = None;
        self.landmarks.moving = None;
        self.landmarks.selected = None;
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
        let index = landmark_catalog().find(&LandmarkKind::from(kind)).unwrap();
        studio.act(Action::LandmarkKind(index), jobs);
    }

    #[test]
    fn shift_repeats_the_last_placed_icon_and_preserves_individual_undo() {
        let mut studio = studio();
        let mut jobs = Jobs::default();
        studio.act(Action::LandmarkTool, &mut jobs);
        studio.act(Action::LandmarkSize(-16.), &mut jobs);
        let point = Vec2::new(305., 390.);
        studio.down(point);
        studio.up(point, false);
        let first_id = studio.landmarks.selected.unwrap();
        assert_eq!(studio.tool, Tool::Select);
        choose(&mut studio, &mut jobs, "town");

        studio.act(Action::LandmarkTool, &mut jobs);
        let revision = studio.editor.revision();
        for i in 1..10 {
            let point = Vec2::new(305. + i as f32 * 80., 390.);
            studio.down_with_shift(point, true);
            studio.up(point, false);
        }
        let landmarks = &studio.editor.document().layers[0].landmarks;
        assert_eq!(landmarks.len(), 10);
        assert!(
            landmarks
                .iter()
                .all(|mark| mark.kind == LandmarkKind::from("town") && mark.size == 128.)
        );
        assert_eq!(studio.editor.revision(), revision + 9);
        assert_eq!(studio.tool, Tool::Landmarks);
        assert!(studio.landmarks.selected.is_none());

        studio.act(Action::Undo, &mut jobs);
        assert_eq!(studio.editor.document().layers[0].landmarks.len(), 9);
        studio.act(Action::Redo, &mut jobs);
        assert_eq!(studio.editor.document().layers[0].landmarks.len(), 10);

        studio.act(Action::SelectTool, &mut jobs);
        let first = studio.editor.landmark(first_id).unwrap().clone();
        studio.annotation_press(first.position, false);
        assert_eq!(studio.landmarks.selected, Some(first_id));
        choose(&mut studio, &mut jobs, "castle");
        let castle = studio.editor.landmark(first_id).unwrap();
        assert_eq!(castle.position, first.position);
        assert_eq!(castle.size, 128.);
        assert_eq!(castle.kind, LandmarkKind::from("castle"));
        studio.act(Action::LandmarkSize(16.), &mut jobs);
        assert_eq!(studio.editor.landmark(first_id).unwrap().size, 144.);
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
        let scene_epoch = studio.scene.epoch();
        studio.moved(cursor + Vec2::new(45., 20.), false);
        studio.moved(cursor + Vec2::new(50., 20.), false);
        assert_eq!(studio.scene.epoch(), scene_epoch);
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
        studio.annotation_press(point, false);
        let id = studio.editor.document().layers[0].landmarks[0].id;
        studio.act(Action::SelectTool, &mut jobs);
        studio.annotation_press(point, false);
        for (index, pack) in landmark_catalog().packs.iter().enumerate() {
            if !studio.disabled_landmark_packs.contains(&pack.id) {
                studio.act(Action::LandmarkPack(index), &mut jobs);
            }
        }
        assert!(jobs.preferences);
        studio.act(Action::LandmarkSize(16.), &mut jobs);
        assert_eq!(studio.editor.landmark(id).unwrap().size, 160.);
        studio.act(Action::LandmarkTool, &mut jobs);
        studio.annotation_press(point, false);
        assert!(studio.placement_landmark().is_none());
        assert_eq!(studio.editor.document().layers[0].landmarks.len(), 1);
        assert!(studio.status.contains("Enable a landmark pack"));

        let catalog = landmark_catalog();
        let pin = catalog.find(&LandmarkKind::default()).unwrap();
        let other_pack = (0..catalog.packs.len())
            .find(|&pack| pack != catalog.items[pin].pack)
            .unwrap();
        studio.act(Action::LandmarkPack(other_pack), &mut jobs);
        studio.annotation_press(point, false);
        let placed = studio.current_landmark().unwrap();
        assert_eq!(
            catalog.items[catalog.find(&placed.kind).unwrap()].pack,
            other_pack
        );
        assert_eq!(studio.landmarks.placement.kind, placed.kind);
    }

    #[test]
    fn biome_landmarks_share_ordered_placement_and_replacement_choices() {
        let mut studio = studio();
        let mut jobs = Jobs::default();
        let desert = biome_packs::find("desert").unwrap();
        let alpine = biome_packs::find("alpine").unwrap();
        let catalog = landmark_catalog();
        let available = |studio: &Studio, pack| {
            biome_packs::landmark_items(pack, catalog, &studio.disabled_landmark_packs)
                .collect::<Vec<_>>()
        };
        let picker = |studio: &Studio| {
            studio
                .landmark_actions()
                .iter()
                .filter_map(|item| match item.action {
                    Some(Action::LandmarkKind(index)) => Some(index),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };

        studio.act(Action::BiomePack(Some(desert)), &mut jobs);
        studio.act(Action::LandmarkTool, &mut jobs);
        let allowed = available(&studio, desert);
        assert_eq!(studio.placement_landmark(), allowed.first().copied());
        let point = Point::new(500., 400.);
        studio.annotation_press(point, false);
        let placed = studio.current_landmark().unwrap();
        assert_eq!(placed.kind, catalog.items[allowed[0]].kind);
        assert_eq!(picker(&studio), allowed);
        choose(&mut studio, &mut jobs, "town");
        assert_eq!(studio.current_landmark().unwrap(), placed);
        studio.act(Action::LandmarkKind(allowed[1]), &mut jobs);
        assert_eq!(studio.placement_landmark(), Some(allowed[1]));

        let updated = studio.current_landmark().unwrap();
        studio.landmarks.radial.path.push(0);
        studio.act(Action::BiomePack(Some(alpine)), &mut jobs);
        assert!(studio.landmarks.radial.path.is_empty());
        assert_eq!(studio.current_landmark().unwrap(), updated);
        assert_eq!(picker(&studio), available(&studio, alpine));

        let nature = catalog
            .packs
            .iter()
            .position(|pack| pack.id == "elmapa.nature")
            .unwrap();
        studio.act(Action::LandmarkPack(nature), &mut jobs);
        let allowed = available(&studio, alpine);
        assert!(!allowed.is_empty());
        assert_eq!(studio.placement_landmark(), allowed.first().copied());
        assert_eq!(picker(&studio), allowed);
        studio.act(Action::LandmarkSize(16.), &mut jobs);
        assert_eq!(studio.current_landmark().unwrap().size, updated.size + 16.);
    }
}
