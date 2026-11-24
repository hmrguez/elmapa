//! Capture volume slider gestures from native input without reloading the dialog.
use crate::app::{
    files::Jobs,
    state::{Action, Studio},
};
use bevy::prelude::*;
use elmapa::library::VolumeChannel;

#[derive(Clone, Copy)]
pub(super) struct VolumeDrag {
    channel: VolumeChannel,
    track: Rect,
    initial: u8,
}

impl VolumeDrag {
    pub(super) fn begin(studio: &mut Studio, channel: VolumeChannel, track: Rect, x: f32) -> Self {
        let initial = studio.audio_levels.level(channel);
        let drag = Self {
            channel,
            track,
            initial,
        };
        focus(studio, channel);
        drag.moved(studio, x);
        drag
    }

    pub(super) fn moved(self, studio: &mut Studio, x: f32) {
        let ratio = ((x - self.track.min.x) / self.track.width()).clamp(0., 1.);
        studio
            .audio_levels
            .set(self.channel, (ratio * 100.).round() as u8);
    }

    pub(super) fn finish(self, studio: &Studio, jobs: &mut Jobs) {
        jobs.preferences |= studio.audio_levels.level(self.channel) != self.initial;
    }
}

fn focus(studio: &mut Studio, channel: VolumeChannel) {
    if studio.audio_focus != Some(channel) {
        studio.audio_focus = Some(channel);
        studio.ui_dirty = true;
    }
}

pub(super) fn keyboard(key: KeyCode, shift: bool, studio: &mut Studio, jobs: &mut Jobs) -> bool {
    if !studio.audio_settings_open() {
        return false;
    }
    if key == KeyCode::Escape {
        studio.act(Action::AudioSettings(false), jobs);
        return true;
    }

    if key == KeyCode::Tab {
        let order = VolumeChannel::ALL;
        let index = studio
            .audio_focus
            .and_then(|channel| order.iter().position(|&c| c == channel));
        let next = match (index, shift) {
            (Some(index), true) => (index + order.len() - 1) % order.len(),
            (Some(index), false) => (index + 1) % order.len(),
            (None, true) => order.len() - 1,
            (None, false) => 0,
        };
        focus(studio, order[next]);
        return true;
    }

    let Some(channel) = studio.audio_focus else {
        return false;
    };
    let level = studio.audio_levels.level(channel);
    let level = match key {
        KeyCode::ArrowLeft | KeyCode::ArrowDown => level.saturating_sub(5),
        KeyCode::ArrowRight | KeyCode::ArrowUp => (level + 5).min(100),
        KeyCode::Home => 0,
        KeyCode::End => 100,
        KeyCode::Enter | KeyCode::Space
            if matches!(channel, VolumeChannel::Brush | VolumeChannel::Tools) =>
        {
            studio.act(Action::PreviewAudio(channel), jobs);
            return true;
        }
        _ => return false,
    };
    jobs.preferences |= studio.audio_levels.set(channel, level);
    true
}
