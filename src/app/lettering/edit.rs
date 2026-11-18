//! UTF-8 field editing and clipboard operations shared with region styles.
use super::Field;
use bevy::{input::keyboard::KeyboardInput, prelude::KeyCode};
use elmapa::document::limits::MAX_LABEL_TEXT_BYTES;

#[derive(Clone)]
pub struct Edit {
    pub kind: Field,
    pub value: String,
    pub cursor: usize,
    pub anchor: usize,
}
impl Edit {
    pub fn range(&self) -> std::ops::Range<usize> {
        self.cursor.min(self.anchor)..self.cursor.max(self.anchor)
    }
    pub(in crate::app) fn insert(&mut self, value: &str) {
        let range = self.range();
        if self.value.len() - range.len() + value.len() > MAX_LABEL_TEXT_BYTES {
            return;
        }
        self.value.replace_range(range.clone(), value);
        self.cursor = range.start + value.len();
        self.anchor = self.cursor;
    }

    pub(in crate::app) fn key(&mut self, event: &KeyboardInput, command: bool, shift: bool) {
        let key = event.key_code;
        if command {
            match key {
                KeyCode::ArrowLeft | KeyCode::ArrowRight => {
                    self.cursor = if key == KeyCode::ArrowLeft {
                        0
                    } else {
                        self.value.len()
                    };
                    if !shift {
                        self.anchor = self.cursor;
                    }
                }
                KeyCode::KeyA => {
                    self.anchor = 0;
                    self.cursor = self.value.len();
                }
                KeyCode::KeyC | KeyCode::KeyX => {
                    clipboard_write(&self.value[self.range()]);
                    if key == KeyCode::KeyX {
                        self.insert("");
                    }
                }
                KeyCode::KeyV => {
                    if let Some(value) = clipboard_read() {
                        self.insert(
                            &value
                                .chars()
                                .filter(|c| !c.is_control())
                                .collect::<String>(),
                        );
                    }
                }
                _ => {}
            }
        } else {
            match key {
                KeyCode::Backspace | KeyCode::Delete => {
                    if self.range().is_empty() {
                        if key == KeyCode::Backspace {
                            self.anchor = self.value[..self.cursor]
                                .char_indices()
                                .last()
                                .map_or(0, |(i, _)| i);
                        } else {
                            self.anchor = self.value[self.cursor..]
                                .chars()
                                .next()
                                .map_or(self.cursor, |c| self.cursor + c.len_utf8());
                        }
                    }
                    self.insert("");
                }
                KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Home | KeyCode::End => {
                    self.cursor = match key {
                        KeyCode::Home => 0,
                        KeyCode::End => self.value.len(),
                        KeyCode::ArrowLeft if !shift && !self.range().is_empty() => {
                            self.range().start
                        }
                        KeyCode::ArrowRight if !shift && !self.range().is_empty() => {
                            self.range().end
                        }
                        KeyCode::ArrowLeft => self.value[..self.cursor]
                            .char_indices()
                            .last()
                            .map_or(0, |(i, _)| i),
                        _ => self.value[self.cursor..]
                            .chars()
                            .next()
                            .map_or(self.cursor, |c| self.cursor + c.len_utf8()),
                    };
                    if !shift {
                        self.anchor = self.cursor;
                    }
                }
                _ => {
                    if let Some(text) = &event.text {
                        let text: String = text.chars().filter(|c| !c.is_control()).collect();
                        if !text.is_empty() {
                            self.insert(&text);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn clipboard_read() -> Option<String> {
    use objc2::{
        msg_send,
        rc::Retained,
        runtime::{AnyClass, AnyObject},
    };
    use objc2_foundation::{NSString, ns_string};
    // SAFETY: NSPasteboard's class and stringForType: signatures are fixed Cocoa APIs;
    // dispatch runs on the main thread and retains the returned string while copying.
    unsafe {
        let board: *mut AnyObject = msg_send![AnyClass::get(c"NSPasteboard")?, generalPasteboard];
        let value: Option<Retained<NSString>> =
            msg_send![board, stringForType: ns_string!("public.utf8-plain-text")];
        value.map(|s| s.to_string())
    }
}
#[cfg(target_os = "macos")]
fn clipboard_write(text: &str) {
    use objc2::{
        msg_send,
        runtime::{AnyClass, AnyObject},
    };
    use objc2_foundation::{NSString, ns_string};
    let Some(class) = AnyClass::get(c"NSPasteboard") else {
        return;
    };
    // SAFETY: standard pasteboard selectors with the documented Cocoa signatures.
    unsafe {
        let board: *mut AnyObject = msg_send![class, generalPasteboard];
        let _: isize = msg_send![board, clearContents];
        let _: bool = msg_send![board, setString: &*NSString::from_str(text), forType: ns_string!("public.utf8-plain-text")];
    }
}
#[cfg(not(target_os = "macos"))]
use crate::app::clipboard::{read as clipboard_read, write as clipboard_write};
