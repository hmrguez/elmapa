use super::{app, button, key, moved};
use crate::app::state::{Studio, Tool};
use crate::app::ui::Hit;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::{
    App, ComputedNode, Entity, KeyCode, Real, Time, UiGlobalTransform, Vec2, default,
};
use bevy::window::WindowEvent;
use elmapa::document::Brush;
use std::time::Duration;

fn type_text(app: &mut App, window: Entity, key_code: KeyCode, text: &str) {
    app.world_mut()
        .write_message(WindowEvent::KeyboardInput(KeyboardInput {
            window,
            key_code,
            state: ButtonState::Pressed,
            logical_key: Key::Character(text.into()),
            text: Some(text.into()),
            repeat: false,
        }));
}

#[test]
fn select_double_click_creates_one_label_draft_on_empty_canvas() {
    let (mut app, window) = app();
    let point = Vec2::new(400., 400.);
    let click = |app: &mut App, point| {
        moved(app, window, point);
        button(app, window, ButtonState::Pressed);
        button(app, window, ButtonState::Released);
        app.update();
    };
    key(&mut app, window, KeyCode::KeyV, ButtonState::Pressed);
    click(&mut app, point);
    assert!(app.world().resource::<Studio>().text.draft.is_none());

    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(Duration::from_secs(5));
    click(&mut app, point);
    assert!(app.world().resource::<Studio>().text.draft.is_none());

    click(&mut app, point + Vec2::X);
    let studio = app.world().resource::<Studio>();
    assert_eq!(studio.tool, Tool::Select);
    assert!(studio.text.draft.is_some());
    assert_eq!(studio.draft_label().unwrap().text, "");
    assert_eq!(studio.draft_preview_label().unwrap().text, "Place name");
    assert!(studio.editor.document().layers[0].labels.is_empty());

    type_text(&mut app, window, KeyCode::KeyA, "A");
    key(&mut app, window, KeyCode::Enter, ButtonState::Pressed);
    app.update();
    let studio = app.world().resource::<Studio>();
    assert_eq!(studio.tool, Tool::Select);
    assert!(studio.text.draft.is_none());
    assert!(studio.text.selected.is_none());
    let labels = &studio.editor.document().layers[0].labels;
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].text, "A");
    let id = labels[0].id;

    click(&mut app, point + Vec2::X);
    click(&mut app, point + Vec2::X);
    let studio = app.world().resource::<Studio>();
    assert!(studio.text.draft.is_none());
    assert_eq!(studio.text.selected, Some(id));
}

#[test]
fn placement_draft_accepts_native_keys_and_ime_without_selecting_a_label() {
    let (mut app, window) = app();
    key(&mut app, window, KeyCode::KeyT, ButtonState::Pressed);
    moved(&mut app, window, Vec2::new(400., 400.));
    button(&mut app, window, ButtonState::Pressed);
    button(&mut app, window, ButtonState::Released);
    type_text(&mut app, window, KeyCode::KeyV, "V");
    for ime in [
        bevy::window::Ime::Preedit {
            window,
            value: "東京".into(),
            cursor: Some((0, 6)),
        },
        bevy::window::Ime::Commit {
            window,
            value: "東京".into(),
        },
    ] {
        app.world_mut().write_message(WindowEvent::Ime(ime));
    }
    app.update();
    let studio = app.world().resource::<Studio>();
    assert_eq!(studio.draft_label().unwrap().text, "V東京");
    assert!(studio.editor.document().layers[0].labels.is_empty());
    assert!(studio.text.selected.is_none());

    key(&mut app, window, KeyCode::Enter, ButtonState::Pressed);
    moved(&mut app, window, Vec2::new(600., 400.));
    button(&mut app, window, ButtonState::Pressed);
    button(&mut app, window, ButtonState::Released);
    type_text(&mut app, window, KeyCode::KeyL, "L");
    key(&mut app, window, KeyCode::Escape, ButtonState::Pressed);
    app.update();
    let studio = app.world().resource::<Studio>();
    assert_eq!(studio.tool, Tool::Text);
    assert_eq!(studio.editor.document().layers[0].labels.len(), 1);
    assert_eq!(studio.editor.document().layers[0].labels[0].text, "V東京");
    assert!(studio.text.draft.is_none());
}

