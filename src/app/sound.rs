//! Local editor playback. Only accepted canvas gestures request sounds.
mod foley;

use super::state::Studio;
use bevy::prelude::*;
use elmapa::{
    brush_materials,
    document::Brush,
    library::{AudioLevels, VolumeChannel},
};
use rodio::{
    OutputStream, OutputStreamBuilder, Sink, Source,
    buffer::SamplesBuffer,
    mixer::{Mixer, mixer},
    source::Zero,
};
use std::{borrow::Cow, collections::HashMap, fs::File, io::BufReader, path::Path, time::Duration};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum Effect {
    Brush(Brush, Option<usize>),
    Select,
    Text,
    Landmark,
    Regions,
    Pan,
}

impl Effect {
    fn id(self) -> Cow<'static, str> {
        match self {
            Self::Brush(brush, index) => {
                if let Some(material) =
                    index.and_then(|index| brush_materials::catalog().get(index))
                {
                    if brush == material.brush {
                        return material.id.as_str().into();
                    }
                    if brush == Brush::River && material.brush == Brush::Water {
                        return format!("river-{}", material.id).into();
                    }
                }
                match brush {
                    Brush::Land => "parchment",
                    Brush::Water => "ocean",
                    Brush::Forest => "mixed-woodland",
                    Brush::Mountain => "engraved-peaks",
                    Brush::River => "river",
                    Brush::Path => "trail",
                    Brush::Erase => "eraser",
                }
                .into()
            }
            Self::Select => "select".into(),
            Self::Text => "text".into(),
            Self::Landmark => "landmark".into(),
            Self::Regions => "regions".into(),
            Self::Pan => "pan".into(),
        }
    }

    fn is_brush(self) -> bool {
        matches!(self, Self::Brush(..))
    }
}

#[derive(Default)]
pub(super) struct Gestures {
    pub brush: Option<Effect>,
    pub moved: bool,
    pub cue: Option<Effect>,
    pub preview: Option<Effect>,
}

#[derive(Default)]
pub(super) struct Playback {
    output: Option<Output>,
    retry_after: Duration,
}

struct SoundBus {
    mixer: Mixer,
    sink: Sink,
}

impl SoundBus {
    fn new(stream: &OutputStream) -> Self {
        let channels = stream.config().channel_count();
        let sample_rate = stream.config().sample_rate();
        let (mixer, source) = mixer(channels, sample_rate);
        // Keep the bus connected during silence between gestures.
        mixer.add(Zero::new(channels, sample_rate));
        let sink = Sink::connect_new(stream.mixer());
        sink.append(source);
        Self { mixer, sink }
    }
}

fn gain(levels: AudioLevels, channel: VolumeChannel) -> f32 {
    (f32::from(levels.level(VolumeChannel::Master)) / 100.)
        * (f32::from(levels.level(channel)) / 100.)
}

struct Output {
    _stream: OutputStream,
    brush_bus: SoundBus,
    tool_bus: SoundBus,
    levels: Option<AudioLevels>,
    previewing: bool,
    clips: HashMap<Effect, SamplesBuffer>,
    brush: Option<BrushVoice>,
    cue: Option<Sink>,
}

struct BrushVoice {
    sink: Sink,
    contact_until: Duration,
    moving_until: Duration,
    gain: f32,
}

impl BrushVoice {
    fn update(&mut self, time: &Time<Real>, held: bool, moved: bool) -> bool {
        let now = time.elapsed();
        if held && moved {
            self.moving_until = now + Duration::from_millis(110);
        }
        let audible = now < self.contact_until || held && now < self.moving_until;
        let gain_change = time.delta_secs() / if audible { 0.045 } else { 0.075 };
        self.gain = if audible {
            (self.gain + gain_change).min(1.)
        } else {
            (self.gain - gain_change).max(0.)
        };
        self.sink.set_volume(0.42 * self.gain);
        if self.gain > 0. || audible {
            self.sink.play();
        } else {
            self.sink.pause();
        }
        held || self.gain > 0. || audible
    }
}

impl Output {
    fn open() -> Result<Self, rodio::StreamError> {
        let mut stream = OutputStreamBuilder::open_default_stream()?;
        stream.log_on_drop(false);
        Ok(Self {
            brush_bus: SoundBus::new(&stream),
            tool_bus: SoundBus::new(&stream),
            _stream: stream,
            levels: None,
            previewing: false,
            clips: HashMap::new(),
            brush: None,
            cue: None,
        })
    }

    fn set_levels(&mut self, levels: AudioLevels) {
        if self.levels != Some(levels) {
            self.brush_bus
                .sink
                .set_volume(gain(levels, VolumeChannel::Brush));
            self.tool_bus
                .sink
                .set_volume(gain(levels, VolumeChannel::Tools));
            self.levels = Some(levels);
        }
    }

    fn clip(&mut self, effect: Effect, root: &Path) -> SamplesBuffer {
        self.clips
            .entry(effect)
            .or_insert_with(|| {
                let id = effect.id();
                if let Some(clip) =
                    local_clip(&root.join("sound-effects").join(format!("{id}.wav")))
                {
                    debug!("Loaded local editor sound: {id}");
                    clip
                } else {
                    foley::clip(&id, effect.is_brush())
                }
            })
            .clone()
    }

    fn start_brush(&mut self, effect: Effect, root: &Path, now: Duration) {
        let clip = self.clip(effect, root);
        let sink = Sink::connect_new(&self.brush_bus.mixer);
        sink.set_volume(0.);
        sink.append(clip.repeat_infinite());
        self.brush = Some(BrushVoice {
            sink,
            contact_until: now + Duration::from_millis(155),
            moving_until: now,
            gain: 0.,
        });
    }

