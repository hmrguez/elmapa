//! Text tool interaction. Field drafts and move previews commit as single edits.
use super::state::{Action, Studio, Tool};
mod edit;
mod keyboard;
#[cfg(test)]
mod tests;
mod transform;

pub use edit::Edit;
use elmapa::document::{
    Label, LabelFont, LabelInk, Point,
    limits::{
        MAX_LABEL_CURVATURE, MAX_LABEL_SIZE, MAX_LABEL_SPACING, MIN_LABEL_SIZE, MIN_LABEL_SPACING,
    },
};
pub use transform::{Handle, TransformDrag, selection_size};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Field {
    Name,
    Size,
    Spacing,
    Opacity,
    Curvature,
}
impl Field {
    const ADVANCED: [Self; 3] = [Self::Spacing, Self::Opacity, Self::Curvature];

    pub fn advanced(self) -> bool {
        self != Self::Name
    }
}
#[derive(Clone, Copy)]
pub struct PlacementDraft {
    pub point: Point,
    pub layer: u64,
}

#[derive(Clone, Copy)]
pub struct LabelStyle {
    pub font: LabelFont,
    pub ink: LabelInk,
    pub size: f32,
    pub spacing: f32,
    pub opacity: f32,
    pub curvature: f32,
}

impl From<&Label> for LabelStyle {
    fn from(label: &Label) -> Self {
        Self {
            font: label.font,
            ink: label.ink,
            size: label.size,
            spacing: label.spacing,
            opacity: label.opacity,
            curvature: label.curvature,
        }
    }
}

impl Default for LabelStyle {
    fn default() -> Self {
        Self::from(&Label::default())
    }
}

impl LabelStyle {
    fn label(self, point: Point, text: String) -> Label {
        Label {
            text,
            position: point,
            font: self.font,
            ink: self.ink,
            size: self.size,
            spacing: self.spacing,
            opacity: self.opacity,
            curvature: self.curvature,
            ..Label::default()
        }
    }
}

#[derive(Default)]
pub struct TextTool {
    pub selected: Option<u64>,
    pub style: LabelStyle,
    pub draft: Option<PlacementDraft>,
    pub style_open: bool,
    pub style_details: bool,
    pub edit: Option<Edit>,
    pub control: Option<Field>,
    pub preview: Option<Label>,
    pub moving: Option<(Point, Label)>,
    pub radial: super::radial::Navigation,
    pub transforming: Option<TransformDrag>,
    pub snap: bool,
    pub error: Option<String>,
    pub composition: String,
    pub animate: Option<u64>,
    pub drop: Option<(u64, f32)>,
}
impl Studio {
    pub fn selected_label(&self) -> Option<&Label> {
        self.text
            .selected
            .filter(|_| self.tool == Tool::Select && !self.home)
            .and_then(|id| self.editor.label(id))
    }

    pub(super) fn dismiss_text_control(&mut self) {
        if self.text.control.take().is_some() {
            if self.text.draft.is_none() {
                self.commit_text_field();
                self.text.error = None;
            }
            self.ui_dirty = true;
        }
    }

