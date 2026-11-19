//! Glass handles and a contextual orbit for an editable river or trail.
use super::orbit::{Orbit, action_orbit};
use crate::app::{
    radial::{Item, Navigation},
    state::{Action, Studio},
    ui::widgets::{Builder, color, shadow},
};
use bevy::prelude::*;

#[derive(Component)]
pub struct PathHandle(usize);

#[derive(Component)]
pub struct PathOrbit;

pub(super) fn build(b: &mut Builder, root: Entity, studio: &Studio) {
    let Some(path) = &studio.path_selection else {
        return;
    };
    let count = path.nodes().len();
    for index in 0..count {
        let handle = b.node(
            root,
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                border_radius: BorderRadius::MAX,
                border: UiRect::all(px(1.5)),
                ..default()
            },
        );
        b.commands.entity(handle).insert((
            PathHandle(index),
            BackgroundColor::default(),
            BorderColor::all(color(0x385749, 0.95)),
            shadow(false),
        ));
    }
    let mut actions = Vec::new();
    if count > 1 {
        if path.density > -2 {
            actions.push(Item::action(
                "Fewer nodes",
                "minus",
                Action::PathNodeDensity(-1),
            ));
        }
        if path.density < 2 {
            actions.push(Item::action(
                "More nodes",
                "plus",
                Action::PathNodeDensity(1),
            ));
        }
    }
    actions.push(Item::action(
        "Done editing nodes · Esc",
        "check",
        Action::DeselectPath,
    ));
    let orbit = action_orbit(
        b,
        root,
        &actions,
        &Navigation::default(),
        &format!("path/{}/{}", path.id.layer, path.id.index),
        |_, _| Action::DeselectPath,
        Action::DeselectPath,
    );
    b.commands.entity(orbit).insert(PathOrbit);
}

pub fn path_selection_follow(
    studio: Res<Studio>,
    mut handles: Query<(&PathHandle, &mut Node, &mut BackgroundColor)>,
    mut orbits: Query<&mut Orbit, With<PathOrbit>>,
) {
    let Some(path) = &studio.path_selection else {
        return;
    };
    let view = studio.viewport.transform(studio.bounds);
    let active = path.active_node();
    let hovered = studio
        .pointer
        .and_then(|p| path.node_at(view.map_point(p), view.scale));
    let count = path.nodes().len();
    for (PathHandle(index), mut node, mut fill) in &mut handles {
        let Some(point) = path.node(*index) else {
            node.display = Display::None;
            continue;
        };
        let center = view.origin + Vec2::new(point.x, point.y) * view.scale;
        let radius = if active == Some(*index) || hovered == Some(*index) {
            7.
        } else if *index == 0 || *index + 1 == count {
            5.5
        } else {
            4.5
        };
        node.display = Display::Flex;
        node.left = px(center.x - radius);
        node.top = px(center.y - radius);
        node.width = px(radius * 2.);
        node.height = px(radius * 2.);
        fill.0 = if active == Some(*index) {
            color(0xa6d9ba, 1.)
        } else if hovered == Some(*index) {
            color(0xffffff, 1.)
        } else {
            color(0xf9faf4, 0.94)
        };
    }
    let anchor = Vec2::new(path.anchor.x, path.anchor.y);
    let closest = path
        .nodes()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let distance =
                |p: &elmapa::document::Point| Vec2::new(p.x, p.y).distance_squared(anchor);
            distance(a).total_cmp(&distance(b))
        })
        .map_or(0, |(index, _)| index);
    let a = path.node(closest.saturating_sub(1)).unwrap_or(path.anchor);
    let b = path
        .node((closest + 1).min(count - 1))
        .unwrap_or(path.anchor);
    let mut normal = Vec2::new(b.y - a.y, a.x - b.x)
        .try_normalize()
        .unwrap_or(Vec2::NEG_Y);
    let anchor = view.origin + anchor * view.scale;
    if normal.y > 0. || normal.y == 0. && normal.x * (anchor.x - studio.bounds.center().x) > 0. {
        normal = -normal;
    }
    let viewport = Rect::from_corners(Vec2::ZERO, studio.bounds.size);
    for mut orbit in &mut orbits {
        // Offset the arc of bubbles to one side of the local course so the
        // contextual actions leave room to grab the nodes underneath.
        orbit.center = anchor + normal * 74. + Vec2::new(-30., 35.);
        orbit.radius = Vec2::splat(58.);
        orbit.hidden = active.is_some() || !viewport.contains(anchor);
    }
}
