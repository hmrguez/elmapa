//! Stream the embedded soundtrack one track at a time.
use bevy::log::warn;
use rodio::{Decoder, Sink, mixer::Mixer};
use std::io::Cursor;

const TRACKS: &[&[u8]] = &[
    include_bytes!("../../../assets/music/secret-of-the-forest.ogg"),
    include_bytes!("../../../assets/music/field-of-fireflies.ogg"),
    include_bytes!("../../../assets/music/calm-waters.ogg"),
];

pub(super) struct Playlist {
    sink: Sink,
    next: Option<usize>,
    attenuation: f32,
}

impl Playlist {
    pub(super) fn new(mixer: &Mixer) -> Self {
        let sink = Sink::connect_new(mixer);
        sink.pause();
        Self {
            sink,
            next: Some(0),
            attenuation: 0.,
        }
    }

    pub(super) fn set_mix(&mut self, volume: f32, brush_level: f32, seconds: f32) {
        const MAX_ATTENUATION: f32 = 0.65;
        const ATTACK: f32 = 0.060;
        const RELEASE: f32 = 0.420;

        let target = MAX_ATTENUATION * brush_level;
        let change = (target - self.attenuation).clamp(
            -MAX_ATTENUATION * seconds / RELEASE,
            MAX_ATTENUATION * seconds / ATTACK,
        );
        self.attenuation += change;
        self.sink.set_volume(volume * (1. - self.attenuation));
    }

    pub(super) fn pause(&self) {
        self.sink.pause();
    }

    pub(super) fn play(&mut self) {
        // Queue the following track before the current one ends.
        if self.sink.len() < 2
            && let Some(index) = self.next
        {
            match Decoder::new_vorbis(Cursor::new(TRACKS[index])) {
                Ok(track) => {
                    self.sink.append(track);
                    self.next = Some((index + 1) % TRACKS.len());
                }
                Err(error) => {
                    warn!("Could not decode soundtrack track {}: {error}", index + 1);
                    self.next = None;
                }
            }
        }
        self.sink.play();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::{Source, mixer::mixer};

    #[test]
    fn every_bundled_track_decodes_as_audible_stereo() {
        for bytes in TRACKS {
            let mut track = Decoder::new_vorbis(Cursor::new(*bytes)).unwrap();
            assert_eq!(track.channels(), 2);
            let sample_count = (track.sample_rate() * 2 * 10) as usize;
            assert!(track.by_ref().take(sample_count).any(|s| s.abs() > 0.01));
        }
    }

    #[test]
    fn queue_refills_in_order_and_wraps_after_the_last_track() {
        let (mixer, mut output) = mixer(2, 44_100);
        let mut playlist = Playlist::new(&mixer);
        playlist.play();
        assert_eq!(playlist.next, Some(1));
        for next in [2, 0, 1] {
            playlist.play();
            assert_eq!(playlist.next, Some(next));
            assert_eq!(playlist.sink.len(), 2);

            // Advance the mixer past the skip without opening an audio device.
            playlist.sink.skip_one();
            for _ in 0..1_000 {
                output.next().unwrap();
            }
            assert_eq!(playlist.sink.len(), 1);
        }
    }

    #[test]
    fn music_dips_for_audible_brushes_and_recovers_afterward() {
        let (mixer, _output) = mixer(2, 44_100);
        let mut playlist = Playlist::new(&mixer);
        playlist.set_mix(0.25, 1., 0.060);
        assert!((playlist.sink.volume() - 0.0875).abs() < 0.0001);

        playlist.set_mix(0.25, 0., 0.060);
        assert!(playlist.sink.volume() > 0.0875);
        assert!(playlist.sink.volume() < 0.25);

        playlist.set_mix(0.25, 0., 0.420);
        assert_eq!(playlist.sink.volume(), 0.25);
        playlist.set_mix(0., 1., 0.060);
        assert_eq!(playlist.sink.volume(), 0.);
    }
}
