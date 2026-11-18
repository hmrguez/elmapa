//! Shared canvas gesture and material picker state for paint tools.
use super::{
    radial::Item,
    state::{Action, Studio},
    ui,
};
use bevy::prelude::*;
use elmapa::{brush_materials, document::Brush};
use std::{sync::Arc, time::Duration};

const HOLD: Duration = Duration::from_millis(1500);
const SLOP: f32 = 5.;
const PAGE_SIZE: usize = 6;

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

#[derive(Default)]
pub struct BrushOrbit {
    pub anchor: Option<Vec2>,
    pub page: usize,
    pub generation: u64,
    selected: [Option<usize>; Brush::ALL.len()],
}

impl BrushOrbit {
    pub(super) fn current_index(&self, brush: Brush) -> Option<usize> {
        self.selected[brush as usize]
            .or_else(|| brush_materials::for_brush(brush).next().map(|(i, _)| i))
    }

    pub fn current(&self, brush: Brush) -> Option<&'static brush_materials::Material> {
        self.current_index(brush)
            .map(|index| &brush_materials::catalog()[index])
    }

    pub fn stroke_material(&self, brush: Brush) -> Option<Arc<str>> {
        self.current(brush)
            .filter(|material| material.texture.is_some() || material.forest.is_some())
            .map(|material| Arc::from(material.id.as_str()))
    }

    pub fn select(&mut self, brush: Brush, index: usize) {
        if brush_materials::catalog()
            .get(index)
            .is_some_and(|material| material.brush == brush)
        {
            self.selected[brush as usize] = Some(index);
        }
    }

    pub fn items(&self, brush: Brush) -> Vec<Item<Action>> {
        let total = brush_materials::for_brush(brush).count();
        let mut actions: Vec<_> = brush_materials::for_brush(brush)
            .skip(self.page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .map(|(index, material)| {
                let mut item = Item::action(
                    &material.name,
                    ui::brush_icon(brush),
                    Action::BrushMaterial(index),
                );
                item.active = self.current_index(brush) == Some(index);
                item
            })
            .collect();
        if total > PAGE_SIZE {
            actions.push(Item::action(
                "More materials",
                "chevron-right",
                Action::BrushOrbitPage((self.page + 1) % total.div_ceil(PAGE_SIZE)),
            ));
        }
        actions.push(Item::action(
            "Close materials · Esc",
            "close",
            Action::CloseBrushOrbit,
        ));
        actions
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
            && brush_materials::for_brush(self.brush).nth(1).is_some()
    }

    pub fn open_brush_orbit(&mut self, point: Vec2) {
        self.brush_orbit.anchor = Some(point);
        self.brush_orbit.page = 0;
        self.brush_orbit.generation = self.brush_orbit.generation.wrapping_add(1);
        self.ui_dirty = true;
    }

    pub fn close_brush_orbit(&mut self) -> bool {
        let was_open = self.brush_orbit.anchor.take().is_some();
        self.ui_dirty |= was_open;
        was_open
    }
}
