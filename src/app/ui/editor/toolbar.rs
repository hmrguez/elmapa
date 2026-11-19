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
use elmapa::{brush_materials, document::Brush};

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
pub(super) fn build(b: &mut Builder, root: Entity, s: &Studio) {
    let p = b.p;
    let bottom = b.node(
        root,
        Node {
            position_type: PositionType::Absolute,
            bottom: px(23.),
            left: px(0.),
            right: px(0.),
            align_items: AlignItems::Center,
            ..column(9.)
        },
    );
    if s.regions.active {
        super::regions::build(b, bottom, s);
    }
    if s.advanced && s.tool == Tool::Brush && !s.regions.active {
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

        let materials: Vec<_> = brush_materials::for_brush(s.brush).collect();
        if materials.len() > 1 {
            b.text(advanced, "Material", 11., p.muted, false);
            let choices = b.node(
                advanced,
                Node {
                    flex_wrap: FlexWrap::Wrap,
                    row_gap: px(5.),
                    ..row(5.)
                },
            );
            let current = s.brush_orbit.current_index(s.brush);
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
    let toolbar = b.panel(bottom, row(3.), 21., 7., 0.93, true);
    // A single decorative surface travels beneath the tool buttons. It has no Hit.
    let bubble = b.node(
        toolbar,
        Node {
            position_type: PositionType::Absolute,
            width: px(34.),
            height: px(34.),
            border_radius: BorderRadius::all(px(10.)),
            ..default()
        },
    );
    b.commands
        .entity(bubble)
        .insert((BrushBubble, BackgroundColor(color(p.wash, 1.))));
    let selecting = s.tool == Tool::Select;
    let select_tool = b.button(
        toolbar,
        "select",
        if selecting { "Select" } else { "" },
        Action::SelectTool,
        selecting,
    );
    tool_surface(b, select_tool, selecting);
    for brush in Brush::ALL
        .into_iter()
        .filter(|&brush| brush != Brush::Erase)
    {
        brush_button(b, toolbar, s, brush);
    }
    let adding_text = s.tool == Tool::Text;
    let text_tool = b.button(
        toolbar,
        "text",
        if adding_text { "Text" } else { "" },
        Action::TextTool,
        adding_text,
    );
    tool_surface(b, text_tool, adding_text);
    let placing_landmarks = s.tool == Tool::Landmarks;
    let landmark_tool = b.button(
        toolbar,
        "landmark",
        if placing_landmarks { "Landmarks" } else { "" },
        Action::LandmarkTool,
        placing_landmarks,
    );
    tool_surface(b, landmark_tool, placing_landmarks);
    brush_button(b, toolbar, s, Brush::Erase);
    let regions = b.button(
        toolbar,
        "regions",
        if s.regions.active { "Regions" } else { "" },
        Action::Regions(RegionAction::Tool),
        s.regions.active,
    );
    tool_surface(b, regions, s.regions.active);
    if adding_text {
        let style = s.text.style;
        let choice = b.button(
            toolbar,
            "",
            style.font.family(),
            Action::TextStyle,
            s.text.style_open,
        );
        let ink = b.swatch(choice, style.ink.color(), 9., None);
        let sample = b.text(choice, "Aa", 15., p.text, true);
        let font = match style.font {
            elmapa::document::LabelFont::Baskerville => b.assets.baskerville.clone(),
            elmapa::document::LabelFont::Didot => b.assets.didot.clone(),
            elmapa::document::LabelFont::Avenir => b.assets.avenir.clone(),
        };
        b.commands
            .entity(sample)
            .insert(TextFont::from_font_size(15.).with_font(font));
        b.commands.entity(choice).insert_children(0, &[sample, ink]);
        b.icon(choice, "chevron-down", p.muted, 13.);
    }
    if placing_landmarks {
        b.button(toolbar, "minus", "", Action::LandmarkSize(-16.), false);
        b.text(
            toolbar,
            format!("{:.0}", s.landmarks.placement.size),
            11.,
            p.text,
            false,
        );
        b.button(toolbar, "plus", "", Action::LandmarkSize(16.), false);
    }
    if s.tool == Tool::Brush && !s.regions.active {
        let divider = b.node(
            toolbar,
            Node {
                width: px(1.),
                height: px(20.),
                margin: UiRect::horizontal(px(5.)),
                ..default()
            },
        );
        b.commands
            .entity(divider)
            .insert(BackgroundColor(color(p.muted, 0.45)));
        b.button(toolbar, "minus", "", Action::Radius(-15.), false);
        let radius = b.node(
            toolbar,
            Node {
                width: px(31.),
                justify_content: JustifyContent::Center,
                ..row(0.)
            },
        );
        b.text(radius, format!("{:.0}", s.radius), 10., p.text, false);
        b.button(toolbar, "plus", "", Action::Radius(15.), false);
        b.button(toolbar, "settings", "", Action::Advanced, s.advanced);
    }
}

fn brush_button(b: &mut Builder, toolbar: Entity, s: &Studio, brush: Brush) {
    let selected = s.tool == Tool::Brush && !s.regions.active && s.brush == brush;
    let button = b.button(
        toolbar,
        brush_icon(brush),
        if selected { brush.label() } else { "" },
        Action::Brush(brush),
        selected,
    );
    tool_surface(b, button, selected);
}

fn tool_surface(b: &mut Builder, button: Entity, selected: bool) {
    // Preserve hover and click targets; selection is painted by the shared bubble.
    let target = color(b.p.wash, if selected { 0. } else { 0.45 });
    b.commands
        .entity(button)
        .insert(BackgroundColor(Color::NONE));
    b.commands
        .entity(button)
        .entry::<Hover>()
        .and_modify(move |mut hover| {
            hover.base = Color::NONE;
            hover.target = target;
        });
    if selected {
        b.commands.entity(button).insert(SelectedTool);
    }
}
