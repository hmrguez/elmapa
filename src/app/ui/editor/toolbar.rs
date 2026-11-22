//! Brush and placement toolbar, including advanced brush settings.
use crate::app::{
    brush_motion::{BrushBubble, SelectedTool},
    regions::RegionAction,
    state::{Action, Studio, Tool},
    ui::{
        feedback::Hover,
        widgets::{Builder, color, column, row},
    },
};
use bevy::prelude::*;
use elmapa::{biome_packs, document::Brush};

pub fn brush_icon(b: Brush) -> &'static str {
    match b {
        Brush::Land => "land",
        Brush::Water => "water",
        Brush::Forest => "forest",
        Brush::Mountain => "mountain",
        Brush::River => "river",
        Brush::Path => "trail",
        Brush::Erase => "eraser",
    }
}
const TOOLBAR_BOTTOM: f32 = 23.;
const TILE_SIZE: f32 = 44.;
const OPTIONS_PADDING: f32 = 3.;
const OPTIONS_HEIGHT: f32 = 34. + 2. * (OPTIONS_PADDING + 1.);
const GAP: f32 = 8.;

pub(in crate::app::ui) const TEXT_STYLE_BOTTOM: f32 =
    TOOLBAR_BOTTOM + TILE_SIZE + OPTIONS_HEIGHT + 2. * GAP;

pub(super) fn build(b: &mut Builder, root: Entity, s: &Studio) {
    let bottom = b.node(
        root,
        Node {
            position_type: PositionType::Absolute,
            bottom: px(TOOLBAR_BOTTOM),
            left: px(0.),
            right: px(0.),
            align_items: AlignItems::Center,
            ..column(GAP)
        },
    );
    if s.regions.active {
        super::regions::build(b, bottom, s);
    } else {
        match s.tool {
            Tool::Select if s.text.draft.is_none() => selection_controls(b, bottom, s),
            Tool::Brush => {
                if s.advanced {
                    advanced_controls(b, bottom, s);
                }
                size_controls(b, bottom, s);
            }
            Tool::Text => text_controls(b, bottom, s),
            Tool::Landmarks => size_controls(b, bottom, s),
            Tool::Select => {}
        }
    }
    tool_tiles(b, bottom, s);
}

