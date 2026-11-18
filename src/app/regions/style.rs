//! Contextual names, pigments, and map labels for region owners.
use super::{Mode, RegionAction};
use crate::app::{
    lettering::{Edit, Field},
    radial::{Item, Navigation},
    state::{Action, Studio},
};
use bevy::{input::keyboard::KeyboardInput, prelude::KeyCode};
use elmapa::{
    document::{Point, RegionLayer, limits::MAX_LABEL_TEXT_BYTES},
    scene::territories::{WASHES, pigment},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StyleAction {
    Rename,
    CustomColor,
    UseColor(u32),
    ToggleLabel,
    Enter(usize, usize),
    Back,
    Apply,
    Cancel,
}

#[derive(Clone, Copy, PartialEq)]
pub enum StyleField {
    Name,
    Color,
}

pub struct StyleEdit {
    pub field: StyleField,
    pub input: Edit,
}

#[derive(Default)]
pub struct StyleOrbit {
    pub anchor: Option<Point>,
    pub navigation: Navigation,
    pub edit: Option<StyleEdit>,
    pub composition: String,
    pub error: Option<String>,
}

impl StyleOrbit {
    fn finish_edit(&mut self) {
        self.edit = None;
        self.composition.clear();
        self.error = None;
    }

    pub(super) fn close_controls(&mut self) {
        self.finish_edit();
        self.navigation.reset();
    }

    fn edit(&mut self, field: StyleField, value: String) {
        self.finish_edit();
        self.edit = Some(StyleEdit {
            field,
            input: Edit {
                kind: Field::Name,
                cursor: value.len(),
                anchor: 0,
                value,
            },
        });
    }
}

impl Studio {
    pub fn region_style_target(&self) -> Option<(u64, &RegionLayer, usize)> {
        if !self.regions.active || self.regions.mode != Mode::Select {
            return None;
        }
        let owner = self.regions.selection.as_ref()?.owner?;
        let layer = self
            .regions_layer()
            .filter(|layer| layer.visible && layer.opacity > 0.)?;
        let data = layer.territories.as_deref()?;
        data.names.get(owner)?;
        Some((layer.id, data, owner))
    }

    pub fn region_style_actions(&self) -> Vec<Item<Action>> {
        let Some((_, data, owner)) = self.region_style_target() else {
            return Vec::new();
        };
        let current = pigment(data, owner);
        let shown = data.styles.get(&owner).and_then(|style| style.label) == Some(true);
        let shades = [
            "Terracotta",
            "Olive",
            "Heather",
            "Lagoon",
            "Ochre",
            "Umber",
            "Jade",
            "Rose",
            "Slate",
        ];
        let mut colors: Vec<_> = shades
            .into_iter()
            .zip(WASHES)
            .map(|(title, color)| {
                let mut item = Item::action(title, "", action(StyleAction::UseColor(color)));
                item.swatch = Some(color);
                item.active = color == current;
                item
            })
            .collect();
        colors.insert(
            0,
            Item::action(
                "Custom hex color…",
                "text-edit",
                action(StyleAction::CustomColor),
            ),
        );
        let mut palette = Item::branch("Region color", "", colors);
        palette.swatch = Some(current);
        vec![
            Item::action("Rename region", "text-edit", action(StyleAction::Rename)),
            palette,
            Item::action(
                if shown {
                    "Hide map label"
                } else {
                    "Show map label"
                },
                if shown { "eye-off" } else { "eye" },
                action(StyleAction::ToggleLabel),
            ),
        ]
    }

    pub(super) fn region_style_action(&mut self, action: StyleAction) {
        let Some((_, data, owner)) = self.region_style_target() else {
            return;
        };
        match action {
            StyleAction::Rename => {
                let name = data.names[owner].clone();
                self.regions.style.close_controls();
                self.regions.style.edit(StyleField::Name, name);
            }
            StyleAction::CustomColor => {
                let color = format!("#{:06X}", pigment(data, owner));
                self.regions.style.edit(StyleField::Color, color);
            }
            StyleAction::UseColor(color) => {
                if color != pigment(data, owner) {
                    self.update_region_style(|data, owner| {
                        data.styles.entry(owner).or_default().color = Some(color);
                    });
                }
                self.regions.style.close_controls();
            }
            StyleAction::ToggleLabel => {
                self.regions.style.close_controls();
                self.update_region_style(|data, owner| {
                    let style = data.styles.entry(owner).or_default();
                    style.label = Some(style.label != Some(true));
                });
            }
            StyleAction::Enter(depth, index) => {
                let tree = self.region_style_actions();
                self.regions.style.finish_edit();
                self.regions.style.navigation.enter_at(&tree, depth, index);
            }
            StyleAction::Back => {
                self.regions.style.finish_edit();
                self.regions.style.navigation.back();
            }
            StyleAction::Apply => self.finish_region_style(),
            StyleAction::Cancel => self.regions.style.finish_edit(),
        }
    }

    pub fn region_style_key(&mut self, event: &KeyboardInput, command: bool, shift: bool) -> bool {
        let key = event.key_code;
        match key {
            KeyCode::Enter | KeyCode::NumpadEnter if !command => {
                self.region_style_action(StyleAction::Apply);
            }
            KeyCode::Escape => self.region_style_action(StyleAction::Cancel),
            _ => {
                if command
                    && !matches!(
                        key,
                        KeyCode::KeyA
                            | KeyCode::KeyC
                            | KeyCode::KeyV
                            | KeyCode::KeyX
                            | KeyCode::ArrowLeft
                            | KeyCode::ArrowRight
                    )
                {
                    return false;
                }
                if let Some(edit) = &mut self.regions.style.edit {
                    edit.input.key(event, command, shift);
                    self.regions.style.error = None;
                }
            }
        }
        self.ui_dirty = true;
        true
    }

    fn finish_region_style(&mut self) {
        let Some(edit) = &self.regions.style.edit else {
            return;
        };
        let value = edit.input.value.trim();
        match edit.field {
            StyleField::Name => {
                if value.is_empty()
                    || value.len() > MAX_LABEL_TEXT_BYTES
                    || value.chars().any(char::is_control)
                {
                    self.regions.style.error = Some("Enter a single-line region name.".into());
                    return;
                }
                let name = value.to_owned();
                self.update_region_style(|data, owner| {
                    if data.names[owner] != name {
                        data.names[owner] = name;
                        data.styles
                            .entry(owner)
                            .or_default()
                            .label
                            .get_or_insert(true);
                    }
                });
                self.regions.style.finish_edit();
            }
            StyleField::Color => {
                let hex = value.strip_prefix('#').unwrap_or(value);
                if hex.len() != 6 || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
                    self.regions.style.error = Some("Enter six hex digits, like #46798A.".into());
                    return;
                }
                let color = u32::from_str_radix(hex, 16).expect("validated RGB digits");
                self.region_style_action(StyleAction::UseColor(color));
            }
        }
    }

    fn update_region_style(&mut self, update: impl FnOnce(&mut RegionLayer, usize)) {
        let Some((id, data, owner)) = self.region_style_target() else {
            return;
        };
        let mut data = data.clone();
        update(&mut data, owner);
        let revision = self.editor.revision();
        let shore = self.scene.snapshot().shoreline().clone();
        match self.editor.set_regions(Some(id), data, &shore) {
            Ok(()) if self.editor.revision() != revision => self.changed(),
            Ok(()) => {}
            Err(error) => self.regions.message = Some(error),
        }
    }
}

fn action(action: StyleAction) -> Action {
    Action::Regions(RegionAction::Style(action))
}