#[test]
fn selected_label_owns_typing_until_escape_and_commits_one_undoable_edit() {
    let (mut app, window) = app();
    let point = Vec2::new(400., 400.);
    let id = {
        let mut s = app.world_mut().resource_mut::<Studio>();
        let position = s.map_point(point);
        s.editor
            .add_label(elmapa::document::Label {
                position,
                ..default()
            })
            .unwrap()
    };
    key(&mut app, window, KeyCode::KeyV, ButtonState::Pressed);
    moved(&mut app, window, point);
    button(&mut app, window, ButtonState::Pressed);
    button(&mut app, window, ButtonState::Released);
    app.update();
    assert_eq!(app.world().resource::<Studio>().text.selected, Some(id));
    assert!(app.world().resource::<Studio>().text.edit.is_none());

    key(
        &mut app,
        window,
        crate::app::input::SHORTCUT_KEYS[0],
        ButtonState::Pressed,
    );
    for shortcut in [KeyCode::KeyD, KeyCode::KeyZ, KeyCode::KeyQ] {
        key(&mut app, window, shortcut, ButtonState::Pressed);
    }
    key(
        &mut app,
        window,
        crate::app::input::SHORTCUT_KEYS[0],
        ButtonState::Released,
    );
    app.update();
    let s = app.world().resource::<Studio>();
    assert!(!s.quit_requested);
    assert!(s.text.edit.is_none());
    let revision = s.editor.revision();

    for (key_code, text) in [
        (KeyCode::KeyP, "P"),
        (KeyCode::Digit1, "1"),
        (KeyCode::BracketLeft, "["),
        (KeyCode::KeyV, "v"),
        (KeyCode::KeyE, "É"),
    ] {
        type_text(&mut app, window, key_code, text);
    }
    app.world_mut()
        .write_message(WindowEvent::Ime(bevy::window::Ime::Commit {
            window,
            value: "東京".into(),
        }));
    key(&mut app, window, KeyCode::Backspace, ButtonState::Pressed);
    app.update();
    let s = app.world().resource::<Studio>();
    assert_eq!(s.current_label().text, "P1[vÉ東");
    assert_eq!(s.editor.revision(), revision);
    assert_eq!(s.editor.stroke_count(), 0);
    assert_eq!(s.brush, Brush::Land);

    key(&mut app, window, KeyCode::Escape, ButtonState::Pressed);
    key(&mut app, window, KeyCode::Digit3, ButtonState::Pressed);
    app.update();
    let s = app.world().resource::<Studio>();
    assert_eq!(s.editor.label(id).unwrap().text, "P1[vÉ東");
    assert_eq!(s.editor.revision(), revision + 1);
    assert!(s.text.selected.is_none());
    assert!(s.text.edit.is_none());
    assert_eq!(s.brush, Brush::Forest);
}

#[test]
fn clicking_outside_typography_commits_the_value_and_dismisses_the_card() {
    use crate::app::lettering::Field;
    let (mut app, window) = app();
    let point = Vec2::new(100., 50.);
    let id = {
        let mut s = app.world_mut().resource_mut::<Studio>();
        let id = s
            .editor
            .add_label(elmapa::document::Label::default())
            .unwrap();
        s.tool = Tool::Select;
        s.text.selected = Some(id);
        s.focus_text(Field::Curvature);
        id
    };
    type_text(&mut app, window, KeyCode::Minus, "-");
    type_text(&mut app, window, KeyCode::Digit4, "4");
    type_text(&mut app, window, KeyCode::Digit5, "5");
    app.world_mut().spawn((
        Hit {
            action: None,
            order: 1,
        },
        ComputedNode {
            size: Vec2::new(200., 100.),
            inverse_scale_factor: 1.,
            ..default()
        },
        UiGlobalTransform::from_translation(Vec2::new(100., 50.)),
    ));
    moved(&mut app, window, point);
    button(&mut app, window, ButtonState::Pressed);
    button(&mut app, window, ButtonState::Released);
    app.update();
    let s = app.world().resource::<Studio>();
    assert_eq!(s.editor.label(id).unwrap().curvature, -45.);
    assert_eq!(s.text.selected, Some(id));
    assert!(s.text.control.is_none());
    assert!(s.text.edit.is_none());
    assert_eq!(s.editor.stroke_count(), 0);
}
