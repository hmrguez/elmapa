use super::{app, button, key, moved};
use crate::app::state::{Action, Studio};
use crate::app::ui;
use crate::app::ui::{Hit, ScrollArea};
use bevy::input::ButtonState;
use bevy::input::mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel};
use bevy::prelude::{
    App, ComputedNode, Entity, KeyCode, MouseButton, ScrollPosition, UiGlobalTransform, Vec2,
    default,
};
use bevy::window::WindowEvent;
use elmapa::document::Brush;

fn middle(app: &mut App, window: Entity, state: ButtonState) {
    app.world_mut()
        .write_message(WindowEvent::MouseButtonInput(MouseButtonInput {
            window,
            button: MouseButton::Middle,
            state,
        }));
}
#[test]
fn shift_is_read_at_landmark_press_and_releasing_it_restores_selection() {
    use crate::app::state::Tool;
    let (mut app, window) = app();
    key(&mut app, window, KeyCode::KeyL, ButtonState::Pressed);
    key(&mut app, window, KeyCode::ShiftRight, ButtonState::Pressed);
    for point in [Vec2::new(360., 400.), Vec2::new(500., 400.)] {
        moved(&mut app, window, point);
        button(&mut app, window, ButtonState::Pressed);
        button(&mut app, window, ButtonState::Released);
        app.update();
        let studio = app.world().resource::<Studio>();
        assert_eq!(studio.tool, Tool::Landmarks);
        assert!(studio.landmarks.selected.is_none());
    }

    key(&mut app, window, KeyCode::ShiftRight, ButtonState::Released);
    moved(&mut app, window, Vec2::new(640., 400.));
    button(&mut app, window, ButtonState::Pressed);
    button(&mut app, window, ButtonState::Released);
    app.update();
    let studio = app.world().resource::<Studio>();
    let landmarks = &studio.editor.document().layers[0].landmarks;
    assert_eq!(landmarks.len(), 3);
    assert_eq!(studio.tool, Tool::Select);
    assert_eq!(studio.landmarks.selected, Some(landmarks[2].id));
}

