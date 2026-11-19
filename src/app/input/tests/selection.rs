use super::{app, button, key, moved};
use crate::app::{state::Studio, viewport::Viewport};
use bevy::{input::ButtonState, prelude::KeyCode};
use elmapa::{
    document::{Brush, Landmark, Point, SelectionId, Stroke},
    editor::Editor,
    scene::SceneCache,
};

#[test]
fn shift_click_and_marquee_move_mixed_items_in_one_native_gesture() {
    let (mut app, window) = app();
    let (a, b, outside, base, world, revision) = {
        let mut studio = app.world_mut().resource_mut::<Studio>();
        studio.editor = Editor::blank();
        let a = studio
            .editor
            .add_landmark(Landmark {
                position: Point::new(340., 310.),
                ..Landmark::default()
            })
            .unwrap();
        let b = studio
            .editor
            .add_landmark(Landmark {
                position: Point::new(690., 300.),
                ..Landmark::default()
            })
            .unwrap();
        let hidden = studio.editor.add_layer();
        let outside = studio
            .editor
            .add_landmark(Landmark {
                position: Point::new(790., 310.),
                ..Landmark::default()
            })
            .unwrap();
        studio.editor.toggle_layer(hidden);
        studio.editor.select_layer(1);
        assert!(studio.editor.commit_stroke(Stroke {
            creation_id: None,
            material: None,
            brush: Brush::Path,
            points: vec![Point::new(260., 465.), Point::new(740., 465.)].into(),
            size: 100.,
            strength: 0.85,
            seed: 82,
        }));
        let world = studio.editor.document().clone();
        studio
            .scene
            .set_test_snapshot(SceneCache::default().render(&world, None));
        let bounds = studio.bounds;
        studio.viewport = Viewport::new(world.content_bounds());
        studio.viewport.resolve(bounds);
        let view = studio.viewport.transform(bounds);
        (a, b, outside, view, world, studio.editor.revision())
    };
    let screen = |x, y| base.origin + bevy::prelude::Vec2::new(x, y) * base.scale;
    let press = |app: &mut bevy::prelude::App, x, y| {
        moved(app, window, screen(x, y));
        button(app, window, ButtonState::Pressed);
    };
    let release = |app: &mut bevy::prelude::App, x, y| {
        moved(app, window, screen(x, y));
        button(app, window, ButtonState::Released);
    };
    key(&mut app, window, KeyCode::KeyV, ButtonState::Pressed);
    press(&mut app, 340., 310.);
    release(&mut app, 340., 310.);
    key(&mut app, window, KeyCode::ShiftLeft, ButtonState::Pressed);
    press(&mut app, 690., 300.);
    release(&mut app, 690., 300.);
    app.update();
    assert_eq!(app.world().resource::<Studio>().selection.items.len(), 2);

    // Toggle one landmark out, then add the visible landmarks and trail with a box.
    press(&mut app, 690., 300.);
    release(&mut app, 690., 300.);
    app.update();
    assert_eq!(app.world().resource::<Studio>().landmarks.selected, Some(a));
    press(&mut app, 205., 185.);
    release(&mut app, 850., 490.);
    key(&mut app, window, KeyCode::ShiftLeft, ButtonState::Released);
    app.update();
    let studio = app.world().resource::<Studio>();
    assert_eq!(studio.selection.items.len(), 3);
    assert!(
        studio
            .selection
            .items
            .iter()
            .any(|item| matches!(item.id, SelectionId::Course(_)))
    );

    press(&mut app, 340., 310.);
    moved(&mut app, window, screen(400., 353.));
    app.update();
    let studio = app.world().resource::<Studio>();
    assert_eq!(studio.editor.document(), &world);
    assert!(studio.preview_selection().is_some());
    key(&mut app, window, KeyCode::Escape, ButtonState::Pressed);
    release(&mut app, 400., 353.);
    app.update();
    assert_eq!(app.world().resource::<Studio>().editor.revision(), revision);

    press(&mut app, 340., 310.);
    release(&mut app, 400., 353.);
    app.update();
    let mut studio = app.world_mut().resource_mut::<Studio>();
    assert_eq!(studio.editor.revision(), revision + 1);
    let a = studio.editor.landmark(a).unwrap();
    let delta = Point::new(a.position.x - 340., a.position.y - 310.);
    assert!((delta.x - 60.).abs() < 0.01 && (delta.y - 43.).abs() < 0.01);
    let b = studio.editor.landmark(b).unwrap();
    assert_eq!(b.position, Point::new(690. + delta.x, 300. + delta.y));
    assert_eq!(
        studio.editor.landmark(outside).unwrap().position,
        Point::new(790., 310.)
    );
    for (&old, actual) in world.layers[0].strokes[0]
        .points
        .iter()
        .zip(&studio.editor.document().layers[0].strokes[0].points)
    {
        assert!(
            (actual.x - old.x - delta.x).abs() < 0.01 && (actual.y - old.y - delta.y).abs() < 0.01
        );
    }
    assert!(studio.editor.undo());
    assert_eq!(studio.editor.document(), &world);
}
