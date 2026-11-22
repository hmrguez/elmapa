//! Paper UI tool row and its contextual controls.
use super::{
    Action, BrushBubble, Builder, Studio, TOOLBAR_BOTTOM, Tool, color, row, text_style_control,
    tool_surface, tools,
};
use bevy::prelude::*;

const TILE_SIZE: f32 = 44.;
const OPTIONS_PADDING: f32 = 3.;
const OPTIONS_HEIGHT: f32 = 34. + 2. * (OPTIONS_PADDING + 1.);
pub(super) const GAP: f32 = 8.;

pub(super) const TEXT_STYLE_BOTTOM: f32 = TOOLBAR_BOTTOM + TILE_SIZE + OPTIONS_HEIGHT + 2. * GAP;

pub(super) fn build(b: &mut Builder, bottom: Entity, studio: &Studio) {
    if !studio.regions.active {
        match studio.tool {
            Tool::Brush | Tool::Landmarks => size_controls(b, bottom, studio),
            Tool::Text => {
                let strip = b.panel(bottom, row(0.), 11., OPTIONS_PADDING, 0.98, false);
                text_style_control(b, strip, studio);
            }
            Tool::Select => {}
        }
    }
    tool_tiles(b, bottom, studio);
}

fn tool_tiles(b: &mut Builder, bottom: Entity, studio: &Studio) {
    let tiles = b.node(bottom, row(6.));
    let outline = b.node(
        tiles,
        Node {
            position_type: PositionType::Absolute,
            width: px(0.),
            height: px(0.),
            border: UiRect::all(px(2.)),
            ..default()
        },
    );
    // The row has no fill or hit area. Only this outline moves between the tiles.
    b.commands.entity(outline).insert((
        BrushBubble,
        ZIndex(1),
        BorderColor::all(color(0x5c3a22, 1.)),
    ));

    let tools = tools(studio);
    // Pigments follow the tool and illustration order.
    let pigments = [
        0xd8caae, 0xc9c99e, 0xb2c8c3, 0xb7c3a7, 0xccbcab, 0xb2c2ca, 0xe0c9a2, 0xdcbe96, 0xd4b19c,
        0xd9b8ae, 0xddcfa2,
    ];
    for (index, (tool, pigment)) in tools.into_iter().zip(pigments).enumerate() {
        let pigment = color(pigment, 1.);
        let button = b.action_button(
            tiles,
            Node {
                width: px(TILE_SIZE),
                height: px(TILE_SIZE),
                border_radius: BorderRadius::all(px(5.)),
                border: UiRect::all(px(1.)),
                flex_shrink: 0.,
                justify_content: JustifyContent::Center,
                ..row(0.)
            },
            "",
            tool.label,
            tool.action,
            tool.selected,
        );
        let hover_face = pigment.mix(&Color::WHITE, 0.26);
        b.commands.entity(button).insert((
            BorderColor::all(color(0x5c3a22, 0.28)),
            ImageNode {
                image: b.assets.tool_paper.clone(),
                flip_x: index & 1 != 0,
                flip_y: index & 2 != 0,
                ..default()
            },
        ));
        tool_surface(b, button, studio, tool, pigment, hover_face);
        let x = index as f32 * 128.;
        let symbol = b.node(
            button,
            Node {
                width: px(31.),
                height: px(31.),
                flex_shrink: 0.,
                ..default()
            },
        );
        b.commands.entity(symbol).insert(ImageNode {
            image: b.assets.tool_symbols.clone(),
            rect: Some(Rect::new(x, 0., x + 128., 128.)),
            ..default()
        });
    }
}

fn size_controls(b: &mut Builder, bottom: Entity, studio: &Studio) {
    let strip = b.panel(bottom, row(2.), 11., OPTIONS_PADDING, 0.98, false);
    let (label, size, decrease, increase) = if studio.tool == Tool::Landmarks {
        (
            "Landmark size",
            studio.landmarks.placement.size,
            Action::LandmarkSize(-16.),
            Action::LandmarkSize(16.),
        )
    } else {
        (
            "Brush size",
            studio.radius,
            Action::Radius(-15.),
            Action::Radius(15.),
        )
    };
    let title = b.text(strip, label, 12., b.p.text, false);
    b.commands
        .entity(title)
        .entry::<Node>()
        .and_modify(|mut node| {
            node.margin = UiRect::horizontal(px(8.));
        });
    b.button(strip, "minus", "", decrease, false);
    let value = b.node(
        strip,
        Node {
            width: px(30.),
            justify_content: JustifyContent::Center,
            ..row(0.)
        },
    );
    b.text(value, format!("{size:.0}"), 12., b.p.text, false);
    b.button(strip, "plus", "", increase, false);
    if studio.tool == Tool::Brush {
        b.button(strip, "settings", "", Action::Advanced, studio.advanced);
    }
}
