//! Paper styling for shared chrome components.
use super::{Appearance, Builder, Hover, Palette, color};
use bevy::{prelude::*, ui::widget::NodeImageMode};

pub(super) const INK: u32 = 0x5c3a22;

pub(in crate::app::ui) mod pigment {
    pub const LINEN: u32 = 0xd8caae;
    pub const SEA_GLASS: u32 = 0xb2c8c3;
    pub const SAGE: u32 = 0xb7c3a7;
    pub const CLAY: u32 = 0xccbcab;
    pub const BLUE: u32 = 0xb2c2ca;
    pub const SAND: u32 = 0xe0c9a2;
    pub const OCHRE: u32 = 0xdcbe96;
    pub const TERRACOTTA: u32 = 0xd4b19c;
    pub const ROSE: u32 = 0xd9b8ae;
    pub const STRAW: u32 = 0xddcfa2;
}

const PALETTE: Palette = Palette {
    surface: 0xf3e7cf,
    text: 0x3d2b1d,
    muted: 0x695542,
    line: 0x9a7b59,
    accent: INK,
    wash: pigment::SAND,
};

pub(super) fn corners() -> BorderRadius {
    BorderRadius::all(px(5.))
}

impl Builder<'_, '_, '_> {
    /// Applies Paper to the shared components built for a migrated surface.
    pub(in crate::app::ui) fn with_paper<T>(&mut self, build: impl FnOnce(&mut Self) -> T) -> T {
        if self.appearance == Appearance::Holo {
            return build(self);
        }
        let previous = std::mem::replace(&mut self.p, PALETTE);
        let was_paper = std::mem::replace(&mut self.paper, true);
        let result = build(self);
        self.p = previous;
        self.paper = was_paper;
        result
    }

    pub(super) fn paper_grain(&self) -> ImageNode {
        ImageNode::new(self.assets.tool_paper.clone())
    }

    pub(in crate::app::ui) fn focused_field(&mut self, parent: Entity, mut node: Node) -> Entity {
        let (radius, stroke, outline) = if self.paper {
            (corners(), 2., 1.)
        } else {
            (BorderRadius::all(px(9.)), 1., 0.7)
        };
        node.border_radius = radius;
        node.border = UiRect::all(px(stroke));
        let field = self.node(parent, node);
        self.commands.entity(field).insert((
            BackgroundColor(color(self.p.wash, 0.85)),
            BorderColor::all(color(self.p.accent, outline)),
        ));
        field
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

    pub(super) fn style_paper_button(
        &mut self,
        button: Entity,
        active: bool,
        standalone: bool,
        pigment: Option<u32>,
    ) {
        if standalone {
            self.style_paper_panel(button, false);
        }
        let surface = color(self.p.surface, 1.);
        let wash = color(self.p.wash, 1.);
        let (base, target) = if let Some(pigment) = pigment {
            let pigment = color(pigment, 1.);
            let base = if active || standalone {
                pigment
            } else {
                surface.mix(&pigment, 0.6)
            };
            (base, base.mix(&Color::WHITE, 0.26))
        } else if active {
            (wash, wash.mix(&Color::WHITE, 0.18))
        } else if standalone {
            (surface, wash)
        } else {
            (wash.with_alpha(0.), wash.with_alpha(0.65))
        };
        let (border, stroke) = if standalone && !active {
            (color(self.p.line, 0.65), 1.)
        } else {
            (color(INK, if active { 1. } else { 0. }), 2.)
        };
        let mut entity = self.commands.entity(button);
        entity.insert((BackgroundColor(base), BorderColor::all(border)));
        entity.entry::<Node>().and_modify(move |mut node| {
            node.border_radius = corners();
            node.border = UiRect::all(px(stroke));
        });
        entity.entry::<Hover>().and_modify(move |mut hover| {
            hover.base = base;
            hover.target = target;
        });
    }
}
