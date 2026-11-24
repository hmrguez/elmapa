use super::{app, button, key, moved};
use crate::app::{
    files::Jobs,
    state::{Action, Studio},
    ui::Hit,
};
use bevy::input::ButtonState;
use bevy::prelude::{ComputedNode, KeyCode, UiGlobalTransform, Vec2, default};
use elmapa::library::VolumeChannel;

#[test]
fn dragging_a_volume_slider_updates_live_and_saves_on_release_outside() {
    let (mut app, window) = app();
    {
        let mut studio = app.world_mut().resource_mut::<Studio>();
        studio.home = true;
        studio.settings = true;
        studio.audio_settings = true;
    }
    app.world_mut().spawn((
        Hit {
            action: Some(Action::AudioSlider(VolumeChannel::Brush)),
            order: 1,
        },
        ComputedNode {
            size: Vec2::new(200., 36.),
            inverse_scale_factor: 1.,
            ..default()
        },
        UiGlobalTransform::from_translation(Vec2::new(200., 100.)),
    ));
    moved(&mut app, window, Vec2::new(225., 100.));
    button(&mut app, window, ButtonState::Pressed);
    app.update();
    assert_eq!(
        app.world()
            .resource::<Studio>()
            .audio_levels
            .level(VolumeChannel::Brush),
        63
    );
    assert!(!app.world().resource::<Jobs>().preferences);

    moved(&mut app, window, Vec2::new(150., 200.));
    app.update();
    assert_eq!(
        app.world()
            .resource::<Studio>()
            .audio_levels
            .level(VolumeChannel::Brush),
        25
    );
    assert!(!app.world().resource::<Jobs>().preferences);

    button(&mut app, window, ButtonState::Released);
    app.update();
    let studio = app.world().resource::<Studio>();
    assert_eq!(studio.audio_levels.level(VolumeChannel::Brush), 25);
    assert_eq!(studio.audio_levels.level(VolumeChannel::Tools), 100);
    assert_eq!(studio.editor.stroke_count(), 0);
    assert!(app.world().resource::<Jobs>().preferences);
}

#[test]
fn keyboard_adjusts_focused_volumes_and_mute_keeps_the_mix() {
    let (mut app, window) = app();
    {
        let mut studio = app.world_mut().resource_mut::<Studio>();
        studio.home = true;
        studio.settings = true;
        studio.audio_settings = true;
    }
    for code in [
        KeyCode::Tab,
        KeyCode::ArrowLeft,
        KeyCode::Tab,
        KeyCode::Home,
    ] {
        key(&mut app, window, code, ButtonState::Pressed);
        key(&mut app, window, code, ButtonState::Released);
    }
    app.update();
    {
        let studio = app.world().resource::<Studio>();
        assert_eq!(studio.audio_focus, Some(VolumeChannel::Brush));
        assert_eq!(studio.audio_levels.level(VolumeChannel::Master), 95);
        assert_eq!(studio.audio_levels.level(VolumeChannel::Brush), 0);
    }
    let before = app.world().resource::<Studio>().audio_levels;
    app.world_mut()
        .resource_scope(|world, mut studio: bevy::prelude::Mut<Studio>| {
            studio.act(Action::SoundEffects, &mut world.resource_mut::<Jobs>());
        });
    let studio = app.world().resource::<Studio>();
    assert!(studio.sound_effects_disabled);
    assert_eq!(studio.audio_levels, before);
    assert!(app.world().resource::<Jobs>().preferences);
}
