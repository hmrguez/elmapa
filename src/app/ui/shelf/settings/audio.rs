//! Volume controls and previews in the shelf Settings dialog.
use super::{Action, Builder, Palette, Studio, color, column, row};
use bevy::prelude::*;
use elmapa::library::{AudioLevels, VolumeChannel};

#[derive(Component, Clone, Copy)]
pub(in crate::app) enum LevelGraphic {
    Fill(VolumeChannel),
    Thumb(VolumeChannel),
}

#[derive(Component)]
pub(in crate::app) struct LevelText(VolumeChannel);

pub(super) fn build(b: &mut Builder, parent: Entity, studio: &Studio, p: Palette) {
    let mute = b.node(
        parent,
        Node {
            width: percent(100.),
            justify_content: JustifyContent::SpaceBetween,
            ..row(0.)
        },
    );
    let enabled = !studio.sound_effects_disabled;
    b.text(mute, "Sound effects", 13., p.text, false);
    b.shelf_button(
        mute,
        if enabled { "check" } else { "minus" },
        if enabled { "On" } else { "Off" },
        Action::SoundEffects,
    );

    let levels = b.node(parent, column(10.));
    for (channel, label) in [
        (VolumeChannel::Master, "Master volume"),
        (VolumeChannel::Brush, "Brush sounds"),
        (VolumeChannel::Tools, "Tool sounds"),
    ] {
        slider(b, levels, studio, p, channel, label);
    }

    let previews = b.node(
        parent,
        Node {
            min_height: px(42.),
            ..row(8.)
        },
    );
    if enabled {
        b.shelf_button(
            previews,
            "land",
            "Preview brush",
            Action::PreviewAudio(VolumeChannel::Brush),
        );
        b.shelf_button(
            previews,
            "select",
            "Preview tool",
            Action::PreviewAudio(VolumeChannel::Tools),
        );
    } else {
        b.text(
            previews,
            "Turn sound effects on to preview.",
            12.,
            p.muted,
            false,
        );
    }
}

fn slider(
    b: &mut Builder,
    parent: Entity,
    studio: &Studio,
    p: Palette,
    channel: VolumeChannel,
    label: &str,
) {
    let value = studio.audio_levels.level(channel);
    let group = b.node(parent, column(0.));
    let header = b.node(
        group,
        Node {
            justify_content: JustifyContent::SpaceBetween,
            ..row(8.)
        },
    );
    b.text(header, label, 13., p.text, false);
    let text = b.text(header, format!("{value}%"), 12., p.text, false);
    b.commands.entity(text).insert(LevelText(channel));

    let area = b.node(
        group,
        Node {
            width: percent(100.),
            height: px(36.),
            padding: UiRect::horizontal(px(8.)),
            ..row(0.)
        },
    );
    let track = b.node(
        area,
        Node {
            width: percent(100.),
            height: percent(100.),
            ..row(0.)
        },
    );
    b.hit(track, Some(Action::AudioSlider(channel)));
    let groove = b.node(
        track,
        Node {
            width: percent(100.),
            height: px(4.),
            border_radius: BorderRadius::MAX,
            ..default()
        },
    );
    b.commands
        .entity(groove)
        .insert(BackgroundColor(color(p.wash, 1.)));
    let fill = b.node(
        groove,
        Node {
            width: percent(f32::from(value)),
            height: percent(100.),
            border_radius: BorderRadius::MAX,
            ..default()
        },
    );
    b.commands.entity(fill).insert((
        LevelGraphic::Fill(channel),
        BackgroundColor(color(p.accent, 1.)),
    ));
    let thumb = b.node(
        track,
        Node {
            position_type: PositionType::Absolute,
            left: percent(f32::from(value)),
            top: px(11.),
            margin: UiRect::left(px(-7.)),
            width: px(14.),
            height: px(14.),
            border: UiRect::all(px(1.)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
    );
    let mut thumb = b.commands.entity(thumb);
    thumb.insert((
        LevelGraphic::Thumb(channel),
        BackgroundColor(color(p.surface, 1.)),
        BorderColor::all(color(p.accent, 1.)),
    ));
    if studio.audio_focus == Some(channel) {
        thumb.insert(Outline::new(px(2.), px(3.), color(p.accent, 0.9)));
    }
}

pub(in crate::app) fn update_levels(
    studio: Res<Studio>,
    mut previous: Local<Option<AudioLevels>>,
    mut graphics: Query<(&LevelGraphic, &mut Node)>,
    mut readouts: Query<(&LevelText, &mut Text)>,
) {
    if *previous == Some(studio.audio_levels) {
        return;
    }
    *previous = Some(studio.audio_levels);
    for (graphic, mut node) in &mut graphics {
        let (channel, position) = match *graphic {
            LevelGraphic::Fill(channel) => (channel, &mut node.width),
            LevelGraphic::Thumb(channel) => (channel, &mut node.left),
        };
        let value = percent(f32::from(studio.audio_levels.level(channel)));
        if *position != value {
            *position = value;
        }
    }
    for (readout, mut text) in &mut readouts {
        let value = format!("{}%", studio.audio_levels.level(readout.0));
        if text.0 != value {
            text.0 = value;
        }
    }
}
