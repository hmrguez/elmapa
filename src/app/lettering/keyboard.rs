//! Keyboard commands, text composition and live field previews.
use super::Field;
use crate::app::state::Studio;
use bevy::{input::keyboard::KeyboardInput, prelude::KeyCode};

impl Studio {
    /// Selection owns the keyboard even before the first character is typed.
    pub fn text_key(&mut self, event: &KeyboardInput, command: bool, shift: bool) -> bool {
        if self.regions.style.edit.is_some() {
            return self.region_style_key(event, command, shift);
        }
        let placing = self.text.draft.is_some() && !self.text.style_open;
        if !placing && self.selected_label().is_none() && self.name_landmark().is_none() {
            return false;
        }
        let key = event.key_code;
        let text_command = matches!(
            key,
            KeyCode::KeyA
                | KeyCode::KeyC
                | KeyCode::KeyX
                | KeyCode::KeyV
                | KeyCode::ArrowLeft
                | KeyCode::ArrowRight
        );
        if self.name_landmark().is_some() {
            if command && !text_command {
                return false;
            }
            if self.landmarks.moving.is_some() {
                return false;
            }
            if !self.text.landmark_name {
                let typing = !matches!(key, KeyCode::BracketLeft | KeyCode::BracketRight)
                    && event
                        .text
                        .as_ref()
                        .is_some_and(|value| value.chars().any(|c| !c.is_control()));
                let starts_name = if command { text_command } else { typing };
                if !starts_name {
                    return false;
                }
                self.enter_landmark_name();
            }
            if !self.text.composition.is_empty()
                && matches!(key, KeyCode::Enter | KeyCode::NumpadEnter)
            {
                return true;
            }
        }
        if placing {
            match key {
                KeyCode::Escape => {
                    if self.text.control.is_some() {
                        self.dismiss_text_control();
                    } else if !self.text.radial.path.is_empty() {
                        self.text.radial.back();
                        self.ui_dirty = true;
                    } else {
                        self.cancel_label_draft();
                    }
                    return true;
                }
                KeyCode::Enter | KeyCode::NumpadEnter if !command => {
                    self.finish_label_draft();
                    return true;
                }
                KeyCode::Tab => return true,
                KeyCode::KeyS
                | KeyCode::KeyE
                | KeyCode::KeyO
                | KeyCode::KeyQ
                | KeyCode::KeyZ
                | KeyCode::KeyH
                | KeyCode::Comma
                    if command =>
                {
                    return false;
                }
                _ => {}
            }
        }
        let dragging = self.text.moving.is_some() || self.text.transforming.is_some();
        if key == KeyCode::Escape {
            if self.text.control.is_some() {
                self.text.edit = None;
                self.text.composition.clear();
                self.dismiss_text_control();
            } else {
                if !dragging && !self.text.landmark_name {
                    self.commit_text_field();
                }
                self.cancel_text();
            }
            return true;
        }
        if dragging {
            return true;
        }
        if matches!(key, KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Tab) {
            if !command {
                let control = self.text.control;
                self.commit_text_field();
                if key == KeyCode::Tab
                    && let Some(field) = control
                {
                    let fields = Field::ADVANCED;
                    let index = fields.iter().position(|f| *f == field).unwrap_or(0);
                    let step = if shift { fields.len() - 1 } else { 1 };
                    self.focus_text(fields[(index + step) % fields.len()]);
                } else if self.text.error.is_none() {
                    self.dismiss_text_control();
                }
            }
            return true;
        }
        if self.text.edit.is_none() {
            // Begin only on text input or a text-editing key. A modifier key
            // alone must leave the selection and its controls as they are.
            let starts_edit = if command {
                text_command
            } else {
                matches!(
                    key,
                    KeyCode::Backspace
                        | KeyCode::Delete
                        | KeyCode::ArrowLeft
                        | KeyCode::ArrowRight
                        | KeyCode::Home
                        | KeyCode::End
                ) || event
                    .text
                    .as_ref()
                    .is_some_and(|text| text.chars().any(|c| !c.is_control()))
            };
            if !starts_edit {
                return true;
            }
            self.focus_text(self.text.control.unwrap_or(Field::Name));
        }
        self.text.edit.as_mut().unwrap().key(event, command, shift);
        self.preview_text_field();
        self.ui_dirty = true;
        true
    }
    pub fn preview_text_field(&mut self) {
        let Some(edit) = &self.text.edit else {
            return;
        };
        if self.text.draft.is_some() {
            self.text.error = None;
            self.invalidate_preview();
            return;
        }
        if edit.kind != Field::Name || !self.has_text_selection() {
            return;
        }
        let mut label = self.current_label();
        label.text = edit.value.clone();
        if !self.text.composition.is_empty() {
            label
                .text
                .replace_range(edit.range(), &self.text.composition);
        }
        self.text.preview = Some(label);
        self.invalidate_preview();
    }
    #[cfg(target_os = "macos")]
    pub fn text_shortcut(&mut self, key_code: KeyCode) -> bool {
        use bevy::input::{
            ButtonState,
            keyboard::{Key, NativeKey},
        };
        self.text_key(
            &KeyboardInput {
                key_code,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window: bevy::prelude::Entity::PLACEHOLDER,
            },
            true,
            false,
        )
    }
    fn accept_text_ime(&mut self, value: &str) -> bool {
        if self.text.moving.is_some()
            || self.text.transforming.is_some()
            || self.landmarks.moving.is_some()
        {
            return false;
        }
        if self.name_landmark().is_some() && !value.is_empty() {
            self.enter_landmark_name();
        }
        self.has_text_selection() || self.text.draft.is_some()
    }

    pub fn text_preedit(&mut self, value: &str) {
        if self.regions.style.edit.is_some() {
            self.regions.style.composition = value.into();
            self.ui_dirty = true;
            return;
        }
        if !self.accept_text_ime(value) {
            return;
        }
        if self.text.edit.is_none() && !value.is_empty() {
            self.focus_text(self.text.control.unwrap_or(Field::Name));
        }
        self.text.composition = value.into();
        self.preview_text_field();
        self.ui_dirty = true;
    }

    pub fn text_insert(&mut self, value: &str) {
        if let Some(edit) = &mut self.regions.style.edit {
            let value: String = value.chars().filter(|c| !c.is_control()).collect();
            edit.input.insert(&value);
            self.regions.style.composition.clear();
            self.ui_dirty = true;
            return;
        }
        if !self.accept_text_ime(value) {
            return;
        }
        let value: String = value.chars().filter(|c| !c.is_control()).collect();
        if self.text.edit.is_none() && !value.is_empty() {
            self.focus_text(self.text.control.unwrap_or(Field::Name));
        }
        if let Some(edit) = &mut self.text.edit {
            if !value.is_empty() {
                edit.insert(&value);
            }
            self.text.composition.clear();
            self.preview_text_field();
            self.ui_dirty = true;
        }
    }
}