    pub fn current_label(&self) -> Label {
        self.text
            .preview
            .clone()
            .or_else(|| {
                self.text
                    .selected
                    .and_then(|id| self.editor.label(id).cloned())
            })
            .unwrap_or_else(|| self.text.style.label(Point::new(0., 0.), String::new()))
    }
    pub fn focus_text(&mut self, kind: Field) {
        self.text.control = kind.advanced().then_some(kind);
        let label = self.current_label();
        let value = match kind {
            Field::Name => label.text,
            Field::Size => label.size.to_string(),
            Field::Spacing => format!("{}", label.spacing),
            Field::Opacity => (label.opacity * 100.).to_string(),
            Field::Curvature => label.curvature.to_string(),
        };
        self.text.error = None;
        self.text.composition.clear();
        self.text.edit = Some(Edit {
            kind,
            cursor: value.len(),
            anchor: 0,
            value,
        });
        self.ui_dirty = true;
    }
    pub fn commit_text_field(&mut self) {
        let Some(edit) = self.text.edit.take() else {
            return;
        };
        let mut label = self.current_label();
        if self.text.preview.take().is_some() {
            self.invalidate_preview();
        }
        self.text.composition.clear();
        match edit.kind {
            Field::Name => label.text = edit.value,
            _ => {
                let Ok(value) = edit.value.parse::<f32>() else {
                    self.text.error = Some("Enter a number for this setting.".into());
                    self.ui_dirty = true;
                    return;
                };
                match edit.kind {
                    Field::Size => label.size = value,
                    Field::Spacing => label.spacing = value,
                    Field::Opacity if value != label.opacity * 100. => label.opacity = value / 100.,
                    Field::Curvature => label.curvature = value,
                    _ => {}
                }
            }
        }
        if label.validate().is_err() {
            self.text.error = Some(
                match edit.kind {
                    Field::Name => "Use a name of 1–512 bytes on one line.",
                    Field::Size => "Use a size between 8 and 256.",
                    Field::Spacing => "Use spacing between −2 and 24.",
                    Field::Opacity => "Use opacity between 0% and 100%.",
                    Field::Curvature => "Use curvature between −120° and 120°.",
                }
                .into(),
            );
            self.ui_dirty = true;
            return;
        }
        self.apply_label(label);
        self.ui_dirty = true;
    }
    fn apply_label(&mut self, label: Label) {
        self.text.error = None;
        if self.tool == Tool::Text || self.text.draft.is_some() {
            self.text.style = LabelStyle::from(&label);
            self.invalidate_preview();
        } else if self.text.selected.is_some() && self.editor.update_label(label) {
            self.changed();
        }
    }
    pub fn text_action(&mut self, action: Action) {
        match action {
            Action::TextField(field) if self.selected_label().is_some() => self.focus_text(field),
            Action::TextField(field) if self.text.draft.is_some() && field.advanced() => {
                self.text.control = Some(field);
            }
            Action::PlaceLabel => {
                self.finish_label_draft();
            }
            Action::TextStyle if self.tool == Tool::Text => self.text.style_open = true,
            Action::CloseTextStyle => self.text.style_open = false,
            Action::TextStyleDetails if self.text.style_open => {
                self.text.style_details = !self.text.style_details;
            }
            Action::TextReuseStyle => {
                if let Some(label) = self.selected_label() {
                    self.text.style = LabelStyle::from(label);
                }
            }
            Action::TextFont(font) => {
                let mut l = self.current_label();
                l.font = font;
                self.apply_label(l);
            }
            Action::TextInk(ink) => {
                let mut l = self.current_label();
                l.ink = ink;
                self.apply_label(l);
            }
            Action::TextStep(field, delta) => {
                let mut l = self.current_label();
                match field {
                    Field::Size => l.size = (l.size + delta).clamp(MIN_LABEL_SIZE, MAX_LABEL_SIZE),
                    Field::Spacing => {
                        l.spacing = (l.spacing + delta).clamp(MIN_LABEL_SPACING, MAX_LABEL_SPACING)
                    }
                    Field::Opacity => l.opacity = (l.opacity + delta).clamp(0., 1.),
                    Field::Curvature => {
                        l.curvature =
                            (l.curvature + delta).clamp(-MAX_LABEL_CURVATURE, MAX_LABEL_CURVATURE)
                    }
                    Field::Name => {}
                }
                self.apply_label(l);
            }
            Action::RadialEnter(depth, index) => {
                self.text.control = None;
                let items = self.text_actions();
                self.text.radial.enter_at(&items, depth, index);
            }
            Action::RadialBack => {
                self.text.control = None;
                self.text.radial.back();
            }
            Action::TextDuplicate if self.text.selected.is_some() => {
                let mut label = self.current_label();
                label.position.x += 24.;
                label.position.y += 24.;
                if let Some(id) = self.editor.add_label(label) {
                    self.text.selected = Some(id);
                    self.text.animate = Some(id);
                    self.changed();
                }
            }
            Action::TextDelete => {
                if let Some(id) = self.text.selected.take()
                    && self.editor.delete_label(id)
                {
                    self.changed();
                }
            }
            Action::TextReset => {
                let l = self.current_label();
                self.apply_label(Label {
                    id: l.id,
                    text: l.text,
                    position: l.position,
                    ..Label::default()
                });
            }
            _ => {}
        }
    }
    pub(super) fn select_label(&mut self, point: Point, layer: u64, id: u64) {
        self.editor.select_layer(layer);
        if self.text.selected != Some(id) {
            self.text.radial.reset();
            self.text.control = None;
        }
        self.text.selected = Some(id);
        let label = self.editor.label(id).unwrap().clone();
        self.text.moving = Some((point, label));
    }

