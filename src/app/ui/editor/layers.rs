//! Layer stack, visibility and layer settings.
mod regions;
use super::toolbar::brush_icon;
use crate::app::{
    regions::RegionAction,
    state::{Action, LayerBranch, Studio},
    ui::{
        ScrollArea,
        feedback::{Fade, Hover},
        widgets::{Builder, color, column, row},
    },
};
use bevy::prelude::*;
use elmapa::document::{Brush, Layer};

pub(super) fn build(b: &mut Builder, root: Entity, s: &Studio, scroll: f32) {
    let layers = b.panel(
        root,
        Node {
            position_type: PositionType::Absolute,
            top: px(78.),
            right: px(20.),
            width: px(220.),
            max_height: px((s.bounds.size.y - 188.).max(90.)),
            ..column(0.)
        },
        16.,
        5.,
        0.93,
        true,
    );
    let list = b.node(
        layers,
        Node {
            width: percent(100.),
            min_height: px(0.),
            overflow: Overflow::scroll_y(),
            ..column(3.)
        },
    );
    b.commands
        .entity(list)
        .insert((ScrollArea::Layers, ScrollPosition(Vec2::new(0., scroll))));
    let tree = regions::Tree::new(s);
    for layer in s.editor.document().layers.iter().rev().filter(|layer| {
        layer
            .territories
            .as_ref()
            .is_none_or(|data| data.parent.is_none())
    }) {
        let disclosure = tree.root_disclosure(layer);
        layer_row(b, list, s, layer, 0, disclosure);
        if disclosure.is_some_and(|(_, expanded)| expanded) {
            tree.rows(b, list, layer, 1);
        }
    }
    let controls = b.node(
        layers,
        Node {
            width: percent(100.),
            margin: UiRect::top(px(4.)),
            justify_content: JustifyContent::SpaceBetween,
            flex_shrink: 0.,
            ..row(0.)
        },
    );
    b.button(controls, "layer-add", "", Action::AddLayer, false);
    if s.regions.active {
        b.button(
            controls,
            "regions-layer-add",
            "",
            Action::Regions(RegionAction::NewLayer),
            false,
        );
    }
    b.button(controls, "settings", "", Action::LayerTools, s.layer_tools);
    if s.layer_tools {
        let controls = b.node(
            layers,
            Node {
                width: percent(100.),
                padding: UiRect::top(px(4.)),
                justify_content: JustifyContent::SpaceBetween,
                ..row(0.)
            },
        );
        for (icon, action) in [
            ("arrow-down", Action::MoveLayer(-1)),
            ("arrow-up", Action::MoveLayer(1)),
            ("minus", Action::Opacity(-0.1)),
            ("plus", Action::Opacity(0.1)),
            ("delete", Action::DeleteLayer),
        ] {
            b.button(controls, icon, "", action, false);
        }
    }
}

