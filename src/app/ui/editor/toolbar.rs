//! Brush and placement toolbar, including advanced brush settings.
use crate::app::{
    brush_orbit::{BrushOrbit, DEFAULT_RIVER_FINISH},
    regions::RegionAction,
    state::{Action, Appearance, Studio, Tool},
    ui::components::{BUTTON_HEIGHT, Builder, PAPER_TOOL_SIZE, ToolIcon, color, column, row},
};
use bevy::prelude::*;
use elmapa::{brush_materials, document::Brush};

pub fn brush_icon(b: Brush) -> &'static str {
    ToolIcon::Brush(b).symbol()
}

const TOOLBAR_BOTTOM: f32 = 23.;
const PAPER_GAP: f32 = 8.;
const PAPER_OPTIONS_PADDING: f32 = 3.;
const PAPER_OPTIONS_HEIGHT: f32 = BUTTON_HEIGHT + 2. * (PAPER_OPTIONS_PADDING + 1.);

pub(in crate::app::ui) fn text_style_bottom(appearance: Appearance) -> f32 {
    match appearance {
        Appearance::Holo => 82.,
        Appearance::Paper => {
            TOOLBAR_BOTTOM + PAPER_TOOL_SIZE + PAPER_OPTIONS_HEIGHT + 2. * PAPER_GAP
        }
    }
}

pub(super) fn build(b: &mut Builder, root: Entity, s: &Studio) {
    let gap = match s.appearance {
        Appearance::Holo => 9.,
        Appearance::Paper => PAPER_GAP,
    };
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
        b.with_paper(|b| advanced_controls(b, bottom, s));
    }
    if s.appearance == Appearance::Paper && !s.regions.active && s.tool != Tool::Select {
        b.with_paper(|b| {
            let strip = b.panel(bottom, row(2.), 11., PAPER_OPTIONS_PADDING, 0.98, false);
            tool_options(b, strip, s);
        });
    }
    let toolbar = b.tool_row(bottom);
    for tool in tools(s) {
        let tip = match tool.action {
            Action::Brush(brush) if BrushOrbit::supports(brush) => {
                let modifier = if cfg!(target_os = "macos") {
                    "Option"
                } else {
                    "Alt"
                };
                Some(format!(
                    "{} · {modifier}-click or hold for materials",
                    tool.label
                ))
            }
            _ => None,
        };
        b.tool_button(
            toolbar,
            tool.icon,
            tool.label,
            tool.action,
            tool.selected,
            tip,
        );
    }
    if s.appearance == Appearance::Holo {
        tool_options(b, toolbar, s);
    }
}

#[derive(Clone, Copy)]
struct ToolButton {
    icon: ToolIcon,
    label: &'static str,
    action: Action,
    selected: bool,
}

fn tools(s: &Studio) -> [ToolButton; 11] {
    let button = |icon, label, action, selected| ToolButton {
        icon,
        label,
        action,
        selected,
    };
    let regions = s.regions.active;
    let brush = |brush: Brush| {
        button(
            ToolIcon::Brush(brush),
            brush.label(),
            Action::Brush(brush),
            s.tool == Tool::Brush && !regions && s.brush == brush,
        )
    };
    [
        button(
            ToolIcon::Select,
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
            ToolIcon::Text,
            "Text",
            Action::TextTool,
            s.tool == Tool::Text && !regions,
        ),
        button(
            ToolIcon::Landmarks,
            "Landmarks",
            Action::LandmarkTool,
            s.tool == Tool::Landmarks && !regions,
        ),
        brush(Brush::Erase),
        button(
            ToolIcon::Regions,
            "Regions",
            Action::Regions(RegionAction::Tool),
            regions,
        ),
    ]
}

fn tool_options(b: &mut Builder, parent: Entity, s: &Studio) {
    let paper = s.appearance == Appearance::Paper;
    let (label, size, actions, width, font_size) = match s.tool {
        Tool::Text => {
            text_style_control(b, parent, s);
            return;
        }
        Tool::Landmarks => (
            "Landmark size",
            s.landmarks.placement.size,
            (Action::LandmarkSize(-16.), Action::LandmarkSize(16.)),
            Val::Auto,
            11.,
        ),
        Tool::Brush if !s.regions.active => (
            "Brush size",
            s.radius,
            (Action::Radius(-15.), Action::Radius(15.)),
            px(31.),
            10.,
        ),
        _ => return,
    };
    if paper {
        let title = b.text(parent, label, 12., b.p.text, false);
        b.commands
            .entity(title)
            .entry::<Node>()
            .and_modify(|mut node| {
                node.margin = UiRect::horizontal(px(8.));
            });
    } else if s.tool == Tool::Brush {
        let divider = b.node(
            parent,
            Node {
                width: px(1.),
                height: px(20.),
                margin: UiRect::horizontal(px(5.)),
                ..default()
            },
        );
        b.commands
            .entity(divider)
            .insert(BackgroundColor(color(b.p.muted, 0.45)));
    }
    let (width, font_size) = if paper {
        (px(30.), 12.)
    } else {
        (width, font_size)
    };
    b.stepper(parent, format!("{size:.0}"), actions, width, font_size);
    if s.tool == Tool::Brush {
        b.button_with_pigment(
            parent,
            "settings",
            "",
            Action::Advanced,
            s.advanced,
            ToolIcon::Brush(s.brush).pigment(),
        );
    }
}

fn advanced_controls(b: &mut Builder, bottom: Entity, s: &Studio) {
    let p = b.p;
    let paper = s.appearance == Appearance::Paper;
    let advanced = b.panel(
        bottom,
        Node {
            width: px(if paper { 368. } else { 510. }),
            ..column(8.)
        },
        16.,
        14.,
        0.97,
        true,
    );
    if !paper {
        b.commands
            .entity(advanced)
            .insert(BorderColor::all(color(p.line, 1.)));
    }
    b.text(
        advanced,
        format!("{} brush settings", s.brush.label()),
        if paper { 18. } else { 12. },
        p.text,
        paper,
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

    if BrushOrbit::supports(s.brush) {
        let material = b.node(
            advanced,
            Node {
                width: percent(100.),
                justify_content: JustifyContent::SpaceBetween,
                ..row(0.)
            },
        );
        b.text(material, "Material", 12., p.text, false);
        let index = s.brush_orbit.current_index(s.brush);
        let name = index.map_or(DEFAULT_RIVER_FINISH, |index| {
            &brush_materials::catalog()[index].name
        });
        let button = b.button_with_pigment(
            material,
            "",
            name,
            Action::OpenBrushOrbit,
            s.brush_orbit.anchor.is_some(),
            ToolIcon::Brush(s.brush).pigment(),
        );
        if let Some(index) = index {
            let swatch = b.material_swatch(button, index, 16.);
            b.commands.entity(button).insert_children(0, &[swatch]);
        }
        b.icon(button, "chevron-down", p.muted, 13.);
    }
}

fn text_style_control(b: &mut Builder, parent: Entity, studio: &Studio) {
    let style = studio.text.style;
    let choice = b.button_with_pigment(
        parent,
        "",
        style.font.family(),
        Action::TextStyle,
        studio.text.style_open,
        ToolIcon::Text.pigment(),
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