#[test]
fn middle_drag_pans_only_while_held_and_keeps_the_selected_brush() {
    let (mut app, window) = app();
    key(&mut app, window, KeyCode::Digit3, ButtonState::Pressed);
    moved(&mut app, window, Vec2::new(400., 350.));
    middle(&mut app, window, ButtonState::Pressed);
    moved(&mut app, window, Vec2::new(450., 380.));
    // A left click during a pan must neither paint nor release the pan.
    button(&mut app, window, ButtonState::Pressed);
    button(&mut app, window, ButtonState::Released);
    moved(&mut app, window, Vec2::new(500., 410.));
    middle(&mut app, window, ButtonState::Released);
    moved(&mut app, window, Vec2::new(600., 500.));
    app.update();
    let s = app.world().resource::<Studio>();
    assert_eq!(s.viewport.pan, elmapa::document::Point::new(100., 60.));
    assert!(s.drag.is_none() && s.stroke.is_none());
    assert_eq!(s.editor.stroke_count(), 0);
    assert_eq!(s.brush, Brush::Forest);
    button(&mut app, window, ButtonState::Pressed);
    button(&mut app, window, ButtonState::Released);
    app.update();
    assert_eq!(app.world().resource::<Studio>().editor.stroke_count(), 1);
}
#[test]
fn h_does_not_toggle_panning_or_replace_the_selected_brush() {
    let (mut app, window) = app();
    key(&mut app, window, KeyCode::KeyH, ButtonState::Pressed);
    moved(&mut app, window, Vec2::new(400., 350.));
    button(&mut app, window, ButtonState::Pressed);
    moved(&mut app, window, Vec2::new(500., 410.));
    button(&mut app, window, ButtonState::Released);
    app.update();
    let s = app.world().resource::<Studio>();
    assert_eq!(s.viewport.pan, elmapa::document::Point::new(0., 0.));
    assert_eq!(s.editor.stroke_count(), 1);
}
#[test]
fn middle_pan_blocks_panel_starts_and_stops_on_release_over_a_panel() {
    let (mut app, window) = app();
    app.world_mut().spawn((
        Hit {
            action: Some(Action::AddLayer),
            order: 1,
        },
        ComputedNode {
            size: Vec2::new(200., 100.),
            inverse_scale_factor: 1.,
            ..default()
        },
        UiGlobalTransform::from_translation(Vec2::new(100., 50.)),
    ));
    let layers = app
        .world()
        .resource::<Studio>()
        .editor
        .document()
        .layers
        .len();
    moved(&mut app, window, Vec2::new(100., 50.));
    middle(&mut app, window, ButtonState::Pressed);
    moved(&mut app, window, Vec2::new(400., 350.));
    middle(&mut app, window, ButtonState::Released);
    app.update();
    assert_eq!(
        app.world().resource::<Studio>().viewport.pan,
        elmapa::document::Point::new(0., 0.)
    );
    middle(&mut app, window, ButtonState::Pressed);
    moved(&mut app, window, Vec2::new(450., 380.));
    moved(&mut app, window, Vec2::new(100., 50.));
    moved(&mut app, window, Vec2::new(110., 60.));
    // Leaving a panel must not apply movement accumulated underneath it.
    moved(&mut app, window, Vec2::new(210., 110.));
    moved(&mut app, window, Vec2::new(100., 50.));
    middle(&mut app, window, ButtonState::Released);
    moved(&mut app, window, Vec2::new(600., 500.));
    app.update();
    let s = app.world().resource::<Studio>();
    assert_eq!(s.viewport.pan, elmapa::document::Point::new(150., 80.));
    assert!(s.drag.is_none());
    assert_eq!(s.editor.document().layers.len(), layers);
    assert_eq!(s.editor.stroke_count(), 0);
}
#[test]
fn escape_focus_loss_and_cursorless_release_stop_middle_panning() {
    for stop in 0..3 {
        let (mut app, window) = app();
        moved(&mut app, window, Vec2::new(400., 350.));
        middle(&mut app, window, ButtonState::Pressed);
        moved(&mut app, window, Vec2::new(450., 380.));
        match stop {
            0 => key(&mut app, window, KeyCode::Escape, ButtonState::Pressed),
            1 => {
                app.world_mut().write_message(WindowEvent::WindowFocused(
                    bevy::window::WindowFocused {
                        window,
                        focused: false,
                    },
                ));
            }
            _ => {
                app.world_mut()
                    .write_message(WindowEvent::CursorLeft(bevy::window::CursorLeft { window }));
                middle(&mut app, window, ButtonState::Released);
            }
        }
        moved(&mut app, window, Vec2::new(600., 500.));
        app.update();
        let s = app.world().resource::<Studio>();
        assert_eq!(s.viewport.pan, elmapa::document::Point::new(50., 30.));
        assert!(s.drag.is_none());
        assert_eq!(s.editor.stroke_count(), 0);
    }
}
#[test]
fn middle_pan_finishes_existing_paint_without_extending_it_during_pan() {
    let (mut app, window) = app();
    moved(&mut app, window, Vec2::new(400., 350.));
    button(&mut app, window, ButtonState::Pressed);
    middle(&mut app, window, ButtonState::Pressed);
    moved(&mut app, window, Vec2::new(500., 410.));
    middle(&mut app, window, ButtonState::Released);
    moved(&mut app, window, Vec2::new(600., 500.));
    button(&mut app, window, ButtonState::Released);
    app.update();
    let s = app.world().resource::<Studio>();
    assert_eq!(s.editor.stroke_count(), 1);
    assert_eq!(s.editor.document().layers[0].strokes[0].points.len(), 1);
    assert!(s.stroke.is_none() && s.drag.is_none());
}
fn wheel(app: &mut App, window: Entity, unit: MouseScrollUnit, x: f32, y: f32) {
    app.world_mut()
        .write_message(WindowEvent::MouseWheel(MouseWheel {
            window,
            unit,
            x,
            y,
            phase: bevy::input::touch::TouchPhase::Moved,
        }));
}
#[test]
fn plain_scroll_zooms_both_directions_and_keeps_pointer_anchored() {
    for (unit, amount) in [(MouseScrollUnit::Pixel, 100.), (MouseScrollUnit::Line, 5.)] {
        for direction in [-1., 1.] {
            let (mut app, window) = app();
            let point = Vec2::new(400., 350.);
            let bounds = app.world().resource::<Studio>().bounds;
            app.world_mut()
                .resource_mut::<Studio>()
                .viewport
                .resolve(bounds);
            let before = app.world().resource::<Studio>().map_point(point);
            let zoom = app.world().resource::<Studio>().viewport.zoom;
            moved(&mut app, window, point);
            wheel(&mut app, window, unit, 50., direction * amount);
            app.update();
            let studio = app.world().resource::<Studio>();
            let after = studio.map_point(point);
            assert!((before.x - after.x).abs() < 0.001 && (before.y - after.y).abs() < 0.001);
            assert!((studio.viewport.zoom / zoom - (direction * 0.5).exp()).abs() < 0.001);
        }
    }
}
#[test]
fn command_scroll_still_zooms_after_modifier_release_before_the_frame() {
    let (mut app, window) = app();
    let point = Vec2::new(400., 350.);
    let bounds = app.world().resource::<Studio>().bounds;
    app.world_mut()
        .resource_mut::<Studio>()
        .viewport
        .resolve(bounds);
    let zoom = app.world().resource::<Studio>().viewport.zoom;
    moved(&mut app, window, point);
    key(
        &mut app,
        window,
        crate::app::input::SHORTCUT_KEYS[0],
        ButtonState::Pressed,
    );
    wheel(&mut app, window, MouseScrollUnit::Pixel, 0., 100.);
    key(
        &mut app,
        window,
        crate::app::input::SHORTCUT_KEYS[0],
        ButtonState::Released,
    );
    app.update();
    assert!(app.world().resource::<Studio>().viewport.zoom > zoom);
}
#[test]
fn scroll_over_layer_list_or_panel_does_not_move_the_canvas() {
    let (mut app, window) = app();
    let node = ComputedNode {
        size: Vec2::new(200., 100.),
        content_size: Vec2::new(200., 400.),
        inverse_scale_factor: 1.,
        ..default()
    };
    let transform = UiGlobalTransform::from_translation(Vec2::new(100., 50.));
    app.world_mut().spawn((
        Hit {
            action: None,
            order: 1,
        },
        node,
        transform,
    ));
    let list = app
        .world_mut()
        .spawn((
            node,
            transform,
            ScrollPosition::default(),
            ScrollArea::Layers,
        ))
        .id();
    let before = app.world().resource::<Studio>().viewport;
    moved(&mut app, window, Vec2::new(100., 50.));
    wheel(&mut app, window, MouseScrollUnit::Pixel, 0., -100.);
    app.update();
    assert_eq!(app.world().get::<ScrollPosition>(list).unwrap().y, 100.);
    assert_eq!(app.world().resource::<ui::ScrollOffsets>().layers, 100.);
    assert!(app.world().resource::<Studio>().viewport == before);
    app.world_mut().despawn(list);
    wheel(&mut app, window, MouseScrollUnit::Pixel, 0., 100.);
    app.update();
    assert!(app.world().resource::<Studio>().viewport == before);
}
