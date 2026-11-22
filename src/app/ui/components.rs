//! Shared UI components, layout helpers and appearance palettes.
mod toolbar;

use super::{
    Hit,
    assets::UiAssets,
    editor::MenuArrow,
    feedback::{Hover, HoverFrame},
};
use crate::app::state::{Action, Appearance};
use bevy::prelude::*;
use elmapa::{
    brush_materials,
    document::{AtlasStyle, Brush},
};
pub(super) use toolbar::{PAPER_TOOL_SIZE, ToolIcon};

pub(super) const BUTTON_HEIGHT: f32 = 34.;

pub fn color(hex: u32, alpha: f32) -> Color {
    Color::linear_rgba(
        ((hex >> 16) & 255) as f32 / 255.,
        ((hex >> 8) & 255) as f32 / 255.,
        (hex & 255) as f32 / 255.,
        alpha,
    )
}

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub(super) surface: u32,
    pub(super) text: u32,
    pub(super) muted: u32,
    pub(super) line: u32,
    pub(super) accent: u32,
    pub(super) wash: u32,
}
impl Palette {
    fn holo() -> Self {
        Self {
            surface: 0xf8f6ed,
            text: 0x35382d,
            muted: 0x676e5e,
            line: 0xdbd8c7,
            accent: 0x4f6241,
            wash: 0xe1e6d3,
        }
    }

    pub(super) fn shelf() -> Self {
        Self {
            surface: 0xffffff,
            text: 0x304742,
            muted: 0x536d64,
            line: 0xffffff,
            accent: 0x547971,
            wash: 0xafc7bb,
        }
    }
}

pub(super) fn row(gap: f32) -> Node {
    Node {
        align_items: AlignItems::Center,
        column_gap: px(gap),
        ..default()
    }
}
pub(super) fn column(gap: f32) -> Node {
    Node {
        flex_direction: FlexDirection::Column,
        row_gap: px(gap),
        ..default()
    }
}
pub(super) fn absolute(x: Val, y: Val) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: x,
        top: y,
        ..default()
    }
}
pub(super) fn shadow(large: bool) -> BoxShadow {
    let make = |y, blur, spread| ShadowStyle {
        color: color(0x000000, 0.1),
        x_offset: px(0.),
        y_offset: px(y),
        spread_radius: px(spread),
        blur_radius: px(blur),
    };
    BoxShadow(if large {
        vec![make(10., 15., -3.), make(4., 6., -4.)]
    } else {
        vec![make(4., 6., -1.), make(2., 4., -2.)]
    })
}

pub(super) struct Builder<'a, 'w, 's> {
    pub(super) commands: &'a mut Commands<'w, 's>,
    pub(super) assets: &'a UiAssets,
    pub(super) p: Palette,
    appearance: Appearance,
    order: u32,
    pub(super) atlas_style: AtlasStyle,
}
impl<'a, 'w, 's> Builder<'a, 'w, 's> {
    pub(super) fn new(
        commands: &'a mut Commands<'w, 's>,
        assets: &'a UiAssets,
        appearance: Appearance,
    ) -> Self {
        Self {
            commands,
            assets,
            p: Palette::holo(),
            appearance,
            order: 0,
            atlas_style: AtlasStyle::Classic,
        }
    }

