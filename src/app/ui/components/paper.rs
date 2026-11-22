//! Paper styling for shared chrome components.
use super::{Appearance, Builder, Hover, Palette, color};
use bevy::{prelude::*, ui::widget::NodeImageMode};

pub(super) const INK: u32 = 0x5c3a22;

const PALETTE: Palette = Palette {
    surface: 0xf3e7cf,
    text: 0x3d2b1d,
    muted: 0x695542,
    line: 0x9a7b59,
    accent: INK,
    wash: 0xe0c9a2,
};

pub(super) fn corners() -> BorderRadius {
    BorderRadius::all(px(5.))
}

impl Builder<'_, '_, '_> {
    /// Applies Paper to the shared components built for a migrated surface.
    pub(in crate::app::ui) fn with_paper(&mut self, build: impl FnOnce(&mut Self)) {
        if self.appearance == Appearance::Holo {
            build(self);
            return;
        }
        let previous = std::mem::replace(&mut self.p, PALETTE);
        let was_paper = std::mem::replace(&mut self.paper, true);
        build(self);
        self.p = previous;
        self.paper = was_paper;
    }

    pub(super) fn paper_grain(&self) -> ImageNode {
        ImageNode::new(self.assets.tool_paper.clone())
    }

    pub(super) fn style_paper_panel(&mut self, entity: Entity, large: bool) {
        let grain = ImageNode {
            color: Color::WHITE.with_alpha(0.32),
            image_mode: NodeImageMode::Stretch,
            ..self.paper_grain()
        };
        self.commands.entity(entity).insert((
            BackgroundColor(color(self.p.surface, 1.)),
            BorderColor::all(color(self.p.line, 0.65)),
            BoxShadow(vec![ShadowStyle {
                color: color(0x3d2b1d, 0.19),
                x_offset: px(1.),
                y_offset: px(if large { 4. } else { 2. }),
                blur_radius: px(if large { 8. } else { 5. }),
                spread_radius: px(-1.),
            }]),
            grain,
        ));
    }

    pub(super) fn style_paper_button(&mut self, button: Entity, active: bool) {
        let wash = color(self.p.wash, 1.);
        let base = if active { wash } else { wash.with_alpha(0.) };
        let target = if active {
            wash.mix(&Color::WHITE, 0.18)
        } else {
            wash.with_alpha(0.65)
        };
        let border = color(INK, if active { 1. } else { 0. });
        let mut entity = self.commands.entity(button);
        entity.insert((BackgroundColor(base), BorderColor::all(border)));
        entity.entry::<Node>().and_modify(|mut node| {
            node.border_radius = corners();
            node.border = UiRect::all(px(2.));
        });
        entity.entry::<Hover>().and_modify(move |mut hover| {
            hover.base = base;
            hover.target = target;
        });
    }
}
