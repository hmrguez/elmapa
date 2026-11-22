//! Shared canvas gesture and material picker state for paint tools.
use super::{
    radial::{Item, Navigation},
    state::{Action, Studio},
    ui,
};
use bevy::prelude::*;
use elmapa::{biome_packs, brush_materials, document::Brush};
use std::{sync::Arc, time::Duration};

const HOLD: Duration = Duration::from_millis(1500);
const SLOP: f32 = 5.;
const CORE_PACK: usize = 0;
pub const DEFAULT_RIVER_FINISH: &str = "Default channel";

pub struct BrushPress {
    pub start: Vec2,
    brush: Brush,
    began: Duration,
}

impl BrushPress {
    pub fn new(start: Vec2, brush: Brush, began: Duration) -> Self {
        Self {
            start,
            brush,
            began,
        }
    }

    pub fn dragged(&self, point: Vec2) -> bool {
        self.start.distance_squared(point) > SLOP * SLOP
    }

    pub fn held(&self, now: Duration) -> bool {
        now.saturating_sub(self.began) >= HOLD
    }

    pub fn valid(&self, studio: &Studio) -> bool {
        self.brush == studio.brush && studio.can_open_brush_orbit()
    }
}

#[derive(Clone, Copy)]
struct Selection {
    pack: usize,
    material: Option<usize>,
}

#[derive(Default)]
pub struct BrushOrbit {
    pub anchor: Option<Vec2>,
    pub navigation: Navigation,
    pub generation: u64,
    selected: [Option<Selection>; Brush::ALL.len()],
}

impl BrushOrbit {
    pub fn supports(brush: Brush) -> bool {
        matches!(
            brush,
            Brush::Land | Brush::Water | Brush::Forest | Brush::Mountain | Brush::River
        )
    }

    fn selection(&self, brush: Brush) -> Selection {
        self.selected[brush as usize].unwrap_or_else(|| Selection {
            pack: CORE_PACK,
            material: brush_materials::for_brush(brush)
                .next()
                .map(|(index, _)| index),
        })
    }

    pub fn current_index(&self, brush: Brush) -> Option<usize> {
        self.selection(brush).material
    }

    pub fn stroke_material(&self, brush: Brush) -> Option<Arc<str>> {
        self.current_index(brush)
            .map(|index| &brush_materials::catalog()[index])
            .filter(|material| material.texture.is_some() || material.forest.is_some())
            .map(|material| Arc::from(material.id.as_str()))
    }

    pub fn enter(&mut self, brush: Brush, depth: usize, index: usize) {
        self.navigation.enter_at(&self.items(brush), depth, index);
    }

    pub fn select(&mut self, brush: Brush, material: Option<usize>) {
        if let Some(&pack) = self.navigation.path.first()
            && self
                .navigation
                .items(&self.items(brush))
                .iter()
                .any(|item| item.action == Some(Action::BrushMaterial(material)))
        {
            self.selected[brush as usize] = Some(Selection { pack, material });
        }
    }

    pub fn items(&self, brush: Brush) -> Vec<Item<Action>> {
        let selected = self.selection(brush);
        let icon = ui::brush_icon(brush);
        let mut packs = biome_packs::catalog()
            .iter()
            .enumerate()
            .filter_map(|(pack, entry)| {
                let mut materials = biome_packs::materials(Some(pack), brush).peekable();
                let color = materials.peek()?.1.color;
                let mut children = Vec::new();
                if brush == Brush::River && pack == CORE_PACK {
                    let mut default =
                        Item::action(DEFAULT_RIVER_FINISH, icon, Action::BrushMaterial(None));
                    default.active = selected.material.is_none();
                    children.push(default);
                }
                children.extend(materials.map(|(index, material)| {
                    let mut item =
                        Item::action(&material.name, icon, Action::BrushMaterial(Some(index)));
                    item.active = selected.pack == pack && selected.material == Some(index);
                    item
                }));
                let mut branch = Item::branch(&entry.name, icon, children);
                branch.swatch = Some(color);
                branch.active = selected.pack == pack;
                Some(branch)
            })
            .collect::<Vec<_>>();
        packs.push(Item::action(
            "Close materials · Esc",
            "close",
            Action::CloseBrushOrbit,
        ));
        packs
    }
}

impl Studio {
    pub fn can_open_brush_orbit(&self) -> bool {
        !self.home
            && !self.installing
            && self.opening.is_none()
            && !self.menu
            && self.drag.is_none()
            && self.tool == super::state::Tool::Brush
            && !self.regions.active
            && BrushOrbit::supports(self.brush)
    }

    pub fn open_brush_orbit(&mut self, point: Vec2) {
        self.brush_orbit.anchor = Some(point);
        self.brush_orbit.navigation.reset();
        self.brush_orbit.generation = self.brush_orbit.generation.wrapping_add(1);
        self.ui_dirty = true;
    }

    pub fn close_brush_orbit(&mut self) -> bool {
        let was_open = self.brush_orbit.anchor.take().is_some();
        self.ui_dirty |= was_open;
        was_open
    }
}