    pub(super) fn node(&mut self, parent: Entity, node: Node) -> Entity {
        let id = self.commands.spawn(node).id();
        self.commands.entity(parent).add_child(id);
        id
    }
    pub(super) fn hit(&mut self, entity: Entity, action: Option<Action>) {
        self.order += 1;
        self.commands.entity(entity).insert(Hit {
            action,
            order: self.order,
        });
    }
    pub(super) fn text(
        &mut self,
        parent: Entity,
        text: impl Into<String>,
        size: f32,
        hex: u32,
        serif: bool,
    ) -> Entity {
        let id = self
            .commands
            .spawn((
                Text::new(text),
                TextFont::from_font_size(size).with_font(if serif {
                    self.assets.baskerville.clone()
                } else {
                    self.assets.avenir.clone()
                }),
                TextColor(color(hex, 1.)),
                bevy::text::LineHeight::RelativeToFont(1.618034),
                FontHinting::Enabled,
                TextLayout::no_wrap(),
                Node {
                    flex_shrink: 0.,
                    ..default()
                },
            ))
            .id();
        self.commands.entity(parent).add_child(id);
        id
    }
    pub(super) fn icon(&mut self, parent: Entity, symbol: &str, hex: u32, size: f32) -> Entity {
        let catalog = elmapa::scene::landmark_catalog();
        if let Some(pack) = catalog.packs.iter().find(|p| p.icon_key == symbol)
            && pack.icon.is_none()
        {
            return self.text(parent, &pack.initial, size + 3., hex, true);
        }
        if catalog.items.iter().any(|d| d.kind.key() == symbol)
            || catalog.packs.iter().any(|p| p.icon_key == symbol)
        {
            let image = self.assets.icons.get(symbol).cloned().unwrap_or_default();
            return self
                .commands
                .spawn((
                    ImageNode {
                        image,
                        color: if symbol.starts_with("landmark-pack/") {
                            color(hex, 1.)
                        } else {
                            Color::WHITE
                        },
                        ..default()
                    },
                    Node {
                        width: px(size),
                        height: px(size * 0.8),
                        ..default()
                    },
                    ChildOf(parent),
                ))
                .id();
        }
        if symbol == "text" {
            return self.text(parent, "T", size + 3., hex, true);
        }
        let image = self.assets.icons.get(symbol).cloned().unwrap_or_default();
        let id = self
            .commands
            .spawn((
                ImageNode {
                    image,
                    color: if symbol == crate::app::branding::SYMBOL {
                        Color::WHITE
                    } else {
                        color(hex, 1.)
                    },
                    ..default()
                },
                Node {
                    width: px(size),
                    height: px(size),
                    flex_shrink: 0.,
                    ..default()
                },
            ))
            .id();
        self.commands.entity(parent).add_child(id);
        id
    }
    pub(super) fn material_swatch(&mut self, parent: Entity, index: usize, size: f32) -> Entity {
        let index = brush_materials::artwork_index(index, self.atlas_style);
        let material = &brush_materials::artwork_catalog()[index];
        let image = self.assets.brush_swatches.get(&index).cloned();
        let sheet = material.sprites;
        if material.brush != Brush::Forest && sheet.is_none() {
            return self.swatch(parent, material.color, size, image);
        }
        let width = size + 4.;
        let height = sheet.map_or(width, |sheet| {
            width * sheet.frame_height as f32 / sheet.frame_width as f32
        });
        let preview = self.node(
            parent,
            Node {
                width: px(width),
                height: px(height),
                flex_shrink: 0.,
                ..default()
            },
        );
        if let Some(image) = image {
            let mut node = ImageNode::new(image);
            node.rect = sheet.map(|sheet| {
                Rect::from_corners(
                    Vec2::ZERO,
                    Vec2::new(sheet.frame_width as f32, sheet.frame_height as f32),
                )
            });
            self.commands.entity(preview).insert(node);
        }
        preview
    }
    pub(super) fn swatch(
        &mut self,
        parent: Entity,
        pigment: u32,
        size: f32,
        texture: Option<Handle<Image>>,
    ) -> Entity {
        let swatch = self.node(
            parent,
            Node {
                width: px(size),
                height: px(size),
                flex_shrink: 0.,
                border_radius: BorderRadius::MAX,
                border: UiRect::all(px(1.)),
                ..default()
            },
        );
        self.commands.entity(swatch).insert((
            BackgroundColor(color(pigment, 1.)),
            BorderColor::all(color(self.p.muted, 0.45)),
        ));
        if let Some(texture) = texture {
            self.commands.entity(swatch).insert(ImageNode::new(texture));
        }
        swatch
    }
    pub(super) fn panel(
        &mut self,
        parent: Entity,
        mut node: Node,
        radius: f32,
        padding: f32,
        alpha: f32,
        large: bool,
    ) -> Entity {
        node.border = UiRect::all(px(1.));
        node.border_radius = BorderRadius::all(px(radius));
        node.padding = UiRect::all(px(padding));
        let id = self.node(parent, node);
        self.commands.entity(id).insert((
            BackgroundColor(color(self.p.surface, alpha)),
            BorderColor::all(color(self.p.line, 0.85)),
            shadow(large),
        ));
        self.hit(id, None);
        id
    }
    pub(super) fn button(
        &mut self,
        parent: Entity,
        symbol: &str,
        label: &str,
        action: Action,
        active: bool,
    ) -> Entity {
        let p = self.p;
        let id = self.action_button(
            parent,
            Node {
                height: px(BUTTON_HEIGHT),
                padding: UiRect::horizontal(px(11.)),
                border_radius: BorderRadius::all(px(10.)),
                flex_shrink: 0.,
                justify_content: JustifyContent::Center,
                ..row(8.)
            },
            symbol,
            label,
            action,
            active,
        );
        let icon = if symbol.is_empty() {
            id
        } else {
            self.icon(id, symbol, if active { p.accent } else { p.muted }, 17.)
        };
        if matches!(action, Action::Menu) {
            self.commands.entity(icon).insert(MenuArrow);
        }
        if !label.is_empty() {
            self.text(
                id,
                label,
                12.,
                if active { p.accent } else { p.text },
                false,
            );
        }
        id
    }

    pub(super) fn shelf_button(
        &mut self,
        parent: Entity,
        symbol: &str,
        label: &str,
        action: Action,
    ) -> Entity {
        let p = Palette::shelf();
        let base = color(p.surface, 0.55);
        let frame = color(p.line, 0.5);
        let button = self.action_button(
            parent,
            Node {
                height: px(42.),
                width: if label.is_empty() { px(42.) } else { Val::Auto },
                padding: UiRect::horizontal(px(if label.is_empty() { 0. } else { 17. })),
                border: UiRect::all(px(1.)),
                border_radius: BorderRadius::MAX,
                justify_content: JustifyContent::Center,
                ..row(9.)
            },
            symbol,
            label,
            action,
            false,
        );
        self.commands.entity(button).insert((
            BackgroundColor(base),
            BorderColor::all(frame),
            HoverFrame {
                from: frame,
                to: color(p.accent, 0.75),
                card: false,
            },
        ));
        self.commands
            .entity(button)
            .entry::<Hover>()
            .and_modify(move |mut hover| {
                hover.base = base;
                hover.target = color(p.wash, 0.35);
            });
        self.icon(button, symbol, p.muted, 17.);
        if !label.is_empty() {
            self.text(button, label, 12., p.muted, false);
        }
        button
    }

