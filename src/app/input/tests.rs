//! A native input dispatch fixture shared by the gesture and shortcut suites.
mod audio;
mod canvas;
mod lettering;
mod painting;
mod selection;
mod shortcuts;

use super::dispatch;
use crate::app::{files::Jobs, state::Studio, ui};
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput, NativeKey},
        mouse::MouseButtonInput,
    },
    prelude::{App, Entity, KeyCode, MouseButton, Real, Time, Update, Vec2, Window, default},
    window::{CursorMoved, WindowEvent},
    winit::RawWinitWindowEvent,
};

fn app() -> (App, Entity) {
    let mut app = App::new();
    app.init_resource::<Jobs>()
        .init_resource::<Time<Real>>()
        .init_resource::<ui::ScrollOffsets>()
        .init_resource::<crate::app::replay::Replay>()
        .add_message::<WindowEvent>()
        .add_message::<RawWinitWindowEvent>();
    app.insert_resource(Studio {
        home: false,
        ..default()
    })
    .add_systems(Update, dispatch);
    let window = app.world_mut().spawn(Window::default()).id();
    (app, window)
}
fn key(app: &mut App, window: Entity, key_code: KeyCode, state: ButtonState) {
    app.world_mut()
        .write_message(WindowEvent::KeyboardInput(KeyboardInput {
            window,
            key_code,
            state,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            text: None,
            repeat: false,
        }));
}
fn moved(app: &mut App, window: Entity, position: Vec2) {
    app.world_mut()
        .write_message(WindowEvent::CursorMoved(CursorMoved {
            window,
            position,
            delta: None,
        }));
}
fn button(app: &mut App, window: Entity, state: ButtonState) {
    app.world_mut()
        .write_message(WindowEvent::MouseButtonInput(MouseButtonInput {
            window,
            button: MouseButton::Left,
            state,
        }));
}
