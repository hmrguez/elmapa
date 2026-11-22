//! Floating editor chrome in draw and hit order.
mod layers;
mod menu;
mod regions;
mod toolbar;

use super::{
    feedback::{Fade, Hover},
    widgets::{Builder, Palette, absolute, color, column, row},
};
use crate::app::{
    branding,
    state::{Action, Studio, ViewReplay},
};
use bevy::prelude::*;

pub use menu::{ChromeMotion, advance_motion, menu_animation};
pub(super) use toolbar::TEXT_STYLE_BOTTOM;
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

pub(super) fn replay_controls(
    b: &mut Builder,
    root: Entity,
    replay: ViewReplay,
    saving: bool,
    status: &str,
) {
    // This dock renders directly over the composed map on the window camera.
    b.p = Palette {
        surface: 0x0a0c08,
        text: 0xf3e3bd,
        muted: 0xaeaa93,
        line: 0x312c1c,
        accent: 0xf2d09a,
        wash: 0x222418,
    };
    let group = b.node(
        root,
        Node {
            position_type: PositionType::Absolute,
            bottom: px(24.),
            width: percent(100.),
            align_items: AlignItems::Center,
            ..column(8.)
        },
    );
    if status.starts_with("Could not save replay video") || status == "Replay video saved." {
        let message = b.panel(
            group,
            Node {
                max_width: px(500.),
                ..default()
            },
            10.,
            8.,
            0.97,
            false,
        );
        let text = b.text(message, status, 12., b.p.text, false);
        b.commands.entity(text).insert((
            TextLayout::default(),
            Node {
                max_width: percent(100.),
                ..default()
            },
        ));
    }
    let dock = b.panel(group, row(4.), 14., 4., 0.98, true);
    if saving {
        let busy = b.node(
            dock,
            Node {
                padding: UiRect::horizontal(px(11.)),
                ..row(0.)
            },
        );
        b.text(busy, "Saving MP4…", 12., b.p.muted, false);
    } else {
        b.button(dock, "save", "Save video", Action::SaveReplayVideo, false);
    }
    let divider = b.node(
        dock,
        Node {
            width: px(1.),
            height: px(16.),
            ..default()
        },
    );
    b.commands
        .entity(divider)
        .insert(BackgroundColor(color(b.p.line, 1.)));
    let label = match (replay, saving) {
        (_, true) => "Cancel · Esc",
        (ViewReplay::Playing, false) => "Stop · Esc",
        (ViewReplay::Finished, false) => "Exit · Esc",
    };
    b.button(dock, "close", label, Action::ReplayView, false);
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
