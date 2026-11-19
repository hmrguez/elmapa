//! Land selection, shared-border gestures and island assignment.
mod gestures;
mod split;
mod style;
#[cfg(test)]
mod tests;
pub use style::{StyleAction, StyleField};

use super::state::{Studio, Tool};
use elmapa::{
    document::{Point, RegionAddress, RegionLayer, RegionParent, RegionPolygon},
    scene::territories::{self, RegionView, Shoreline},
};
use split::{ChildSplit, SplitDraft};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RegionAction {
    Tool,
    Style(StyleAction),
    NewLayer,
    Make,
    Draw,
    Split,
    ChildRegions,
    ParentLayer,
    SplitCount(isize),
    ApplySplit,
    AddIslands,
    Done,
}

#[derive(Clone, Copy, Default, PartialEq)]
pub enum Mode {
    #[default]
    Select,
    Draw,
    Split,
    Assign(usize),
}

pub struct Selection {
    pub owner: Option<usize>,
    address: Option<RegionAddress>,
    polygon: RegionPolygon,
    pub outline: Vec<Vec<Point>>,
    shore: Arc<Shoreline>,
}

impl Selection {
    fn new(
        polygon: RegionPolygon,
        owned: Option<(RegionAddress, usize)>,
        shore: Arc<Shoreline>,
    ) -> Self {
        let outline = territories::visible_face(&polygon, &shore)
            .into_iter()
            .flatten()
            .map(|ring| {
                ring.into_iter()
                    .map(|[x, y]| Point::new(x as f32, y as f32))
                    .collect()
            })
            .collect();
        Self {
            owner: owned.map(|(_, owner)| owner),
            address: owned.map(|(address, _)| address),
            polygon,
            outline,
            shore,
        }
    }
}

struct BorderDrag {
    address: RegionAddress,
    start: Point,
    anchor: Point,
    border: Vec<Point>,
    weights: Vec<f32>,
    moved: bool,
}

enum Gesture {
    Cut(Vec<Point>),
    Move(BorderDrag, Vec<Point>),
    Assign { point: Point, owner: usize },
}

struct RecentRegion {
    layer: u64,
    revision: u64,
    terrain: Arc<Shoreline>,
    scope: Arc<Shoreline>,
    view: RegionView,
}

#[derive(Default)]
pub struct RegionTool {
    pub active: bool,
    pub selection: Option<Selection>,
    pub mode: Mode,
    pub message: Option<String>,
    pub style: style::StyleOrbit,
    gesture: Option<Gesture>,
    split: Option<SplitDraft>,
    artwork_layer: Option<u64>,
    pub overlay_revision: u64,
    recent: Option<RecentRegion>,
}

impl RegionTool {
    fn refresh_overlay(&mut self) {
        self.overlay_revision = self.overlay_revision.wrapping_add(1);
    }

    pub fn clear(&mut self) {
        self.mode = Mode::Select;
        self.selection = None;
        self.message = None;
        self.gesture = None;
        self.split = None;
        self.recent = None;
        self.style = style::StyleOrbit::default();
        self.refresh_overlay();
    }

    pub fn cancel_gesture(&mut self) {
        self.gesture = None;
        if let Some(split) = &mut self.split {
            split.cancel_move();
        }
        self.refresh_overlay();
    }

    pub fn is_dragging(&self) -> bool {
        matches!(self.gesture, Some(Gesture::Move(..)))
    }

    pub fn split_count(&self) -> Option<usize> {
        self.split.as_ref().map(|draft| draft.count)
    }

    pub fn split_preview(&self) -> Option<(&RegionView, &[elmapa::scene::Shape])> {
        self.split
            .as_ref()
            .map(|draft| (draft.view(), draft.washes()))
    }

    pub fn draft_scope(&self) -> Option<&Shoreline> {
        let recent = self.recent.as_ref()?;
        (!Arc::ptr_eq(&recent.terrain, &recent.scope)).then_some(&recent.scope)
    }

    pub fn draft(&self) -> Option<&[Point]> {
        if self.mode == Mode::Split {
            return None;
        }
        match self.gesture.as_ref()? {
            Gesture::Cut(path) | Gesture::Move(_, path) => Some(path),
            Gesture::Assign { .. } => None,
        }
    }

    pub fn handle(&self, cursor: Option<Point>) -> Option<Point> {
        if self.mode == Mode::Split {
            return None;
        }
        let Gesture::Move(drag, _) = self.gesture.as_ref()? else {
            return None;
        };
        Some(gestures::translate(
            drag.anchor,
            cursor.unwrap_or(drag.start),
            drag.start,
            1.,
        ))
    }
}

