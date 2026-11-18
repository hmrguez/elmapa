use super::{app, button, key, moved};
use crate::app::state::{Action, Studio};
use crate::app::ui::Hit;
use bevy::input::ButtonState;
use bevy::prelude::{App, ComputedNode, KeyCode, Real, Time, UiGlobalTransform, Vec2, default};
use bevy::winit::RawWinitWindowEvent;
use elmapa::document::Brush;

#[test]
fn bevy_pointer_dispatch_blocks_panels_and_commits_release_over_chrome() {
    let (mut app, window) = app();
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
    moved(&mut app, window, Vec2::new(100., 50.));
    button(&mut app, window, ButtonState::Pressed);
    app.update();
    assert!(app.world().resource::<Studio>().stroke.is_none());
    button(&mut app, window, ButtonState::Released);
    app.update();
    moved(&mut app, window, Vec2::new(400., 400.));
    button(&mut app, window, ButtonState::Pressed);
    app.update();
    moved(&mut app, window, Vec2::new(100., 50.));
    button(&mut app, window, ButtonState::Released);
    app.update();
    let s = app.world().resource::<Studio>();
    assert_eq!(s.editor.stroke_count(), 1);
    assert_eq!(s.editor.document().layers[0].strokes[0].points.len(), 1);
}
#[test]
fn option_click_opens_materials_without_painting_and_selection_sets_the_next_stroke() {
    for brush in [Brush::Land, Brush::Forest, Brush::Mountain] {
        let (mut app, window) = app();
        app.world_mut().resource_mut::<Studio>().brush = brush;
        let anchor = Vec2::new(400., 400.);
        let swatch = Vec2::new(400., 320.);
        let index = elmapa::brush_materials::for_brush(brush).nth(1).unwrap().0;
        app.world_mut().spawn((
            Hit {
                action: Some(Action::BrushMaterial(index)),
                order: 1,
            },
            ComputedNode {
                size: Vec2::splat(36.),
                inverse_scale_factor: 1.,
                ..default()
            },
            UiGlobalTransform::from_translation(swatch),
        ));
        moved(&mut app, window, anchor);
        key(&mut app, window, KeyCode::AltRight, ButtonState::Pressed);
        button(&mut app, window, ButtonState::Pressed);
        key(&mut app, window, KeyCode::AltRight, ButtonState::Released);
        button(&mut app, window, ButtonState::Released);
        app.update();
        let studio = app.world().resource::<Studio>();
        assert_eq!(studio.brush_orbit.anchor, Some(anchor));
        assert_eq!(studio.editor.stroke_count(), 0);

        moved(&mut app, window, swatch);
        button(&mut app, window, ButtonState::Pressed);
        button(&mut app, window, ButtonState::Released);
        app.update();
        let studio = app.world().resource::<Studio>();
        assert!(studio.brush_orbit.anchor.is_none());
        assert_eq!(studio.editor.stroke_count(), 0);

        moved(&mut app, window, anchor);
        button(&mut app, window, ButtonState::Pressed);
        button(&mut app, window, ButtonState::Released);
        app.update();
        let studio = app.world().resource::<Studio>();
        assert_eq!(studio.editor.stroke_count(), 1);
        let stroke = &studio.editor.document().layers[0].strokes[0];
        assert_eq!(
            stroke.material.as_deref(),
            Some(elmapa::brush_materials::catalog()[index].id.as_str()),
        );
    }
}

#[test]
fn native_modifier_flags_remain_paired_with_each_click_in_a_frame() {
    use winit::{
        event::{DeviceId, ElementState, MouseButton as NativeButton, WindowEvent as NativeEvent},
        keyboard::ModifiersState,
        window::WindowId,
    };
    let (mut app, window) = app();
    let anchor = Vec2::new(400., 400.);
    moved(&mut app, window, anchor);
    for modifiers in [ModifiersState::ALT, ModifiersState::empty()] {
        app.world_mut().write_message(RawWinitWindowEvent {
            window_id: WindowId::dummy(),
            event: NativeEvent::ModifiersChanged(modifiers.into()),
        });
        app.world_mut().write_message(RawWinitWindowEvent {
            window_id: WindowId::dummy(),
            event: NativeEvent::MouseInput {
                device_id: DeviceId::dummy(),
                state: ElementState::Pressed,
                button: NativeButton::Left,
            },
        });
        button(&mut app, window, ButtonState::Pressed);
        button(&mut app, window, ButtonState::Released);
    }
    app.update();
    let studio = app.world().resource::<Studio>();
    // The first click opens the Orbit. The next click dismisses it.
    assert_eq!(studio.brush_orbit.generation, 1);
    assert!(studio.brush_orbit.anchor.is_none());
    assert_eq!(studio.editor.stroke_count(), 0);
}

#[test]
fn stationary_land_press_opens_on_threshold_and_consumes_release_and_dismissal() {
    let (mut app, window) = app();
    let anchor = Vec2::new(400., 400.);
    moved(&mut app, window, anchor);
    button(&mut app, window, ButtonState::Pressed);
    app.update();
    let advance = |app: &mut App, millis| {
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(std::time::Duration::from_millis(millis));
        app.update();
    };
    moved(&mut app, window, anchor + Vec2::new(2., 1.));
    advance(&mut app, 1499);
    let studio = app.world().resource::<Studio>();
    assert!(studio.stroke.is_none() && studio.brush_orbit.anchor.is_none());
    advance(&mut app, 1);
    let studio = app.world().resource::<Studio>();
    assert_eq!(studio.brush_orbit.anchor, Some(anchor));
    assert!(studio.stroke.is_none());

    moved(&mut app, window, Vec2::new(600., 500.));
    button(&mut app, window, ButtonState::Released);
    app.update();
    assert!(
        app.world()
            .resource::<Studio>()
            .brush_orbit
            .anchor
            .is_some()
    );

    button(&mut app, window, ButtonState::Pressed);
    button(&mut app, window, ButtonState::Released);
    app.update();
    let studio = app.world().resource::<Studio>();
    assert!(studio.brush_orbit.anchor.is_none());
    assert_eq!(studio.editor.stroke_count(), 0);
}

#[test]
fn a_land_drag_keeps_painting_when_it_pauses_past_the_hold_threshold() {
    let (mut app, window) = app();
    moved(&mut app, window, Vec2::new(400., 400.));
    button(&mut app, window, ButtonState::Pressed);
    moved(&mut app, window, Vec2::new(450., 410.));
    app.update();
    assert!(app.world().resource::<Studio>().stroke.is_some());
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(std::time::Duration::from_secs(2));
    app.update();
    button(&mut app, window, ButtonState::Released);
    app.update();
    let studio = app.world().resource::<Studio>();
    assert!(studio.brush_orbit.anchor.is_none());
    assert_eq!(studio.editor.stroke_count(), 1);
    assert_eq!(
        studio.editor.document().layers[0].strokes[0].points.len(),
        2
    );
}

#[test]
fn native_coalesced_drag_keeps_every_point_and_one_history_entry() {
    let (mut app, window) = app();
    moved(&mut app, window, Vec2::new(100., 300.));
    button(&mut app, window, ButtonState::Pressed);
    for position in [
        Vec2::new(200., 300.),
        Vec2::new(300., 400.),
        Vec2::new(450., 500.),
    ] {
        moved(&mut app, window, position);
    }
    button(&mut app, window, ButtonState::Released);
    app.update();
    let s = app.world().resource::<Studio>();
    assert_eq!(s.editor.stroke_count(), 1);
    assert_eq!(s.editor.document().layers[0].strokes[0].points.len(), 4);
}
