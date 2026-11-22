//! Appearance of a standalone button in a contextual orbit.
use super::{Action, Builder, Hover, color, pigment, shadow};
use bevy::prelude::*;

impl Builder<'_, '_, '_> {
    pub(in crate::app::ui) fn orbit_button(
        &mut self,
        parent: Entity,
        node: Node,
        title: &str,
        symbol: &str,
        action: Action,
        active: bool,
    ) -> Entity {
        let button = self.action_button(parent, node, "", "", action, active);
        let title = title.to_owned();
        self.commands
            .entity(button)
            .entry::<Hover>()
            .and_modify(move |mut hover| hover.tip = title);
        if self.paper {
            self.style_paper_button(button, active, true, branch_pigment(action, symbol));
        } else {
            let p = self.p;
            let base = color(if active { p.wash } else { p.surface }, 0.94);
            let mut entity = self.commands.entity(button);
            entity.insert((
                BackgroundColor(base),
                BorderColor::all(color(if active { p.accent } else { p.line }, 0.85)),
                shadow(false),
            ));
            entity.entry::<Node>().and_modify(|mut node| {
                node.border = UiRect::all(px(1.));
                node.border_radius = BorderRadius::MAX;
            });
            entity.entry::<Hover>().and_modify(move |mut hover| {
                hover.base = base;
                hover.target = color(p.wash, 1.);
            });
            let glint = self.node(
                button,
                Node {
                    position_type: PositionType::Absolute,
                    top: px(2.),
                    left: px(10.),
                    width: px(14.),
                    height: px(1.),
                    ..default()
                },
            );
            self.commands
                .entity(glint)
                .insert(BackgroundColor(color(0xffffff, 0.46)));
        }
        button
    }
}

fn branch_pigment(action: Action, symbol: &str) -> Option<u32> {
    match action {
        Action::BrushOrbitEnter(0, index) => {
            match elmapa::biome_packs::catalog().get(index)?.id.as_str() {
                "core" => Some(pigment::SAGE),
                "desert" => Some(pigment::OCHRE),
                "alpine" => Some(pigment::BLUE),
                _ => None,
            }
        }
        Action::LandmarkEnter(..) => match symbol {
            "landmark-pack/brillanto.brilliant" => Some(pigment::STRAW),
            "landmark-pack/elmapa.basic" => Some(pigment::TERRACOTTA),
            "landmark-pack/elmapa.nature" => Some(pigment::SAGE),
            _ => None,
        },
        Action::RadialEnter(..) => match symbol {
            "typeface" => Some(pigment::OCHRE),
            "ink" => Some(pigment::BLUE),
            "typography" => Some(pigment::SEA_GLASS),
            _ => None,
        },
        _ => None,
    }
}
