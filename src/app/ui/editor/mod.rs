//! Floating editor chrome in draw and hit order.
mod layers;
mod menu;
mod regions;
mod toolbar;

use super::{
    feedback::{Fade, Hover},
    widgets::{Builder, absolute, color, row},
};
use crate::app::{
    branding,
    state::{Action, Studio},
};
use bevy::prelude::*;

pub use menu::{ChromeMotion, advance_motion, menu_animation};
pub use toolbar::brush_icon;

#[derive(Component)]
pub struct ZoomLabel;
#[derive(Component)]
pub struct MenuArrow;

pub(super) fn build(b: &mut Builder, root: Entity, s: &Studio, menu_progress: f32, scroll: f32) {
    header(b, root, s);
    layers::build(b, root, s, scroll);
    zoom(b, root, s);
    toolbar::build(b, root, s);
    menu::build(b, root, s, menu_progress);
    replay_controls(b, root, s);
    status(b, root, s);
}

fn header(b: &mut Builder, root: Entity, s: &Studio) {
    let p = b.p;
    let back = b.panel(root, absolute(px(20.), px(20.)), 14., 0., 0.93, false);
    b.button(back, branding::SYMBOL, "Atlases", Action::Home, false);
    let header = b.panel(
        root,
        Node {
            position_type: PositionType::Absolute,
            top: px(20.),
            right: px(20.),
            width: px(330.),
            ..row(3.)
        },
        16.,
        6.,
        0.93,
        true,
    );
    let title = b.node(
        header,
        Node {
            flex_grow: 1.,
            min_width: px(0.),
            padding: UiRect::left(px(9.)),
            overflow: Overflow::clip(),
            ..default()
        },
    );
    b.text(title, &s.editor.document().name, 18., p.text, true);
    b.commands.entity(title).insert(Hover {
        base: Color::NONE,
        target: Color::NONE,
        tip: format!(
            "{} · {}",
            s.editor.document().name,
            if s.dirty { "Unsaved changes" } else { "Saved" }
        ),
        ..default()
    });
    b.hit(title, None);

    for (symbol, action, enabled) in [
        ("undo", Action::Undo, s.editor.can_undo()),
        ("redo", Action::Redo, s.editor.can_redo()),
    ] {
        let button = b.button(header, symbol, "", action, false);
        if !enabled {
            b.commands.entity(button).insert(Fade(0.3));
        }
    }
    b.button(header, "chevron-down", "", Action::Menu, s.menu);
}

fn zoom(b: &mut Builder, root: Entity, s: &Studio) {
    let p = b.p;
    let zoom = b.panel(
        root,
        Node {
            position_type: PositionType::Absolute,
            bottom: px(110.),
            left: px(28.),
            ..row(4.)
        },
        12.,
        0.,
        0.88,
        false,
    );
    b.commands.entity(zoom).remove::<BoxShadow>();
    b.commands
        .entity(zoom)
        .insert(BorderColor::all(color(p.line, 0.7)));
    b.button(zoom, "minus", "", Action::Zoom(0.8), false);
    let label = b.node(
        zoom,
        Node {
            width: px(38.),
            justify_content: JustifyContent::Center,
            ..row(0.)
        },
    );
    let label = b.text(
        label,
        format!("{:.0}%", s.viewport.zoom * 100.),
        10.,
        p.text,
        false,
    );
    b.commands.entity(label).insert(ZoomLabel);
    b.button(zoom, "plus", "", Action::Zoom(1.25), false);
    b.button(zoom, "fit-artwork", "", Action::Fit, false);
}

fn replay_controls(b: &mut Builder, root: Entity, s: &Studio) {
    if s.replay_view {
        let playback = b.panel(
            root,
            Node {
                position_type: PositionType::Absolute,
                top: px(20.),
                left: percent(50.),
                width: px(116.),
                margin: UiRect::left(px(-58.)),
                justify_content: JustifyContent::Center,
                ..row(0.)
            },
            14.,
            4.,
            0.93,
            true,
        );
        b.button(playback, "close", "Stop replay", Action::ReplayView, false);
    }
}

fn status(b: &mut Builder, root: Entity, s: &Studio) {
    let p = b.p;
    if s.status.starts_with("Could not")
        || s.status.starts_with("Saving")
        || s.status.starts_with("Opening")
        || s.status.starts_with("Exporting")
        || s.status.starts_with("Show this layer")
        || s.status.starts_with("Keep at least")
        || s.status.starts_with("Enable a landmark")
        || s.status.starts_with("No artwork in this view")
        || s.status.starts_with("Replay follows Reduced Motion")
    {
        let status = b.node(
            root,
            Node {
                max_width: px(320.),
                padding: UiRect::all(px(12.)),
                border_radius: BorderRadius::all(px(12.)),
                ..absolute(px(20.), px(76.))
            },
        );
        b.commands
            .entity(status)
            .insert(BackgroundColor(color(p.surface, 0.93)));
        b.hit(status, None);
        b.text(status, &s.status, 11., p.text, false);
    }
}
