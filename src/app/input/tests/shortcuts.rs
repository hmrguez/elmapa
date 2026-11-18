use super::{app, button, key, moved};
use crate::app::files::Jobs;
use crate::app::state::Studio;
use bevy::input::ButtonState;
use bevy::prelude::{KeyCode, Vec2};
use elmapa::document::Brush;

#[test]
fn select_shortcut_does_not_create_text_or_focus_a_field() {
    let (mut app, window) = app();
    key(&mut app, window, KeyCode::KeyV, ButtonState::Pressed);
    moved(&mut app, window, Vec2::new(400., 400.));
    button(&mut app, window, ButtonState::Pressed);
    button(&mut app, window, ButtonState::Released);
    key(&mut app, window, KeyCode::Enter, ButtonState::Pressed);
    app.update();
    let s = app.world().resource::<Studio>();
    assert_eq!(s.tool, crate::app::state::Tool::Select);
    assert!(s.text.edit.is_none());
    assert_eq!(s.editor.stroke_count(), 0);
    assert!(
        s.editor
            .document()
            .layers
            .iter()
            .all(|l| l.labels.is_empty())
    );
    key(&mut app, window, KeyCode::KeyT, ButtonState::Pressed);
    app.update();
    key(&mut app, window, KeyCode::Enter, ButtonState::Pressed);
    app.update();
    assert!(
        app.world().resource::<Studio>().text.edit.is_none(),
        "Enter before placement must not focus an invisible name field"
    );
}
#[test]
fn bevy_keyboard_dispatch_preserves_shortcuts_and_escape_cancels() {
    let (mut app, window) = app();
    key(&mut app, window, KeyCode::Digit3, ButtonState::Pressed);
    app.update();
    assert_eq!(app.world().resource::<Studio>().brush, Brush::Forest);
    app.world_mut()
        .resource_mut::<Studio>()
        .down(Vec2::new(400., 400.));
    key(&mut app, window, KeyCode::Escape, ButtonState::Pressed);
    app.update();
    assert!(app.world().resource::<Studio>().stroke.is_none());
    assert_eq!(app.world().resource::<Studio>().editor.stroke_count(), 0);
}

#[test]
fn command_shortcut_survives_modifier_release_before_the_frame() {
    let (mut app, window) = app();
    for (code, state) in [
        (crate::app::input::SHORTCUT_KEYS[0], ButtonState::Pressed),
        (KeyCode::KeyS, ButtonState::Pressed),
        (KeyCode::KeyS, ButtonState::Released),
        (crate::app::input::SHORTCUT_KEYS[0], ButtonState::Released),
    ] {
        key(&mut app, window, code, state);
    }
    app.update();
    assert!(app.world().resource::<Jobs>().busy());
}
