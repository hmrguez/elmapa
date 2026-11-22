//! Region styling stays beside the land the user clicked.
use super::{
    orbit::{Orbit, RadialElement, action_orbit},
    text::edit_text,
};
use crate::app::{
    regions::{RegionAction, StyleAction, StyleField},
    state::{Action, Studio},
    ui::components::{Builder, column, pigment, row},
};
use bevy::prelude::*;

#[derive(Component)]
pub struct RegionOrbit;

pub(super) fn build(b: &mut Builder, root: Entity, studio: &Studio) {
    let style = &studio.regions.style;
    let Some((layer, _, owner)) = studio.region_style_target() else {
        return;
    };
    if style.anchor.is_none() {
        return;
    }
    let orbit = action_orbit(
        b,
        root,
        &studio.region_style_actions(),
        &style.navigation,
        &format!("region/{layer}/{owner}"),
        |depth, index| action(StyleAction::Enter(depth, index)),
        action(StyleAction::Back),
    );
    b.commands.entity(orbit).insert(RegionOrbit);

    let Some(edit) = &style.edit else {
        return;
    };
    let panel = b.panel(
        root,
        Node {
            position_type: PositionType::Absolute,
            width: px(300.),
            ..column(8.)
        },
        17.,
        11.,
        0.98,
        true,
    );
    b.commands.entity(panel).insert(RadialElement::card(orbit));
    let title = match edit.field {
        StyleField::Name => "Rename region",
        StyleField::Color => "Custom hex color",
    };
    b.text(panel, title, 16., b.p.text, true);

    let field = b.focused_field(
        panel,
        Node {
            width: percent(100.),
            height: px(34.),
            padding: UiRect::horizontal(px(9.)),
            overflow: Overflow::clip(),
            ..row(0.)
        },
    );
    edit_text(b, field, &edit.input, &style.composition, 14.);
    if let Some(error) = &style.error {
        b.text(panel, error, 12., b.p.text, false);
    }
    let buttons = b.node(
        panel,
        Node {
            width: percent(100.),
            justify_content: JustifyContent::End,
            ..row(4.)
        },
    );
    b.button(buttons, "", "Cancel", action(StyleAction::Cancel), false);
    let (apply, pigment) = match edit.field {
        StyleField::Name => ("Rename", pigment::OCHRE),
        StyleField::Color => ("Apply", pigment::BLUE),
    };
    b.button_with_pigment(
        buttons,
        "check",
        apply,
        action(StyleAction::Apply),
        true,
        pigment,
    );
}

pub fn region_selection_follow(
    studio: Res<Studio>,
    mut orbits: Query<&mut Orbit, With<RegionOrbit>>,
) {
    let Some(point) = studio.regions.style.anchor else {
        return;
    };
    let view = studio.viewport.transform(studio.bounds);
    let center = view.origin + Vec2::new(point.x, point.y) * view.scale;
    for mut orbit in &mut orbits {
        orbit.center = center;
        orbit.radius = Vec2::splat(60.);
        orbit.card_height = if studio.regions.style.error.is_some() {
            162.
        } else {
            139.
        };
        orbit.hidden = studio.regions.is_dragging();
    }
}

fn action(action: StyleAction) -> Action {
    Action::Regions(RegionAction::Style(action))
}