fn layer_row(
    b: &mut Builder,
    list: Entity,
    s: &Studio,
    layer: &Layer,
    depth: usize,
    disclosure: Option<(LayerBranch, bool)>,
) {
    let p = b.p;
    let selected = layer.id == s.editor.active_layer();
    let row = b.node(
        list,
        Node {
            height: px(36.),
            width: percent(100.),
            padding: UiRect {
                left: px(9. + regions::indent(depth)),
                right: px(3.),
                ..default()
            },
            border: UiRect::all(px(1.)),
            border_radius: BorderRadius::all(px(10.)),
            flex_shrink: 0.,
            ..row(5.)
        },
    );
    let base = color(p.wash, if selected { 0.95 } else { 0. });
    b.commands.entity(row).insert((
        BackgroundColor(base),
        BorderColor::all(color(p.accent, if selected { 0.32 } else { 0. })),
        Hover {
            base,
            target: color(p.wash, 0.65),
            progress: 0.,
            from: 0.,
            started: 0.,
            hovered: false,
            tip: if let Some(regions) = &layer.territories {
                format!(
                    "{} · {} regions · {:.0}% opacity",
                    layer.name,
                    regions.names.len(),
                    layer.opacity * 100.
                )
            } else {
                format!(
                    "{} · {} strokes · {} labels · {} landmarks · {:.0}% opacity{}",
                    layer.name,
                    layer.strokes.len(),
                    layer.labels.len(),
                    layer.landmarks.len(),
                    layer.opacity * 100.,
                    if layer.visible { "" } else { " · Hidden" }
                )
            },
        },
    ));
    b.hit(row, Some(Action::SelectLayer(layer.id)));
    if let Some((branch, expanded)) = disclosure {
        regions::disclosure(b, row, branch, expanded, &layer.name);
    }
    let icons = b.node(
        row,
        Node {
            flex_grow: 1.,
            min_width: px(0.),
            ..self::row(2.)
        },
    );
    if !layer.visible {
        b.commands.entity(icons).insert(Fade(0.35));
    }
    let brushes = layer.brushes();
    if let Some(regions) = &layer.territories {
        let icon = if regions.parent.is_some() {
            "layer"
        } else {
            "land"
        };
        b.icon(icons, icon, p.accent, 19.);
        let name = b.text(icons, &layer.name, 12., p.text, false);
        b.commands.entity(name).insert(Node {
            flex_shrink: 1.,
            min_width: px(0.),
            overflow: Overflow::clip(),
            ..default()
        });
    } else if brushes.is_empty() && layer.labels.is_empty() {
        let icon = b.node(
            icons,
            Node {
                width: px(24.),
                height: px(24.),
                border: UiRect::all(px(1.)),
                border_radius: BorderRadius::all(px(7.)),
                justify_content: JustifyContent::Center,
                ..self::row(0.)
            },
        );
        b.commands
            .entity(icon)
            .insert(BorderColor::all(color(p.line, 1.)));
        b.icon(icon, "layer", p.muted, 17.);
    }
    for brush in brushes {
        let hex = brush_color(brush, s.dark);
        let icon = b.node(
            icons,
            Node {
                width: px(20.),
                height: px(20.),
                border_radius: BorderRadius::all(px(7.)),
                justify_content: JustifyContent::Center,
                ..self::row(0.)
            },
        );
        b.commands
            .entity(icon)
            .insert(BackgroundColor(color(hex, 0.13)));
        b.icon(icon, brush_icon(brush), hex, 17.);
    }
    if !layer.landmarks.is_empty() {
        b.icon(icons, "landmark", p.accent, 20.);
    }
    if !layer.labels.is_empty() {
        let swatch = b.node(
            icons,
            Node {
                width: px(20.),
                height: px(20.),
                border_radius: BorderRadius::all(px(7.)),
                justify_content: JustifyContent::Center,
                ..self::row(0.)
            },
        );
        b.commands
            .entity(swatch)
            .insert(BackgroundColor(color(p.accent, 0.13)));
        b.icon(swatch, "text", p.accent, 17.);
    }
    let eye = b.node(
        row,
        Node {
            width: px(30.),
            height: px(30.),
            border_radius: BorderRadius::all(px(8.)),
            justify_content: JustifyContent::Center,
            flex_shrink: 0.,
            ..self::row(0.)
        },
    );
    b.commands.entity(eye).insert((
        BackgroundColor(Color::NONE),
        Hover {
            base: Color::NONE,
            target: color(p.line, 0.5),
            progress: 0.,
            from: 0.,
            started: 0.,
            hovered: false,
            tip: "Toggle visibility".into(),
        },
    ));
    b.hit(eye, Some(Action::Visibility(layer.id)));
    b.icon(
        eye,
        if layer.visible { "eye" } else { "eye-off" },
        p.muted,
        17.,
    );
}

fn brush_color(b: Brush, dark: bool) -> u32 {
    match (b, dark) {
        (Brush::Land, false) => 0x856d46,
        (Brush::Land, true) => 0xd4c18e,
        (Brush::Water, false) => 0x4e746f,
        (Brush::Water, true) => 0x9fbfb5,
        (Brush::Forest, false) => 0x59694b,
        (Brush::Forest, true) => 0xa2bb89,
        (Brush::Mountain, false) => 0x78644e,
        (Brush::Mountain, true) => 0xcab7a0,
        (Brush::River, false) => 0x48766e,
        (Brush::River, true) => 0x9cc1ba,
        (Brush::Path, false) => 0x89603c,
        (Brush::Path, true) => 0xd7aa83,
        (Brush::Erase, false) => 0x8c7478,
        (Brush::Erase, true) => 0xc7a7ae,
    }
}
