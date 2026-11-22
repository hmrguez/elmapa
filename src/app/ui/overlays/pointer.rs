//! Tool cursors, material picker and hover tooltips.
use super::orbit::{Orbit, action_orbit};
use crate::app::{
    radial,
    state::{Action, Studio, Tool},
    ui::{
        components::{Builder, color, shadow},
        contains,
        feedback::Hover,
    },
};
use bevy::prelude::*;

#[derive(Component)]
pub struct Cursor;
#[derive(Component)]
pub struct LandmarkCursor {
    anchor: Vec2,
    size: Vec2,
    artwork_width: f32,
}
#[derive(Component)]
pub struct Tip;

pub(in crate::app::ui) fn build_feedback(b: &mut Builder, root: Entity, studio: &Studio) {
    // Brush ring uses its own node, independent of panel layout.
    let cursor = b.node(
        root,
        Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            border: UiRect::all(px(1.)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
    );
    b.commands
        .entity(cursor)
        .insert((Cursor, BorderColor::all(color(0x75654f, 0.65))));
    if studio.tool == Tool::Landmarks
        && let Some(index) = studio.placement_landmark()
        && let Some((image, size)) = b.assets.landmark_ghosts.get(index)
    {
        let definition = &elmapa::scene::landmark_catalog().items[index];
        b.commands.spawn((
            LandmarkCursor {
                anchor: Vec2::new(definition.anchor.x, definition.anchor.y),
                size: *size,
                artwork_width: definition.width,
            },
            ImageNode {
                image: image.clone(),
                color: Color::WHITE.with_alpha(0.46),
                ..default()
            },
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                ..default()
            },
            GlobalZIndex(25),
            ChildOf(root),
        ));
    }
    let tip = b.node(
        root,
        Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            padding: UiRect::axes(px(12.), px(8.)),
            border_radius: BorderRadius::all(px(8.)),
            ..default()
        },
    );
    b.commands.entity(tip).insert((
        Tip,
        BackgroundColor(color(b.p.surface, 1.)),
        shadow(false),
        GlobalZIndex(100),
    ));
    b.text(tip, "", 11., b.p.text, false);
}

pub(in crate::app::ui) fn update_brush_cursor(
    studio: &Studio,
    cursor: &mut Query<&mut Node, With<Cursor>>,
) {
    for mut node in cursor {
        if studio.home
            || studio.installing
            || studio.drag.is_some()
            || studio.tool != Tool::Brush
            || studio.regions.active
            || studio.brush_orbit.anchor.is_some()
            || studio.pointer.is_none()
            || studio.opening.is_some()
        {
            node.display = Display::None;
            continue;
        }
        let point = studio.pointer.expect("pointer");
        let diameter = studio.radius * studio.viewport.zoom;
        node.display = Display::Flex;
        node.left = px(point.x - diameter / 2. - 0.5);
        node.top = px(point.y - diameter / 2. - 0.5);
        node.width = px(diameter + 1.);
        node.height = px(diameter + 1.);
    }
}

pub fn landmark_cursor(studio: Res<Studio>, mut nodes: Query<(&LandmarkCursor, &mut Node)>) {
    let show = studio.tool == Tool::Landmarks
        && !studio.home
        && !studio.installing
        && studio.opening.is_none()
        && studio.replay_view.is_none()
        && studio.drag.is_none();
    for (ghost, mut node) in &mut nodes {
        if let Some(point) = studio.pointer.filter(|_| show) {
            let scale =
                studio.landmarks.placement.size * studio.viewport.zoom / ghost.artwork_width;
            let top_left = point - ghost.anchor * scale;
            node.display = Display::Flex;
            node.left = px(top_left.x);
            node.top = px(top_left.y);
            node.width = px(ghost.size.x * scale);
            node.height = px(ghost.size.y * scale);
        } else {
            node.display = Display::None;
        }
    }
}

pub(super) fn brush_orbit(b: &mut Builder, root: Entity, studio: &Studio) {
    let Some(anchor) = studio.brush_orbit.anchor else {
        return;
    };
    let orbit = action_orbit(
        b,
        root,
        &studio.brush_orbit.items(studio.brush, studio.biome_pack),
        &radial::Navigation::default(),
        &format!(
            "brush/{}/{}",
            studio.brush_orbit.generation, studio.brush_orbit.page
        ),
        |_, _| Action::CloseBrushOrbit,
        Action::CloseBrushOrbit,
    );
    b.commands
        .entity(orbit)
        .entry::<Orbit>()
        .and_modify(move |mut orbit| {
            orbit.center = anchor;
            orbit.radius = Vec2::splat(80.);
        });
}

pub fn tooltips(
    window: Single<&Window>,
    hovered: Query<(
        &Hover,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&CalculatedClip>,
    )>,
    mut tip: Query<(&mut Node, &Children, &ComputedNode), With<Tip>>,
    mut texts: Query<&mut Text>,
    time: Res<Time<Real>>,
    mut dwell: Local<(String, f32)>,
) {
    if let Ok((mut node, children, computed)) = tip.single_mut() {
        let candidate = window.cursor_position().and_then(|point| {
            hovered
                .iter()
                .filter(|(h, n, t, c)| h.progress > 0.99 && contains(point, n, t, *c))
                .last()
                .map(|(h, _, _, _)| (point, h.tip.clone()))
        });
        if let Some((point, label)) = candidate {
            if dwell.0 != label {
                dwell.0 = label.clone();
                dwell.1 = time.elapsed_secs();
            }
            if time.elapsed_secs() - dwell.1 < 0.5 {
                node.display = Display::None;
                return;
            }
            node.display = Display::Flex;
            let width = if computed.size().x > 0. {
                computed.size().x * computed.inverse_scale_factor()
            } else {
                label.chars().count() as f32 * 6. + 24.
            };
            node.left = px((point.x + 12.).min(window.width() - width - 12.).max(0.));
            node.top = px((point.y + 24.).min(window.height() - 40.));
            for child in children {
                if let Ok(mut text) = texts.get_mut(*child)
                    && text.0 != label
                {
                    text.0 = label.clone();
                }
            }
        } else {
            node.display = Display::None;
            dwell.0.clear();
        }
    }
}
