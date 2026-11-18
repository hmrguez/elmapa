//! Keep X11 selection ownership between copy and paste operations.
use arboard::Clipboard;
use std::cell::RefCell;

thread_local! {
    static CLIPBOARD: RefCell<Option<Clipboard>> = const { RefCell::new(None) };
}

fn with_clipboard<T>(
    operation: impl FnOnce(&mut Clipboard) -> Result<T, arboard::Error>,
) -> Option<T> {
    CLIPBOARD.with_borrow_mut(|slot| {
        if slot.is_none() {
            *slot = Clipboard::new().ok();
        }
        operation(slot.as_mut()?).ok()
    })
}

pub(super) fn read() -> Option<String> {
    with_clipboard(|clipboard| clipboard.get_text())
}

pub(super) fn write(text: &str) {
    with_clipboard(|clipboard| clipboard.set_text(text));
}
