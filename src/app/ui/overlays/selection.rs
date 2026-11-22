//! Screen-sized selection ink, the marquee and the group grip.
use crate::app::{
    state::{Studio, Tool},
    ui::components::{Builder, color, shadow},
};
use bevy::prelude::*;
use elmapa::document::{ContentBounds, SelectionId};

#[derive(Component)]
pub enum SelectionOverlay {
    Marquee,
    Frame,
    Grip,
    Pin(usize),
}

const INK: u32 = 0x354b41;
const PAPER: u32 = 0xfff6df;
const SAGE: u32 = 0xadc9a4;

pub(super) fn build(b: &mut Builder, root: Entity, studio: &Studio) {
    if studio.tool != Tool::Select || studio.regions.active {
        return;
    }
    let pos = Node {
        position_type: PositionType::Absolute,
        display: Display::None,
        ..default()
    };
    let stroke = |radius| Node {
        border_radius: BorderRadius::all(px(radius)),
        border: UiRect::all(px(1.5)),
        ..pos.clone()
    };
    let marquee = b.node(root, stroke(5.));
    b.commands.entity(marquee).insert((
        SelectionOverlay::Marquee,
        BackgroundColor(color(SAGE, 0.22)),
        BorderColor::all(color(INK, 0.85)),
    ));
    if studio.selection.items.is_empty() {
        return;
    }
    if studio.path_selection.is_none() {
        let frame = b.node(root, stroke(8.));
        b.commands.entity(frame).insert((
            SelectionOverlay::Frame,
            BorderColor::all(color(INK, 0.86)),
            // A pale outer keyline keeps the frame readable over shadow and water.
            Outline {
                width: px(1.),
                offset: px(0.),
                color: color(PAPER, 0.86),
            },
        ));
    }
    let grip = b.node(
        root,
        Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..stroke(20.)
        },
    );
    b.commands.entity(grip).insert((
        SelectionOverlay::Grip,
        BackgroundColor(color(PAPER, 0.98)),
        BorderColor::all(color(INK, 0.9)),
        shadow(false),
    ));
    b.icon(grip, "hand", INK, 13.);
    if studio.selection.items.len() > 1 {
        for (index, _) in studio
            .selection
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| !matches!(item.id, SelectionId::Course(_)))
            .take(96)
        {
            let pin = b.node(root, stroke(10.));
            b.commands.entity(pin).insert((
                SelectionOverlay::Pin(index),
                BackgroundColor(color(PAPER, 0.97)),
                BorderColor::all(color(INK, 0.9)),
            ));
        }
    }
}

pub fn selection_follow(studio: Res<Studio>, mut overlays: Query<(&SelectionOverlay, &mut Node)>) {
    let view = studio.viewport.transform(studio.bounds);
    let delta = studio.selection.delta();
    let origin = view.origin + Vec2::new(delta.x, delta.y) * view.scale;
    let project = |bounds: ContentBounds, origin: Vec2| {
        Rect::from_corners(
            origin + Vec2::new(bounds.min.x, bounds.min.y) * view.scale,
            origin + Vec2::new(bounds.max.x, bounds.max.y) * view.scale,
        )
    };
    let frame = studio
        .selection
        .frame()
        .map(|bounds| project(bounds, origin).inflate(5.));
    let viewport = Rect::from_corners(Vec2::ZERO, studio.bounds.size);
    for (kind, mut node) in &mut overlays {
        let rect = match kind {
            SelectionOverlay::Marquee => studio
                .selection
                .marquee(view.scale)
                .map(|bounds| project(bounds, view.origin)),
            SelectionOverlay::Frame => frame,
            SelectionOverlay::Grip => studio
                .group_grip()
                .map(|p| Rect::from_center_size(p, Vec2::new(28., 24.))),
            SelectionOverlay::Pin(index) => studio.selection.items.get(*index).map(|item| {
                let point = origin + Vec2::new(item.anchor.x, item.anchor.y) * view.scale;
                Rect::from_center_size(point, Vec2::splat(6.))
            }),
        }
        .filter(|rect| !rect.intersect(viewport).is_empty());
        if let Some(rect) = rect {
            node.display = Display::Flex;
            node.left = px(rect.min.x);
            node.top = px(rect.min.y);
            node.width = px(rect.width());
            node.height = px(rect.height());
        } else {
            node.display = Display::None;
        }
    }
}
