//! Toolbar components with Holo and Paper faces.
use super::{Appearance, BUTTON_HEIGHT, Builder, Hover, color, paper, pigment, row};
use crate::app::{
    brush_motion::{BrushBubble, SelectedTool},
    state::Action,
};
use bevy::prelude::*;
use elmapa::document::Brush;

pub(in crate::app::ui) const PAPER_TOOL_SIZE: f32 = 44.;
const PAPER_SYMBOL_CELL: f32 = 128.;

#[derive(Clone, Copy)]
pub(in crate::app::ui) enum ToolIcon {
    Select,
    Brush(Brush),
    Text,
    Landmarks,
    Regions,
}

impl ToolIcon {
    pub(in crate::app::ui) fn symbol(self) -> &'static str {
        self.artwork().0
    }

    pub(in crate::app::ui) fn pigment(self) -> u32 {
        self.artwork().2
    }

    // The cell selects an illustration from assets/tool-icons/symbols.svg.
    fn artwork(self) -> (&'static str, usize, u32) {
        match self {
            Self::Select => ("select", 0, pigment::LINEN),
            Self::Brush(Brush::Land) => ("land", 1, 0xc9c99e),
            Self::Brush(Brush::Water) => ("water", 2, pigment::SEA_GLASS),
            Self::Brush(Brush::Forest) => ("forest", 3, pigment::SAGE),
            Self::Brush(Brush::Mountain) => ("mountain", 4, pigment::CLAY),
            Self::Brush(Brush::River) => ("river", 5, pigment::BLUE),
            Self::Brush(Brush::Path) => ("trail", 6, pigment::SAND),
            Self::Text => ("text", 7, pigment::OCHRE),
            Self::Landmarks => ("landmark", 8, pigment::TERRACOTTA),
            Self::Brush(Brush::Erase) => ("eraser", 9, pigment::ROSE),
            Self::Regions => ("regions", 10, pigment::STRAW),
        }
    }
}

impl Builder<'_, '_, '_> {
    pub(in crate::app::ui) fn tool_row(&mut self, parent: Entity) -> Entity {
        let (row, indicator, background, border, z) = match self.appearance {
            Appearance::Holo => (
                self.panel(parent, row(3.), 21., 7., 0.93, true),
                Node {
                    width: px(34.),
                    height: px(BUTTON_HEIGHT),
                    border_radius: BorderRadius::all(px(10.)),
                    ..default()
                },
                color(self.p.wash, 1.),
                Color::NONE,
                0,
            ),
            Appearance::Paper => (
                self.node(parent, row(6.)),
                Node {
                    width: px(0.),
                    height: px(0.),
                    border: UiRect::all(px(2.)),
                    ..default()
                },
                Color::NONE,
                color(paper::INK, 1.),
                1,
            ),
        };
        // This decoration follows the selected button and has no hit area.
        let marker = self.node(
            row,
            Node {
                position_type: PositionType::Absolute,
                ..indicator
            },
        );
        self.commands.entity(marker).insert((
            BrushBubble,
            BackgroundColor(background),
            BorderColor::all(border),
            ZIndex(z),
        ));
        row
    }

    pub(in crate::app::ui) fn tool_button(
        &mut self,
        parent: Entity,
        icon: ToolIcon,
        label: &str,
        action: Action,
        selected: bool,
        tip: Option<String>,
    ) -> Entity {
        let (button, base, target) = match self.appearance {
            Appearance::Holo => (
                self.button(
                    parent,
                    icon.symbol(),
                    if selected { label } else { "" },
                    action,
                    selected,
                ),
                Color::NONE,
                color(self.p.wash, if selected { 0. } else { 0.45 }),
            ),
            Appearance::Paper => self.paper_tool_button(parent, icon, label, action, selected),
        };
        let mut entity = self.commands.entity(button);
        entity.insert(BackgroundColor(base));
        entity.entry::<Hover>().and_modify(move |mut hover| {
            hover.base = base;
            hover.target = target;
            if let Some(tip) = tip {
                hover.tip = tip;
            }
        });
        if selected {
            entity.insert(SelectedTool);
        }
        button
    }

    fn paper_tool_button(
        &mut self,
        parent: Entity,
        icon: ToolIcon,
        label: &str,
        action: Action,
        selected: bool,
    ) -> (Entity, Color, Color) {
        let pigment = color(icon.pigment(), 1.);
        let button = self.action_button(
            parent,
            Node {
                width: px(PAPER_TOOL_SIZE),
                height: px(PAPER_TOOL_SIZE),
                border_radius: paper::corners(),
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
        self.paper_tool_art(button, icon, 31.);
        (button, pigment, pigment.mix(&Color::WHITE, 0.26))
    }

    pub(super) fn paper_tool_art(&mut self, tile: Entity, icon: ToolIcon, size: f32) {
        let (_, cell, _) = icon.artwork();
        let grain = ImageNode {
            flip_x: cell & 1 != 0,
            flip_y: cell & 2 != 0,
            ..self.paper_grain()
        };
        self.commands
            .entity(tile)
            .insert((BorderColor::all(color(paper::INK, 0.28)), grain));
        let x = cell as f32 * PAPER_SYMBOL_CELL;
        let icon = self.node(
            tile,
            Node {
                width: px(size),
                height: px(size),
                flex_shrink: 0.,
                ..default()
            },
        );
        self.commands.entity(icon).insert(ImageNode {
            image: self.assets.tool_symbols.clone(),
            rect: Some(Rect::new(x, 0., x + PAPER_SYMBOL_CELL, PAPER_SYMBOL_CELL)),
            ..default()
        });
    }
}
