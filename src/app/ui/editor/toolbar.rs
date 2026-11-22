//! Brush and placement toolbar, including advanced brush settings.
mod paper;

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

pub(in crate::app::ui) fn text_style_bottom(paper_ui: bool) -> f32 {
    if paper_ui {
        paper::TEXT_STYLE_BOTTOM
    } else {
        82.
    }
}

pub(super) fn build(b: &mut Builder, root: Entity, s: &Studio) {
    let gap = if s.paper_ui { paper::GAP } else { 9. };
    let bottom = b.node(
        root,
        Node {
            position_type: PositionType::Absolute,
            bottom: px(TOOLBAR_BOTTOM),
            left: px(0.),
            right: px(0.),
            align_items: AlignItems::Center,
            ..column(gap)
        },
    );
    if s.regions.active {
        super::regions::build(b, bottom, s);
    } else if s.tool == Tool::Select && s.text.draft.is_none() {
        selection_controls(b, bottom, s);
    }
    if s.advanced && s.tool == Tool::Brush && !s.regions.active {
        advanced_controls(b, bottom, s);
    }
    if s.paper_ui {
        paper::build(b, bottom, s);
    } else {
        classic_toolbar(b, bottom, s);
    }
}

#[derive(Clone, Copy)]
struct ToolButton {
    symbol: &'static str,
    label: &'static str,
    action: Action,
    selected: bool,
}

fn tools(s: &Studio) -> [ToolButton; 11] {
    let button = |symbol, label, action, selected| ToolButton {
        symbol,
        label,
        action,
        selected,
    };
    let regions = s.regions.active;
    let brush = |brush: Brush| {
        button(
            brush_icon(brush),
            brush.label(),
            Action::Brush(brush),
            s.tool == Tool::Brush && !regions && s.brush == brush,
        )
    };
    // This order also matches the icon cells in assets/tool-icons/symbols.svg.
    [
        button(
            "select",
            "Select",
            Action::SelectTool,
            s.tool == Tool::Select && !regions,
        ),
        brush(Brush::Land),
        brush(Brush::Water),
        brush(Brush::Forest),
        brush(Brush::Mountain),
        brush(Brush::River),
        brush(Brush::Path),
        button(
            "text",
            "Text",
            Action::TextTool,
            s.tool == Tool::Text && !regions,
        ),
        button(
            "landmark",
            "Landmarks",
            Action::LandmarkTool,
            s.tool == Tool::Landmarks && !regions,
        ),
        brush(Brush::Erase),
        button(
            "regions",
            "Regions",
            Action::Regions(RegionAction::Tool),
            regions,
        ),
    ]
}

fn tool_surface(
    b: &mut Builder,
    button: Entity,
    studio: &Studio,
    tool: ToolButton,
    base: Color,
    target: Color,
) {
    let pack = studio.biome_pack;
    b.commands.entity(button).insert(BackgroundColor(base));
    b.commands
        .entity(button)
        .entry::<Hover>()
        .and_modify(move |mut hover| {
            hover.base = base;
            hover.target = target;
            if let Action::Brush(brush) = tool.action
                && biome_packs::materials(pack, brush).nth(1).is_some()
            {
                let modifier = if cfg!(target_os = "macos") {
                    "Option"
                } else {
                    "Alt"
                };
                hover.tip = format!("{} · {modifier}-click or hold for materials", tool.label);
            }
        });
    if tool.selected {
        b.commands.entity(button).insert(SelectedTool);
    }
}

fn classic_toolbar(b: &mut Builder, bottom: Entity, s: &Studio) {
    let p = b.p;
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
    for tool in tools(s) {
        let button = b.button(
            toolbar,
            tool.symbol,
            if tool.selected { tool.label } else { "" },
            tool.action,
            tool.selected,
        );
        let hover = color(p.wash, if tool.selected { 0. } else { 0.45 });
        tool_surface(b, button, s, tool, Color::NONE, hover);
    }
    if s.tool == Tool::Text {
        text_style_control(b, toolbar, s);
    }
    if s.tool == Tool::Landmarks {
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

fn text_style_control(b: &mut Builder, parent: Entity, studio: &Studio) {
    let style = studio.text.style;
    let choice = b.button(
        parent,
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
