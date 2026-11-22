//! Glass handles and a contextual orbit for an editable river or trail.
use crate::app::{
    state::Studio,
    ui::components::{Builder, color, shadow},
};
use bevy::prelude::*;

#[derive(Component)]
pub struct PathHandle(usize);

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
            BorderColor::all(color(0x354b41, 0.95)),
            shadow(false),
        ));
    }
}

pub fn path_selection_follow(
    studio: Res<Studio>,
    mut handles: Query<(&PathHandle, &mut Node, &mut BackgroundColor)>,
) {
    let Some(path) = &studio.path_selection else {
        return;
    };
    let view = studio.viewport.transform(studio.bounds);
    let delta = studio.selection.delta();
    let moving = delta != elmapa::document::Point::new(0., 0.);
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
        let center = view.origin + Vec2::new(point.x + delta.x, point.y + delta.y) * view.scale;
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
        fill.0 = if moving || active == Some(*index) {
            color(0xc6e4bd, 1.)
        } else if hovered == Some(*index) {
            color(0xffffff, 1.)
        } else {
            color(0xfff6df, 0.94)
        };
    }
}