impl Studio {
    pub fn regions_layer(&self) -> Option<&elmapa::document::Layer> {
        self.editor
            .document()
            .layer(self.editor.active_layer())
            .filter(|layer| layer.territories.is_some())
    }

    pub fn activate_regions(&mut self) {
        if self.regions_layer().is_none() {
            self.regions.artwork_layer = Some(self.editor.active_layer());
        }
        self.regions.active = true;
        self.regions.clear();
        self.advanced = false;
        self.cancel_text();
        self.cancel_landmark();
        self.clear_path_selection();
        self.tool = Tool::Brush;
    }

    pub fn leave_regions(&mut self) {
        self.regions.active = false;
        self.regions.clear();
        if self.regions_layer().is_some() {
            if let Some(id) = self.regions.artwork_layer.take()
                && self
                    .editor
                    .document()
                    .layer(id)
                    .is_some_and(|layer| layer.visible && layer.territories.is_none())
            {
                self.editor.select_layer(id);
            } else {
                self.editor.select_artwork_layer();
            }
        }
    }

    pub fn cancel_region_step(&mut self) -> bool {
        if self.regions.active && !self.regions.style.navigation.path.is_empty() {
            self.region_action(RegionAction::Style(StyleAction::Back));
            self.ui_dirty = true;
            return true;
        }
        if !self.regions.active
            || (self.regions.mode == Mode::Select && self.regions.gesture.is_none())
        {
            return false;
        }
        self.region_action(RegionAction::Done);
        self.ui_dirty = true;
        true
    }

    pub fn region_action(&mut self, action: RegionAction) {
        if let RegionAction::Style(action) = action {
            self.region_style_action(action);
            return;
        }
        self.regions.message = None;
        self.regions.style.close_controls();
        match action {
            RegionAction::Style(_) => unreachable!(),
            RegionAction::Tool => {
                self.activate_regions();
                if self.regions_layer().is_none()
                    && let Some(id) = self
                        .editor
                        .document()
                        .layers
                        .iter()
                        .rev()
                        .find(|layer| {
                            layer.visible && layer.opacity > 0. && layer.territories.is_some()
                        })
                        .map(|layer| layer.id)
                {
                    self.editor.select_layer(id);
                }
            }
            RegionAction::NewLayer => {
                self.activate_regions();
                let shore = self.scene.snapshot().shoreline().clone();
                if let Err(error) = self
                    .editor
                    .set_regions(None, RegionLayer::default(), &shore)
                {
                    self.regions.message = Some(error);
                }
            }
            RegionAction::Make => {
                if let Some(selection) = self.regions.selection.take()
                    && selection.owner.is_none()
                {
                    self.claim_region(selection, None);
                }
            }
            RegionAction::Draw if self.regions.selection.is_some() => {
                self.regions.mode = Mode::Draw
            }
            RegionAction::Split if self.regions.mode == Mode::Select => self.preview_split(2, None),
            RegionAction::ChildRegions => self.open_child_regions(),
            RegionAction::ParentLayer => {
                if let Some(parent) = self.active_region_parent() {
                    self.editor.select_layer(parent.layer);
                    self.regions.clear();
                }
            }
            RegionAction::SplitCount(delta) => {
                if let Some(count) = self
                    .regions
                    .split_count()
                    .and_then(|count| count.checked_add_signed(delta))
                    && (2..=territories::MAX_SPLIT_COUNT).contains(&count)
                {
                    let child = self.regions.split.as_ref().and_then(|draft| draft.child);
                    self.preview_split(count, child);
                }
            }
            RegionAction::ApplySplit => self.apply_split(),
            RegionAction::Split => {}
            RegionAction::AddIslands => {
                if let Some(owner) = self
                    .regions
                    .selection
                    .as_ref()
                    .and_then(|selection| selection.owner)
                {
                    self.regions.mode = Mode::Assign(owner);
                }
            }
            RegionAction::Done | RegionAction::Draw => {
                self.regions.mode = Mode::Select;
                self.regions.split = None;
                self.regions.cancel_gesture();
            }
        }
    }

    pub fn active_region_parent(&self) -> Option<RegionParent> {
        self.regions_layer()?.territories.as_ref()?.parent
    }

    pub fn split_parent_name(&self) -> Option<&str> {
        let parent = self.regions.split.as_ref()?.child?.parent;
        self.editor.document().region_name(parent)
    }

    fn open_child_regions(&mut self) {
        let Some((layer, _, owner)) = self.region_style_target() else {
            return;
        };
        let parent = RegionParent { layer, owner };
        if let Some(child) = self.editor.document().child_regions(parent) {
            self.editor.select_layer(child.id);
            self.regions.clear();
        } else if let Some(anchor) = self.regions.style.anchor {
            self.preview_split(2, Some(ChildSplit { parent, anchor }));
        }
    }

