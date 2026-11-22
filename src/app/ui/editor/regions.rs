//! Show the current region action and its next step above the toolbar.
use crate::app::{
    regions::{Mode, RegionAction},
    state::{Action, Appearance, Studio},
    ui::components::{Builder, color, pigment as paper, row},
};
use bevy::prelude::*;
use elmapa::scene::territories::{MAX_SPLIT_COUNT, pigment};

pub(super) const PAPER_STRIP_CLEARANCE: f32 = 170.;

pub(super) fn build(b: &mut Builder, root: Entity, s: &Studio) {
    let p = b.p;
    let paper = s.appearance == Appearance::Paper;
    let strip_layout = if paper {
        Node {
            max_width: percent(95.),
            flex_wrap: FlexWrap::Wrap,
            justify_content: JustifyContent::Center,
            row_gap: px(3.),
            ..row(5.)
        }
    } else {
        row(5.)
    };
    let strip = b.panel(root, strip_layout, 13., 5., 0.96, false);
    if let Some(count) = s.regions.split_count() {
        split_controls(b, strip, s, count);
        return;
    }
    let parent_name = s
        .active_region_parent()
        .and_then(|parent| s.editor.document().region_name(parent));
    if parent_name.is_some() {
        b.button_with_pigment(
            strip,
            "arrow-up",
            "",
            Action::Regions(RegionAction::ParentLayer),
            false,
            paper::LINEN,
        );
    }
    let selection = s.regions.selection.as_ref();
    let owner = selection.and_then(|selection| selection.owner);
    let active_layer = s.regions_layer();
    let hidden = active_layer.is_some_and(|layer| !layer.visible || layer.opacity <= 0.);
    let owners = active_layer.and_then(|layer| layer.territories.as_deref());
    let name = owner
        .and_then(|owner| owners?.names.get(owner))
        .map_or("Region", String::as_str);

    let title = b.node(
        strip,
        Node {
            padding: UiRect::horizontal(px(8.)),
            flex_wrap: if paper {
                FlexWrap::Wrap
            } else {
                FlexWrap::NoWrap
            },
            ..row(7.)
        },
    );
    if let Some((owner, data)) = owner.zip(owners) {
        let dot = b.node(
            title,
            Node {
                width: px(8.),
                height: px(8.),
                border_radius: BorderRadius::MAX,
                ..default()
            },
        );
        b.commands.entity(dot).insert(BackgroundColor(color(
            pigment(data, owner, b.atlas_style),
            1.,
        )));
        b.text(title, short_name(name), 14., p.text, true);
    } else if let Some(parent) = parent_name {
        b.text(
            title,
            format!("Inside {}", short_name(parent)),
            14.,
            p.text,
            true,
        );
    }

    let hint = match s.regions.mode {
        _ if hidden => "Show this Regions layer to edit it.",
        Mode::Split => "Drag a dashed border to adjust it.",
        Mode::Draw => "Drag from one edge of the selected land to another.",
        Mode::Assign(_) => "Click unassigned islands to add them.",
        Mode::Select if owner.is_some() => "Drag a border to adjust it.",
        Mode::Select if selection.is_some() => "Land selected",
        Mode::Select if owners.is_some_and(|data| !data.names.is_empty()) => {
            if parent_name.is_some() {
                "Click land inside the parent region."
            } else {
                "Click land or drag a shared border."
            }
        }
        Mode::Select => "Click land to select it.",
    };
    let hint = s.regions.message.as_deref().unwrap_or(hint);
    b.text(title, hint, 12., p.text, false);
    if hidden {
        return;
    }
    let button = |b: &mut Builder, label, action, active, pigment| {
        b.button_with_pigment(strip, "", label, Action::Regions(action), active, pigment);
    };
    match s.regions.mode {
        Mode::Draw => button(b, "Cancel", RegionAction::Done, false, None),
        Mode::Assign(_) => button(b, "Done", RegionAction::Done, true, Some(paper::SAGE)),
        Mode::Select if owner.is_some() => {
            button(
                b,
                "Draw border",
                RegionAction::Draw,
                false,
                Some(paper::SAND),
            );
            button(
                b,
                "Add islands",
                RegionAction::AddIslands,
                false,
                Some(paper::SEA_GLASS),
            );
            button(
                b,
                "Split into…",
                RegionAction::Split,
                true,
                Some(paper::STRAW),
            );
        }
        Mode::Select if selection.is_some() => {
            button(
                b,
                "Make region",
                RegionAction::Make,
                false,
                Some(paper::SAGE),
            );
            button(
                b,
                "Draw border",
                RegionAction::Draw,
                false,
                Some(paper::SAND),
            );
            button(
                b,
                "Split into…",
                RegionAction::Split,
                true,
                Some(paper::STRAW),
            );
        }
        Mode::Select | Mode::Split => {}
    }
}

fn split_controls(b: &mut Builder, strip: Entity, s: &Studio, count: usize) {
    let parent = s.split_parent_name();
    if let Some(parent) = parent {
        let scope = b.node(
            strip,
            Node {
                padding: UiRect::horizontal(px(8.)),
                ..row(6.)
            },
        );
        b.icon(scope, "layer-add", b.p.accent, 16.);
        b.text(
            scope,
            format!("Inside {}", short_name(parent)),
            14.,
            b.p.text,
            true,
        );
    }
    let stepper = b.node(strip, row(0.));
    for (icon, delta, enabled) in [
        ("minus", -1, count > 2),
        ("plus", 1, count < MAX_SPLIT_COUNT),
    ] {
        if delta > 0 {
            b.text(stepper, format!("{count} regions"), 14., b.p.text, true);
        }
        let item = b.button_with_pigment(
            stepper,
            icon,
            "",
            Action::Regions(RegionAction::SplitCount(delta)),
            false,
            paper::BLUE,
        );
        if !enabled {
            b.hit(item, None);
            b.commands
                .entity(item)
                .insert(crate::app::ui::feedback::Fade(0.32));
        }
    }
    let hint = b.node(
        strip,
        Node {
            padding: UiRect::horizontal(px(8.)),
            ..row(0.)
        },
    );
    b.text(
        hint,
        s.regions.message.as_deref().unwrap_or(if parent.is_some() {
            "Drag dashed borders to adjust."
        } else {
            "Drag a dashed border to adjust it."
        }),
        12.,
        b.p.text,
        false,
    );
    b.button(
        strip,
        "",
        "Cancel",
        Action::Regions(RegionAction::Done),
        false,
    );
    b.button_with_pigment(
        strip,
        "",
        if parent.is_some() {
            "Create child layer"
        } else {
            "Apply split"
        },
        Action::Regions(RegionAction::ApplySplit),
        true,
        paper::STRAW,
    );
}

fn short_name(name: &str) -> String {
    let mut summary: String = name.chars().take(22).collect();
    if summary.len() < name.len() {
        summary.push('…');
    }
    summary
}