    pub(super) fn appearance_picker(&mut self, parent: Entity) {
        let picker = self.node(parent, row(3.));
        for (appearance, label) in [(Appearance::Holo, "Holo"), (Appearance::Paper, "Paper")] {
            let active = self.appearance == appearance;
            let choice = self.button(picker, "", label, Action::SetAppearance(appearance), active);
            let p = self.p;
            let mut entity = self.commands.entity(choice);
            entity.entry::<Node>().and_modify(|mut node| {
                node.min_width = px(57.);
                node.border = UiRect::all(px(1.));
            });
            if active {
                entity.insert(BorderColor::all(color(p.accent, 0.5)));
            } else {
                entity.entry::<Hover>().and_modify(move |mut hover| {
                    hover.target = color(p.wash, 0.5);
                });
            }
        }
    }

    pub(super) fn stepper(
        &mut self,
        parent: Entity,
        value: impl Into<String>,
        (decrease, increase): (Action, Action),
        width: Val,
        font_size: f32,
    ) -> Entity {
        self.button(parent, "minus", "", decrease, false);
        let display = self.node(
            parent,
            Node {
                width,
                justify_content: JustifyContent::Center,
                ..row(0.)
            },
        );
        let label = self.text(display, value, font_size, self.p.text, false);
        self.button(parent, "plus", "", increase, false);
        label
    }

    pub(super) fn action_button(
        &mut self,
        parent: Entity,
        node: Node,
        symbol: &str,
        label: &str,
        action: Action,
        active: bool,
    ) -> Entity {
        let p = self.p;
        let id = self.node(parent, node);
        let base = color(
            if active { p.wash } else { p.surface },
            if active { 1. } else { 0.1 },
        );
        let tip = match action {
            Action::Brush(b) => format!("{} · {}", b.label(), b.shortcut()),
            Action::Menu => "Map actions".into(),
            Action::Save => crate::app::input::shortcut_hint("Save map", "S", false),
            Action::Open => crate::app::input::shortcut_hint("Open map", "O", false),
            Action::Export => crate::app::input::shortcut_hint("Export SVG", "E", false),
            Action::ReplayView => crate::app::input::shortcut_hint("Replay this view", "R", true),
            Action::Undo => crate::app::input::shortcut_hint("Undo", "Z", false),
            Action::Redo => crate::app::input::shortcut_hint("Redo", "Z", true),
            Action::LayerTools => "Selected layer settings".into(),
            Action::AddLayer => "New layer".into(),
            Action::Regions(crate::app::regions::RegionAction::Tool) => "Regions".into(),
            Action::Regions(crate::app::regions::RegionAction::NewLayer) => {
                "New Regions layer".into()
            }
            Action::Regions(crate::app::regions::RegionAction::ParentLayer) => {
                "Back to parent Regions layer".into()
            }
            Action::Regions(crate::app::regions::RegionAction::SplitCount(delta)) => {
                if delta < 0 {
                    "Fewer regions".into()
                } else {
                    "More regions".into()
                }
            }
            Action::Fit => crate::app::input::shortcut_hint("Fit artwork", "0", false),
            Action::Advanced => "Advanced brush settings".into(),
            Action::LandmarkTool => "Landmarks · L / 0".into(),
            Action::SelectTool => "Select artwork · V / 9".into(),
            Action::ClearSelection => "Clear selection · Esc".into(),
            Action::PathNodeDensity(step) if step < 0 => "Fewer course nodes".into(),
            Action::PathNodeDensity(_) => "More course nodes".into(),
            Action::TextStyle => "New label style".into(),
            Action::PlaceLabel => "Place label · Return".into(),
            Action::TextTool => "Text · T / 8".into(),
            Action::TextDuplicate => "Duplicate label".into(),
            Action::TextDelete => "Delete label".into(),
            Action::LandmarkSize(v) if v < 0. => "Smaller landmark".into(),
            Action::LandmarkSize(_) => "Larger landmark".into(),
            Action::Radius(v) => {
                if v < 0. {
                    "Smaller brush · [".into()
                } else {
                    "Larger brush · ]".into()
                }
            }
            _ => {
                if label.is_empty() {
                    symbol.replace('-', " ")
                } else {
                    label.to_string()
                }
            }
        };
        self.commands.entity(id).insert((
            BackgroundColor(base),
            Hover {
                base,
                target: color(p.wash, 1.),
                progress: 0.,
                from: 0.,
                started: 0.,
                hovered: false,
                tip,
            },
        ));
        self.hit(id, Some(action));
        id
    }
}
