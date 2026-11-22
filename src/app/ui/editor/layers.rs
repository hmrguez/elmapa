//! Layer stack, visibility and layer settings.
mod regions;
use crate::app::{
    regions::RegionAction,
    state::{Action, Appearance, LayerBranch, Studio},
    ui::{
        ScrollArea,
        components::{Builder, LayerRow, ToolIcon, column, row},
        feedback::Fade,
    },
};
use bevy::prelude::*;
use elmapa::document::Layer;

pub(super) fn build(b: &mut Builder, root: Entity, s: &Studio, scroll: f32) {
    // Leave room for the Regions strip to wrap above the toolbar on narrow windows.
    let bottom = if s.appearance == Appearance::Paper && s.regions.active && s.bounds.size.x < 1024.
    {
        170.
    } else {
        110.
    };
    let layers = b.panel(
        root,
        Node {
            position_type: PositionType::Absolute,
            top: px(78.),
            right: px(20.),
            width: px(220.),
            max_height: px((s.bounds.size.y - 78. - bottom).max(90.)),
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
    b.layer_panel_button(controls, "layer-add", Action::AddLayer, false);
    if s.regions.active {
        b.layer_panel_button(
            controls,
            "regions-layer-add",
            Action::Regions(RegionAction::NewLayer),
            false,
        );
    }
    b.layer_panel_button(controls, "settings", Action::LayerTools, s.layer_tools);
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
            b.layer_panel_button(controls, icon, action, false);
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
    let tip = if let Some(regions) = &layer.territories {
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
    };
    let row = b.layer_row(
        list,
        Node {
            height: px(36.),
            width: percent(100.),
            padding: UiRect {
                left: px(9. + regions::indent(depth)),
                right: px(3.),
                ..default()
            },
            flex_shrink: 0.,
            ..row(5.)
        },
        LayerRow::Layer,
        selected,
        tip,
    );
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
    let has_labels = !layer.labels.is_empty();
    let has_landmarks = !layer.landmarks.is_empty();
    let icon_count = brushes.len() + usize::from(has_labels) + usize::from(has_landmarks);
    let icon_size = if icon_count > 7 { 15. } else { 20. };
    if let Some(regions) = &layer.territories {
        let icon = if regions.parent.is_some() {
            "layer"
        } else {
            "land"
        };
        b.layer_type_icon(icons, icon, true);
        let name = b.text(icons, &layer.name, 12., p.text, false);
        b.commands.entity(name).insert(Node {
            flex_shrink: 1.,
            min_width: px(0.),
            overflow: Overflow::clip(),
            ..default()
        });
    } else if brushes.is_empty() && !has_labels {
        b.layer_type_icon(icons, "layer", false);
    }
    for brush in brushes {
        b.layer_tool_icon(icons, ToolIcon::Brush(brush), icon_size);
    }
    if has_landmarks {
        b.layer_tool_icon(icons, ToolIcon::Landmarks, icon_size);
    }
    if has_labels {
        b.layer_tool_icon(icons, ToolIcon::Text, icon_size);
    }
    b.layer_visibility(row, layer.visible, Action::Visibility(layer.id));
}