fn advanced_controls(b: &mut Builder, bottom: Entity, s: &Studio) {
    let p = b.p;
    let advanced = b.panel(
        bottom,
        Node {
            width: px(510.),
            ..column(8.)
        },
        16.,
        14.,
        0.97,
        true,
    );
    b.commands
        .entity(advanced)
        .insert(BorderColor::all(color(p.line, 1.)));
    b.text(
        advanced,
        format!("{} brush settings", s.brush.label()),
        12.,
        p.text,
        false,
    );
    let strength = b.node(
        advanced,
        Node {
            width: percent(100.),
            justify_content: JustifyContent::SpaceBetween,
            ..row(0.)
        },
    );
    b.text(
        strength,
        format!("Strength  {:.0}%", s.strength * 100.),
        12.,
        p.text,
        false,
    );
    let buttons = b.node(strength, row(0.));
    b.button(buttons, "minus", "", Action::Strength(-0.1), false);
    b.button(buttons, "plus", "", Action::Strength(0.1), false);

    let materials: Vec<_> = biome_packs::materials(s.biome_pack, s.brush).collect();
    if materials.len() > 1 || s.biome_pack.is_some() && !materials.is_empty() {
        b.text(advanced, "Material", 11., p.muted, false);
        let choices = b.node(
            advanced,
            Node {
                flex_wrap: FlexWrap::Wrap,
                row_gap: px(5.),
                ..row(5.)
            },
        );
        let current = s.brush_orbit.current_index(s.brush, s.biome_pack);
        for (index, material) in materials {
            let active = current == Some(index);
            let button = b.button(
                choices,
                "",
                &material.name,
                Action::AdvancedBrushMaterial(index),
                active,
            );
            let swatch = b.material_swatch(button, index, 16.);
            b.commands.entity(button).insert_children(0, &[swatch]);
            b.commands.entity(button).insert(BorderColor::all(color(
                if active { p.accent } else { p.line },
                0.75,
            )));
            b.commands
                .entity(button)
                .entry::<Node>()
                .and_modify(|mut node| {
                    node.border = UiRect::all(px(1.));
                    node.padding = UiRect::horizontal(px(7.));
                });
        }
    }
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

    let regions = studio.regions.active;
    let brush = |brush: Brush, pigment| {
        (
            brush.label(),
            Action::Brush(brush),
            studio.tool == Tool::Brush && !regions && studio.brush == brush,
            pigment,
        )
    };
    // Keep the order in sync with the cells in assets/tool-icons/symbols.svg.
    let tools = [
        (
            "Select",
            Action::SelectTool,
            studio.tool == Tool::Select && !regions,
            color(0xd8caae, 1.),
        ),
        brush(Brush::Land, color(0xc9c99e, 1.)),
        brush(Brush::Water, color(0xb2c8c3, 1.)),
        brush(Brush::Forest, color(0xb7c3a7, 1.)),
        brush(Brush::Mountain, color(0xccbcab, 1.)),
        brush(Brush::River, color(0xb2c2ca, 1.)),
        brush(Brush::Path, color(0xe0c9a2, 1.)),
        (
            "Text",
            Action::TextTool,
            studio.tool == Tool::Text && !regions,
            color(0xdcbe96, 1.),
        ),
        (
            "Landmarks",
            Action::LandmarkTool,
            studio.tool == Tool::Landmarks && !regions,
            color(0xd4b19c, 1.),
        ),
        brush(Brush::Erase, color(0xd9b8ae, 1.)),
        (
            "Regions",
            Action::Regions(RegionAction::Tool),
            regions,
            color(0xddcfa2, 1.),
        ),
    ];
    for (index, (label, action, selected, pigment)) in tools.into_iter().enumerate() {
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
            label,
            action,
            selected,
        );
        let hover_face = pigment.mix(&Color::WHITE, 0.26);
        b.commands.entity(button).insert((
            BackgroundColor(pigment),
            BorderColor::all(color(0x5c3a22, 0.28)),
            ImageNode {
                image: b.assets.tool_paper.clone(),
                flip_x: index & 1 != 0,
                flip_y: index & 2 != 0,
                ..default()
            },
        ));
        if selected {
            b.commands.entity(button).insert(SelectedTool);
        }
        let pack = studio.biome_pack;
        b.commands
            .entity(button)
            .entry::<Hover>()
            .and_modify(move |mut hover| {
                hover.base = pigment;
                hover.target = hover_face;
                if let Action::Brush(brush) = action
                    && biome_packs::materials(pack, brush).nth(1).is_some()
                {
                    let modifier = if cfg!(target_os = "macos") {
                        "Option"
                    } else {
                        "Alt"
                    };
                    hover.tip = format!("{label} · {modifier}-click or hold for materials");
                }
            });
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

fn text_controls(b: &mut Builder, bottom: Entity, studio: &Studio) {
    let strip = b.panel(bottom, row(0.), 11., OPTIONS_PADDING, 0.98, false);
    let style = studio.text.style;
    let choice = b.button(
        strip,
        "",
        style.font.family(),
        Action::TextStyle,
        studio.text.style_open,
    );
    let ink = b.swatch(choice, style.ink.color_in(b.atlas_style), 9., None);
    let sample = b.text(choice, "Aa", 15., b.p.text, true);
    let font = match style.font {
        elmapa::document::LabelFont::Baskerville => b.assets.baskerville.clone(),
        elmapa::document::LabelFont::Didot => b.assets.didot.clone(),
        elmapa::document::LabelFont::Avenir => b.assets.avenir.clone(),
    };
    b.commands
        .entity(sample)
        .insert(TextFont::from_font_size(15.).with_font(font));
    b.commands.entity(choice).insert_children(0, &[sample, ink]);
    b.icon(choice, "chevron-down", b.p.muted, 13.);
}

fn selection_controls(b: &mut Builder, bottom: Entity, studio: &Studio) {
    use elmapa::document::{SelectionId, Specimen};

    let items = &studio.selection.items;
    let count = items.len();
    let (title, hint) = if let Some(path) = &studio.path_selection {
        let title = studio
            .editor
            .linear_stroke(path.id)
            .map_or("Course", |stroke| stroke.brush.label());
        (
            title.to_string(),
            "Drag the hand to move · Drag a node to shape",
        )
    } else if let [item] = items.as_slice() {
        let title = match item.id {
            SelectionId::Relief { key, .. } => match key.specimen {
                Specimen::Tree { .. } => "Tree",
                Specimen::Peak(_) => "Peak",
            },
            _ => "Selection",
        };
        (title.into(), "Drag to move · Shift-click to add")
    } else if count > 1 {
        (
            format!("{count} selected"),
            "Drag to move together · Shift-click to toggle",
        )
    } else if studio.landmarks.selected.is_some() {
        ("Landmark".into(), "Drag to move · Shift-click to add")
    } else if studio.text.selected.is_some() {
        ("Label".into(), "Drag to move · Shift-click to add")
    } else {
        ("Select artwork".into(), "Drag a box · Shift-click to add")
    };
    let panel = b.panel(bottom, row(8.), 15., 4., 0.95, false);
    let copy = b.node(
        panel,
        Node {
            padding: UiRect::horizontal(px(10.)),
            ..column(0.)
        },
    );
    b.text(copy, title, 17., b.p.text, true);
    if studio.bounds.size.x >= 520. {
        b.text(copy, hint, 11., b.p.text, false);
    }
    if let Some(path) = &studio.path_selection
        && path.nodes().len() > 1
    {
        for (step, symbol) in [(-1, "minus"), (1, "plus")] {
            if (-2..=2).contains(&(path.density + step)) {
                b.button(panel, symbol, "", Action::PathNodeDensity(step), false);
            }
        }
    }
    if studio.has_selection() {
        b.button(panel, "close", "", Action::ClearSelection, false);
    }
}
