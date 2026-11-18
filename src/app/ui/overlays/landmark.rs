//! Selection frame and action orbit for a placed landmark.
use super::orbit::{Orbit, action_orbit};
use crate::app::{
    landmarks::PickerTarget,
    state::{Action, Studio, Tool},
    ui::{
        ScrollArea,
        widgets::{Builder, color, column, row},
    },
};
use bevy::prelude::*;

#[derive(Component)]
pub struct LandmarkOrbit;
#[derive(Component)]
pub struct LandmarkSelection;
pub(super) fn build(b: &mut Builder, root: Entity, s: &Studio) {
    if s.tool != Tool::Select || s.current_landmark().is_none() {
        return;
    }
    let frame = b.node(
        root,
        Node {
            position_type: PositionType::Absolute,
            border: UiRect::all(px(1.)),
            border_radius: BorderRadius::all(px(8.)),
            ..default()
        },
    );
    b.commands
        .entity(frame)
        .insert((LandmarkSelection, BorderColor::all(color(b.p.accent, 0.45))));
    let orbit = action_orbit(
        b,
        root,
        &s.landmark_actions(),
        &Default::default(),
        &format!("landmark/{:?}", s.landmarks.selected),
        |_, _| Action::LandmarkPicker,
        Action::CloseLandmarkPicker,
    );
    b.commands.entity(orbit).insert(LandmarkOrbit);
}
#[derive(Component)]
pub struct LandmarkPickerCard;

pub(super) fn picker(b: &mut Builder, root: Entity, s: &Studio, scroll: f32) {
    let Some(target) = s.landmarks.picker else {
        return;
    };
    let (title, current) = match target {
        PickerTarget::Placement => ("Choose a symbol", Some(&s.landmarks.placement)),
        PickerTarget::Replacement(id) => ("Change icon", s.editor.landmark(id)),
    };
    let guard = b.node(
        root,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100.),
            height: percent(100.),
            ..default()
        },
    );
    b.hit(guard, Some(Action::CloseLandmarkPicker));
    let width = (s.bounds.size.x - 24.).clamp(210., 440.);
    let panel = b.panel(
        root,
        Node {
            position_type: PositionType::Absolute,
            left: px((s.bounds.size.x - width) / 2.),
            bottom: px(82.),
            width: px(width),
            ..column(7.)
        },
        17.,
        10.,
        0.98,
        true,
    );
    b.commands.entity(panel).insert(GlobalZIndex(30));
    if matches!(target, PickerTarget::Replacement(_)) {
        b.commands.entity(panel).insert(LandmarkPickerCard);
    }
    let p = b.p;
    let header = b.node(
        panel,
        Node {
            justify_content: JustifyContent::SpaceBetween,
            ..row(6.)
        },
    );
    b.text(header, title, 20., p.text, true);
    b.button(header, "close", "", Action::CloseLandmarkPicker, false);

    let catalog = elmapa::scene::landmark_catalog();
    let choices = catalog.enabled_items(&s.disabled_landmark_packs);
    if choices.is_empty() {
        b.text(
            panel,
            "Enable a landmark pack in Settings.",
            12.,
            p.muted,
            false,
        );
        return;
    }
    let gallery = b.node(
        panel,
        Node {
            max_height: px((s.bounds.size.y - 180.).clamp(64., 320.)),
            overflow: Overflow::scroll_y(),
            ..column(10.)
        },
    );
    b.commands
        .entity(gallery)
        .insert((ScrollArea::Landmarks, ScrollPosition(Vec2::new(0., scroll))));
    for (pack_index, pack) in catalog.packs.iter().enumerate() {
        if s.disabled_landmark_packs.contains(&pack.id) {
            continue;
        }
        let group = b.node(
            gallery,
            Node {
                flex_shrink: 0.,
                ..column(4.)
            },
        );
        let heading = b.node(
            group,
            Node {
                overflow: Overflow::clip(),
                ..row(6.)
            },
        );
        b.icon(heading, &pack.icon_key, p.muted, 13.);
        b.text(heading, &pack.name, 12., p.text, false);

        let grid = b.node(
            group,
            Node {
                flex_wrap: FlexWrap::Wrap,
                row_gap: px(4.),
                ..row(4.)
            },
        );
        for &index in &choices {
            let item = &catalog.items[index];
            if item.pack != pack_index {
                continue;
            }
            let active = current.is_some_and(|mark| mark.kind == item.kind);
            let tile = b.button(grid, "", &item.name, Action::LandmarkKind(index), active);
            let thumbnail = b.icon(tile, item.kind.key(), p.text, 56.);
            b.commands
                .entity(tile)
                .insert_children(0, &[thumbnail])
                .entry::<Node>()
                .and_modify(|mut node| {
                    node.flex_direction = FlexDirection::Column;
                    node.row_gap = px(0.);
                    node.padding = UiRect::axes(px(3.), px(2.));
                    node.width = px(98.);
                    node.height = px(67.);
                    node.overflow = Overflow::clip();
                });
        }
    }
}

type PickerCards<'w, 's> = Query<
    'w,
    's,
    (&'static mut Node, &'static ComputedNode),
    (With<LandmarkPickerCard>, Without<LandmarkSelection>),
>;

pub fn landmark_selection_follow(
    studio: Res<Studio>,
    mut frames: Query<&mut Node, With<LandmarkSelection>>,
    mut orbits: Query<&mut Orbit, With<LandmarkOrbit>>,
    mut cards: PickerCards,
) {
    let Some(p) = studio.current_landmark() else {
        return;
    };
    let bounds = p.bounds();
    let view = studio.viewport.transform(studio.bounds);
    let a = view.origin + Vec2::new(bounds.min.x, bounds.min.y) * view.scale;
    let size = Vec2::new(bounds.width(), bounds.height()) * view.scale;
    for mut frame in &mut frames {
        frame.left = px(a.x - 6.);
        frame.top = px(a.y - 6.);
        frame.width = px(size.x + 12.);
        frame.height = px(size.y + 12.);
    }
    for (mut card, computed) in &mut cards {
        let measured = computed.size() * computed.inverse_scale_factor();
        if measured.x > 0. && measured.y > 0. {
            let room = studio.bounds.size;
            let x = if a.x + size.x + 18. + measured.x <= room.x - 12. {
                a.x + size.x + 18.
            } else {
                a.x - measured.x - 18.
            };
            card.left = px(x.clamp(12., (room.x - measured.x - 12.).max(12.)));
            card.top =
                px((a.y + (size.y - measured.y) / 2.)
                    .clamp(12., (room.y - measured.y - 12.).max(12.)));
            card.bottom = Val::Auto;
        }
    }
    for mut orbit in &mut orbits {
        orbit.center = a + size / 2.;
        orbit.radius = (size / 2. + Vec2::new(42., 46.)).max(Vec2::new(84., 76.));
        orbit.hidden = studio.landmarks.moving.is_some();
    }
}