    fn play_cue(&mut self, effect: Effect, root: &Path) {
        let clip = self.clip(effect, root);
        let bus = if effect.is_brush() {
            &self.brush_bus
        } else {
            &self.tool_bus
        };
        let sink = Sink::connect_new(&bus.mixer);
        sink.set_volume(0.38);
        sink.append(clip);
        self.cue = Some(sink);
    }
}

fn local_clip(path: &Path) -> Option<SamplesBuffer> {
    let file = File::open(path).ok()?;
    if file.metadata().ok()?.len() > 2 * 1024 * 1024 {
        return None;
    }
    let mut reader = hound::WavReader::new(BufReader::new(file)).ok()?;
    let spec = reader.spec();
    if spec.sample_format != hound::SampleFormat::Int
        || spec.bits_per_sample != 16
        || !(1..=2).contains(&spec.channels)
        || !(8_000..=96_000).contains(&spec.sample_rate)
        || !(1..=5 * spec.sample_rate).contains(&reader.duration())
    {
        return None;
    }
    let samples = reader
        .samples::<i16>()
        .map(|sample| sample.map(|value| f32::from(value) / 32768.))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    Some(SamplesBuffer::new(spec.channels, spec.sample_rate, samples))
}

pub(super) fn play(
    mut studio: ResMut<Studio>,
    mut playback: NonSendMut<Playback>,
    time: Res<Time<Real>>,
) {
    let Gestures {
        brush,
        moved,
        cue,
        preview,
    } = std::mem::take(&mut studio.sound);
    let previewing = studio.audio_settings_open();
    if studio.sound_effects_disabled
        || (studio.home && !previewing)
        || studio.installing
        || studio.replay_view.is_some()
        || studio.quit_requested
    {
        if let Some(output) = playback.output.as_mut() {
            output.brush = None;
            output.cue = None;
        }
        return;
    }
    let brush = if previewing { None } else { brush };
    let cue = if previewing { preview } else { cue };
    let now = time.elapsed();
    if playback.output.is_none()
        && (brush.is_some() || cue.is_some())
        && now >= playback.retry_after
    {
        match Output::open() {
            Ok(output) => playback.output = Some(output),
            Err(error) => {
                playback.retry_after = now + Duration::from_secs(10);
                debug!("Editor sound output unavailable: {error}");
            }
        }
    }
    let (Some(output), Some(library)) = (playback.output.as_mut(), studio.library.as_ref()) else {
        return;
    };
    if output.previewing != previewing {
        output.brush = None;
        output.cue = None;
        output.previewing = previewing;
    }
    output.set_levels(studio.audio_levels);
    let root = library.root_path();
    if let Some(effect) = brush {
        output.start_brush(effect, root, now);
    }
    output.cue.take_if(|cue| cue.empty());
    if let Some(effect) = cue {
        output.play_cue(effect, root);
    }
    output.brush.take_if(|voice| {
        !voice.update(
            &time,
            studio.stroke.is_some(),
            studio.pointer.is_some() && moved,
        )
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn master_and_channel_levels_combine_without_affecting_the_other_channel() {
        let mut levels = AudioLevels::default();
        levels.set(VolumeChannel::Master, 50);
        levels.set(VolumeChannel::Brush, 20);
        assert_eq!(gain(levels, VolumeChannel::Brush), 0.1);
        assert_eq!(gain(levels, VolumeChannel::Tools), 0.5);
        levels.set(VolumeChannel::Tools, 0);
        assert_eq!(gain(levels, VolumeChannel::Tools), 0.);
    }

    #[test]
    fn every_finish_and_tool_has_its_own_sound() {
        let mut ids = HashSet::new();
        let mut add = |effect: Effect| {
            let id = effect.id();
            assert!(foley::profile(&id).is_some(), "missing sound: {id}");
            assert!(ids.insert(id), "duplicated effect: {effect:?}");
        };
        for (index, material) in brush_materials::catalog().iter().enumerate() {
            add(Effect::Brush(material.brush, Some(index)));
            if material.brush == Brush::Water {
                add(Effect::Brush(Brush::River, Some(index)));
            }
        }
        for brush in [Brush::River, Brush::Path, Brush::Erase] {
            add(Effect::Brush(brush, None));
        }
        for cue in [
            Effect::Select,
            Effect::Text,
            Effect::Landmark,
            Effect::Regions,
            Effect::Pan,
        ] {
            add(cue);
        }
        for brush in [Brush::Land, Brush::Water, Brush::Forest, Brush::Mountain] {
            let (default, _) = brush_materials::for_brush(brush).next().unwrap();
            assert_eq!(
                Effect::Brush(brush, None).id(),
                Effect::Brush(brush, Some(default)).id()
            );
        }
    }

    #[test]
    fn a_canvas_press_uses_the_active_material_and_only_moves_when_paint_extends() {
        use crate::app::state::Action;
        let mut studio = Studio {
            home: false,
            ..default()
        };
        let mut jobs = crate::app::files::Jobs::default();
        let (index, _) = brush_materials::for_brush(Brush::Land).nth(1).unwrap();
        studio.open_brush_orbit(Vec2::ZERO);
        studio.act(Action::BrushOrbitEnter(0, 0), &mut jobs);
        studio.act(Action::BrushMaterial(Some(index)), &mut jobs);
        let start = studio.bounds.size / 2.;
        studio.down(start);
        assert_eq!(
            studio.sound.brush,
            Some(Effect::Brush(Brush::Land, Some(index)))
        );
        studio.moved(start + Vec2::new(18., 0.), true);
        assert!(!studio.sound.moved);
        studio.moved(start + Vec2::new(36., 0.), false);
        assert!(studio.sound.moved);
        studio.up(start + Vec2::new(36., 0.), false);
        assert!(studio.stroke.is_none());
    }
}