    pub(super) fn place_text(&mut self, point: Point) {
        if !self.finish_label_draft() {
            return;
        }
        self.text.draft = Some(PlacementDraft {
            point,
            layer: self.editor.active_layer(),
        });
        self.text.edit = Some(Edit {
            kind: Field::Name,
            value: String::new(),
            cursor: 0,
            anchor: 0,
        });
        self.invalidate_preview();
        self.ui_dirty = true;
    }

    pub(super) fn draft_label(&self) -> Option<Label> {
        let draft = self.text.draft?;
        let edit = self.text.edit.as_ref()?;
        Some(self.text.style.label(draft.point, edit.value.clone()))
    }

    pub(super) fn draft_preview_label(&self) -> Option<Label> {
        let mut label = self.draft_label()?;
        if !self.text.composition.is_empty()
            && let Some(edit) = &self.text.edit
        {
            label
                .text
                .replace_range(edit.range(), &self.text.composition);
        }
        if label.text.trim().is_empty() {
            label.text = Label::default().text;
            label.opacity *= 0.48;
        }
        Some(label)
    }

    /// A failed commit keeps the current text and position available for correction.
    pub(super) fn finish_label_draft(&mut self) -> bool {
        let Some(label) = self.draft_label() else {
            return true;
        };
        if !self.text.composition.is_empty() {
            return false;
        }
        if self
            .text
            .edit
            .as_ref()
            .is_some_and(|edit| edit.value.is_empty())
        {
            self.cancel_label_draft();
            return true;
        }
        if label.validate().is_err() {
            self.text.error = Some("Enter a name on one line.".into());
        } else if let Some(draft) = self.text.draft
            && let Some(id) = self.editor.add_label_on_layer(label, draft.layer)
        {
            self.cancel_label_draft();
            self.text.animate = Some(id);
            self.changed();
            return true;
        } else {
            self.text.error = Some("Show the destination layer or free room for a label.".into());
        }
        self.ui_dirty = true;
        false
    }

    pub(super) fn cancel_label_draft(&mut self) {
        self.text.draft = None;
        self.text.radial.reset();
        self.text.control = None;
        self.text.edit = None;
        self.text.composition.clear();
        self.text.error = None;
        self.text.preview = None;
        self.invalidate_preview();
        self.ui_dirty = true;
    }

    pub fn cancel_text(&mut self) {
        self.cancel_label_draft();
        self.text.style_open = false;
        self.text.style_details = false;
        self.text.moving = None;
        self.text.transforming = None;
        self.text.radial.reset();
        self.text.control = None;
        self.text.selected = None;
    }
    pub fn text_actions(&self) -> Vec<super::radial::Item<Action>> {
        use super::radial::Item;
        use elmapa::document::{LabelFont, LabelInk};
        let label = self.current_label();
        let fonts = LabelFont::ALL
            .into_iter()
            .map(|font| {
                let mut item = Item::action(font.family(), "text", Action::TextFont(font));
                item.active = label.font == font;
                item
            })
            .collect();
        let inks = LabelInk::ALL
            .into_iter()
            .map(|ink| {
                let mut item = Item::action(ink.name(), "", Action::TextInk(ink));
                item.active = label.ink == ink;
                item.swatch = Some(ink.color());
                item
            })
            .collect();
        let mut actions = vec![
            Item::branch("Typeface", "typeface", fonts),
            Item::branch("Ink", "ink", inks),
        ];
        let mut advanced = vec![
            Item::action(
                "Letter spacing",
                "letter-spacing",
                Action::TextField(Field::Spacing),
            ),
            Item::action(
                "Ink opacity",
                "text-opacity",
                Action::TextField(Field::Opacity),
            ),
            Item::action(
                "Curvature",
                "text-curvature",
                Action::TextField(Field::Curvature),
            ),
        ];
        if self.text.draft.is_some() {
            advanced.insert(
                0,
                Item::action("Size", "text", Action::TextField(Field::Size)),
            );
        } else {
            actions.push(Item::action(
                "Duplicate",
                "text-duplicate",
                Action::TextDuplicate,
            ));
            actions.push(Item::action("Delete", "delete", Action::TextDelete));
            advanced.push(Item::action(
                "Use style for new labels",
                "text-duplicate",
                Action::TextReuseStyle,
            ));
        }
        advanced.push(Item::action("Reset style", "text-reset", Action::TextReset));
        actions.push(Item::branch("Advanced typography", "typography", advanced));
        if self.text.draft.is_some() {
            actions.push(Item::action(
                "Place label · Return",
                "check",
                Action::PlaceLabel,
            ));
        }
        actions
    }
}