    fn preview_split(&mut self, count: usize, child: Option<ChildSplit>) {
        let Some(selection) = &self.regions.selection else {
            return;
        };
        let source = self
            .regions_layer()
            .and_then(|layer| layer.territories.as_deref());
        match SplitDraft::new(selection, source, count, child) {
            Ok(draft) => {
                self.regions.split = Some(draft);
                self.regions.mode = Mode::Split;
                self.regions.cancel_gesture();
            }
            Err(error) => self.regions.message = Some(error),
        }
    }

    fn apply_split(&mut self) {
        let (Some(draft), Some(selection)) = (&self.regions.split, &self.regions.selection) else {
            return;
        };
        let creating_child = draft.child.is_some();
        let (id, data) = if creating_child {
            (None, draft.data.clone())
        } else {
            let (id, mut data) = self.region_data();
            if draft.apply_to(&mut data, selection).is_none() {
                self.regions.message = Some("Select the land again to start a new split.".into());
                return;
            }
            (id, data)
        };
        match self.editor.set_regions(id, data, &selection.shore) {
            Ok(()) => {
                self.regions.clear();
                self.regions.message = Some(if creating_child {
                    "Child layer created. Click a region to edit it.".into()
                } else {
                    "Split applied. Select a region to add islands.".into()
                });
            }
            Err(error) => self.regions.message = Some(error),
        }
    }

    fn claim_region(&mut self, selection: Selection, owner: Option<usize>) {
        let (id, mut data) = self.region_data();
        let index = owner.unwrap_or_else(|| data.new_region());
        let address = data.claim(selection.polygon.clone(), index);
        let shore = selection.shore.clone();
        match self.editor.set_regions(id, data, &shore) {
            Ok(()) if owner.is_some() => {
                self.regions.message =
                    Some("Island added. Click another island or choose Done.".into());
            }
            Ok(()) => {
                self.regions.selection = Some(Selection::new(
                    selection.polygon,
                    Some((address, index)),
                    shore,
                ));
            }
            Err(error) => {
                self.regions.message = Some(error);
                if owner.is_none() {
                    self.regions.selection = Some(selection);
                }
            }
        }
        self.regions.refresh_overlay();
    }

    fn region_data(&self) -> (Option<u64>, RegionLayer) {
        self.regions_layer().map_or_else(
            || (None, RegionLayer::default()),
            |layer| {
                (
                    Some(layer.id),
                    layer.territories.as_deref().expect("regions layer").clone(),
                )
            },
        )
    }

    fn region_view(&mut self) -> Option<&RegionView> {
        let layer = self
            .regions_layer()
            .filter(|layer| layer.visible && layer.opacity > 0.)?;
        let id = layer.id;
        let data = layer.territories.clone()?;
        let revision = self.editor.revision();
        let scene = self.scene.snapshot();
        if data.parent.is_none()
            && let Some(view) = scene.current_region_view(id, &data)
        {
            return Some(view);
        }
        let terrain = scene.shoreline();
        if self.regions.recent.as_ref().is_none_or(|prior| {
            prior.layer != id || prior.revision != revision || !Arc::ptr_eq(&prior.terrain, terrain)
        }) {
            let ancestors = data.ancestors(|layer| self.editor.document().region_data(layer))?;
            let scope = Shoreline::scoped(terrain, &ancestors)?;
            let view = RegionView::resolve(&data, &scope)?;
            self.regions.recent = Some(RecentRegion {
                layer: id,
                revision,
                terrain: terrain.clone(),
                scope,
                view,
            });
        }
        self.regions.recent.as_ref().map(|recent| &recent.view)
    }

    fn region_shore(&mut self) -> Option<Arc<Shoreline>> {
        if self.active_region_parent().is_none() {
            return Some(self.scene.snapshot().shoreline().clone());
        }
        self.region_view()?;
        Some(self.regions.recent.as_ref()?.scope.clone())
    }

    fn select_land(&mut self, point: Point) -> Option<Selection> {
        let shore = self.region_shore()?;
        let island = shore.island_at(point)?;
        if let Some(view) = self.region_view()
            && let Some(face) = view
                .faces
                .iter()
                .find(|face| territories::contains(&face.polygon, point))
        {
            return Some(Selection::new(
                face.polygon.clone(),
                Some((face.address, face.owner)),
                shore,
            ));
        }
        let polygon = if let Some(data) = self
            .regions_layer()
            .and_then(|layer| layer.territories.as_ref())
        {
            territories::unclaimed_at(island, data, point)?
        } else {
            island.clone()
        };
        Some(Selection::new(polygon, None, shore))
    }
}
