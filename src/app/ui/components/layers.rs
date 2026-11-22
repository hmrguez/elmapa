//! Shared row and control treatments for the Layers panel.
use super::{Action, Builder, Hover, ToolIcon, color, paper, pigment, row};
use crate::app::regions::RegionAction;
use bevy::prelude::*;
use elmapa::document::Brush;

pub(in crate::app::ui) const DISCLOSURE_WIDTH: f32 = 22.;

pub(in crate::app::ui) enum LayerRow {
    Layer,
    Region,
}

impl Builder<'_, '_, '_> {
    pub(in crate::app::ui) fn layer_row(
        &mut self,
        parent: Entity,
        mut node: Node,
        kind: LayerRow,
        selected: bool,
        tip: String,
    ) -> Entity {
        let (radius, wash, outline, hover) = match kind {
            LayerRow::Layer => (10., 0.95, 0.32, 0.65),
            LayerRow::Region => (8., 1., 0.58, 0.74),
        };
        node.border = UiRect::all(px(1.));
        node.border_radius = BorderRadius::all(px(radius));
        let item = self.node(parent, node);
        let p = self.p;
        let base = color(p.wash, if selected { wash } else { 0. });
        self.commands.entity(item).insert((
            BackgroundColor(base),
            BorderColor::all(color(p.accent, if selected { outline } else { 0. })),
            Hover {
                base,
                target: color(p.wash, hover),
                tip,
                ..default()
            },
        ));
        if self.paper {
            self.style_paper_button(item, selected, false, None);
        }
        item
    }

    pub(in crate::app::ui) fn layer_visibility(
        &mut self,
        parent: Entity,
        visible: bool,
        action: Action,
    ) {
        let symbol = if visible { "eye" } else { "eye-off" };
        let button = self.layer_control(
            parent,
            Vec2::splat(30.),
            "Toggle visibility".into(),
            action,
            (!visible).then_some(pigment::ROSE),
        );
        self.icon(button, symbol, self.p.muted, 17.);
    }

    pub(in crate::app::ui) fn layer_disclosure(
        &mut self,
        parent: Entity,
        expanded: bool,
        label: &str,
        action: Action,
    ) {
        let (verb, symbol) = if expanded {
            ("Collapse", "chevron-down")
        } else {
            ("Expand", "chevron-right")
        };
        let button = self.layer_control(
            parent,
            Vec2::new(DISCLOSURE_WIDTH, 28.),
            format!("{verb} {label}"),
            action,
            Some(pigment::LINEN),
        );
        self.icon(button, symbol, self.p.accent, 13.);
    }

    fn layer_control(
        &mut self,
        parent: Entity,
        size: Vec2,
        tip: String,
        action: Action,
        paper_pigment: Option<u32>,
    ) -> Entity {
        let (radius, hover) = if matches!(action, Action::Visibility(_)) {
            (8., 0.5)
        } else {
            (6., 0.55)
        };
        let button = self.node(
            parent,
            Node {
                width: px(size.x),
                height: px(size.y),
                flex_shrink: 0.,
                border_radius: BorderRadius::all(px(radius)),
                justify_content: JustifyContent::Center,
                ..row(0.)
            },
        );
        self.commands.entity(button).insert((
            BackgroundColor(Color::NONE),
            Hover {
                base: Color::NONE,
                target: color(self.p.line, hover),
                tip,
                ..default()
            },
        ));
        if self.paper {
            self.style_paper_button(button, false, false, paper_pigment);
        }
        self.hit(button, Some(action));
        button
    }

    pub(in crate::app::ui) fn layer_panel_button(
        &mut self,
        parent: Entity,
        symbol: &str,
        action: Action,
        active: bool,
    ) {
        let pigment = match action {
            Action::AddLayer => pigment::LINEN,
            Action::Regions(RegionAction::NewLayer) => ToolIcon::Regions.pigment(),
            Action::LayerTools => pigment::OCHRE,
            Action::MoveLayer(_) => pigment::BLUE,
            Action::Opacity(_) => pigment::SEA_GLASS,
            Action::DeleteLayer => pigment::ROSE,
            _ => pigment::LINEN,
        };
        let button = self.button_with_pigment(parent, symbol, "", action, active, pigment);
        if self.paper {
            self.commands
                .entity(button)
                .entry::<Node>()
                .and_modify(|mut node| {
                    node.width = px(36.);
                    node.padding = UiRect::ZERO;
                });
        }
    }

    pub(in crate::app::ui) fn layer_tool_icon(
        &mut self,
        parent: Entity,
        tool: ToolIcon,
        size: f32,
    ) {
        if self.paper {
            let tile = self.node(
                parent,
                Node {
                    width: px(size),
                    height: px(size),
                    border: UiRect::all(px(1.)),
                    border_radius: paper::corners(),
                    justify_content: JustifyContent::Center,
                    flex_shrink: 0.,
                    ..row(0.)
                },
            );
            self.commands
                .entity(tile)
                .insert(BackgroundColor(color(tool.pigment(), 1.)));
            self.paper_tool_art(tile, tool, size - 4.);
            return;
        }
        let hex = match tool {
            ToolIcon::Brush(brush) => holo_brush_color(brush),
            ToolIcon::Landmarks => {
                self.icon(parent, tool.symbol(), self.p.accent, 20.);
                return;
            }
            _ => self.p.accent,
        };
        let tile = self.layer_badge(parent, 20., 7.);
        self.commands
            .entity(tile)
            .insert(BackgroundColor(color(hex, 0.13)));
        self.icon(tile, tool.symbol(), hex, 17.);
    }

    pub(in crate::app::ui) fn layer_type_icon(
        &mut self,
        parent: Entity,
        symbol: &str,
        regions: bool,
    ) {
        if self.paper {
            let tile = self.layer_badge(parent, 20., 5.);
            let background = if regions {
                ToolIcon::Regions.pigment()
            } else {
                pigment::LINEN
            };
            self.commands
                .entity(tile)
                .insert(BackgroundColor(color(background, 1.)));
            self.icon(tile, symbol, self.p.accent, 15.);
        } else if regions {
            self.icon(parent, symbol, self.p.accent, 19.);
        } else {
            let tile = self.layer_badge(parent, 24., 7.);
            self.commands
                .entity(tile)
                .entry::<Node>()
                .and_modify(|mut node| node.border = UiRect::all(px(1.)));
            self.commands
                .entity(tile)
                .insert(BorderColor::all(color(self.p.line, 1.)));
            self.icon(tile, symbol, self.p.muted, 17.);
        }
    }

    fn layer_badge(&mut self, parent: Entity, size: f32, radius: f32) -> Entity {
        self.node(
            parent,
            Node {
                width: px(size),
                height: px(size),
                border_radius: BorderRadius::all(px(radius)),
                justify_content: JustifyContent::Center,
                ..row(0.)
            },
        )
    }
}

fn holo_brush_color(brush: Brush) -> u32 {
    match brush {
        Brush::Land => 0x856d46,
        Brush::Water => 0x4e746f,
        Brush::Forest => 0x59694b,
        Brush::Mountain => 0x78644e,
        Brush::River => 0x48766e,
        Brush::Path => 0x89603c,
        Brush::Erase => 0x8c7478,
    }
}
