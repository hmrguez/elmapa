//! Built-in foley for installations without local recordings.
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rodio::buffer::SamplesBuffer;
use std::f32::consts::TAU;

const RATE: u32 = 24_000;

#[derive(Clone, Copy)]
pub(super) struct Profile {
    low: f32,
    high: f32,
    accents_per_second: f32,
    accent_gain: f32,
    resonance: f32,
    decay: f32,
    stone: bool,
}

impl Profile {
    const fn new(
        low: f32,
        high: f32,
        accents_per_second: f32,
        resonance: f32,
        decay: f32,
        accent_gain: f32,
    ) -> Self {
        Self {
            low,
            high,
            accents_per_second,
            accent_gain,
            resonance,
            decay,
            stone: false,
        }
    }

    const fn stone(self) -> Self {
        Self {
            stone: true,
            ..self
        }
    }
}

pub(super) fn profile(id: &str) -> Option<Profile> {
    // Air band, tap rate, pitch, decay in seconds and tap level.
    let profile = match id {
        "parchment" => Profile::new(115., 890., 3.8, 208., 0.026, 0.12),
        "ochre-sands" => Profile::new(190., 1210., 5.1, 246., 0.019, 0.12),
        "lichen-heath" => Profile::new(95., 680., 3.1, 185., 0.031, 0.10),
        "weathered-shale" => Profile::new(165., 1090., 4.6, 277., 0.022, 0.15),
        "dune-sand" => Profile::new(250., 1510., 4.3, 311., 0.018, 0.09),
        "alpine-meadow" => Profile::new(175., 1040., 4.1, 233., 0.027, 0.12),
        "ocean" => Profile::new(65., 550., 2.9, 277., 0.049, 0.17),
        "shallow-ocean" => Profile::new(140., 960., 3.8, 392., 0.036, 0.16),
        "lake" => Profile::new(95., 620., 3.1, 311., 0.043, 0.17),
        "oasis-water" => Profile::new(125., 750., 3.4, 349., 0.042, 0.16),
        "glacial-water" => Profile::new(165., 990., 4.2, 440., 0.031, 0.16),
        "mixed-woodland" => Profile::new(210., 1230., 4.3, 247., 0.023, 0.13),
        "fir-grove" => Profile::new(270., 1460., 5.3, 294., 0.017, 0.12),
        "oak-grove" => Profile::new(190., 1000., 3.7, 220., 0.028, 0.13),
        "desert-scrub" => Profile::new(330., 1590., 4.9, 330., 0.017, 0.12),
        "engraved-peaks" => Profile::new(110., 760., 3.4, 155., 0.034, 0.18).stone(),
        "alpine-crags" => Profile::new(160., 1040., 4.1, 233., 0.030, 0.18).stone(),
        "ochre-escarpments" => Profile::new(145., 970., 4.6, 196., 0.023, 0.17).stone(),
        "weathered-highlands" => Profile::new(90., 650., 3.0, 139., 0.037, 0.17).stone(),
        "river" => Profile::new(90., 720., 5.2, 349., 0.034, 0.15),
        "river-ocean" => Profile::new(65., 580., 4.5, 294., 0.041, 0.17),
        "river-shallow-ocean" => Profile::new(165., 1030., 5.6, 415., 0.029, 0.15),
        "river-lake" => Profile::new(90., 670., 4.8, 330., 0.038, 0.16),
        "river-oasis-water" => Profile::new(115., 830., 5.3, 370., 0.035, 0.15),
        "river-glacial-water" => Profile::new(155., 1100., 6.2, 466., 0.027, 0.15),
        "trail" => Profile::new(200., 1170., 5.0, 261., 0.021, 0.14),
        "eraser" => Profile::new(155., 1000., 3.0, 185., 0.018, 0.10),
        "select" => Profile::new(130., 700., 0., 349., 0.023, 0.19),
        "text" => Profile::new(310., 1420., 5.0, 392., 0.021, 0.16),
        "landmark" => Profile::new(115., 775., 0., 220., 0.040, 0.24),
        "regions" => Profile::new(205., 1080., 4.3, 294., 0.027, 0.17),
        "pan" => Profile::new(170., 1040., 3.7, 196., 0.028, 0.09),
        _ => return None,
    };
    Some(profile)
}

pub(super) fn clip(id: &str, brush: bool) -> SamplesBuffer {
    let profile = profile(id).unwrap_or_else(|| profile("parchment").unwrap());
    let seed = id.bytes().fold(0u64, |seed, byte| {
        seed.wrapping_mul(131).wrapping_add(u64::from(byte))
    });
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let rate = RATE as f32;
    let seconds = if brush { 1.8 } else { 0.34 };
    let band_high = 1. - (-TAU * profile.high / rate).exp();
    let band_low = 1. - (-TAU * profile.low / rate).exp();
    let decay = (-1. / (profile.decay * rate)).exp();
    let tap = profile.accents_per_second == 0.;
    let mut high = 0.;
    let mut low = 0.;
    let mut flutter = 0.;
    let mut phase = 0.;
    let mut pitch = 0.;
    let mut level = 0.;
    let mut accent_start = 0;
    let mut next_accent = (rate * if brush { 0.048 } else { 0.015 }) as usize;
    let sample_count = (seconds * rate) as usize;
    let mut samples = Vec::with_capacity(sample_count);
    for i in 0..sample_count {
        let t = i as f32 / rate;
        let noise = rng.random_range(-1.0..=1.0);
        high += (noise - high) * band_high;
        low += (high - low) * band_low;
        flutter += (noise - flutter) * 0.003;

        // Space the resonances apart and round both edges of each tap.
        if i == next_accent {
            accent_start = i;
            next_accent = if tap {
                usize::MAX
            } else {
                i + (rate / profile.accents_per_second * rng.random_range(0.72..=1.33)) as usize
            };
            phase = 0.;
            level = profile.accent_gain * rng.random_range(0.76..=1.0);
            pitch = TAU * profile.resonance * rng.random_range(0.95..=1.05) / rate;
        }
        let age = (i - accent_start) as f32 / rate;
        let attack = (age / if profile.stone { 0.004 } else { 0.010 }).min(1.);
        let release = ((next_accent - i) as f32 / (rate * 0.018)).min(1.);
        let resonance = if profile.stone {
            stone_resonance(phase, pitch * rate, age)
        } else {
            f32::sin(phase) + 0.22 * f32::sin(phase * 2.)
        };
        let accent = level * attack * release * resonance;
        let air = (high - low) * (0.29 + 2.3 * flutter).clamp(0.12, 0.47);
        let envelope = if brush {
            0.79 + 0.21 * (t * TAU * 1.13).cos()
        } else if tap {
            ((seconds - t) / 0.12).min(1.)
        } else {
            (t / 0.036).min(1.) * ((seconds - t) / 0.23).min(1.).powi(2)
        };
        let air = if tap {
            air * (-t / 0.029).exp() * 0.4
        } else {
            air
        };
        samples.push((air + accent) * envelope);
        level *= decay;
        phase += pitch;
    }
    if brush {
        let overlap = RATE as usize / 12;
        let tail = samples.len() - overlap;
        for i in 0..overlap {
            let phase = std::f32::consts::PI * i as f32 / overlap as f32;
            let mix = (1. - phase.cos()) * 0.5;
            samples[i] = samples[tail + i] * (1. - mix) + samples[i] * mix;
        }
        samples.truncate(tail);
    }
    SamplesBuffer::new(1, RATE, samples)
}

/// Two softly damped stone impacts with short, inharmonic overtones.
fn stone_resonance(phase: f32, pitch: f32, age: f32) -> f32 {
    let bright = (1. - age / 0.027).max(0.).powi(2);
    let first = phase.sin() + bright * (0.48 * (phase * 2.73).sin() + 0.28 * (phase * 4.61).sin());

    let rebound_age = (age - 0.021).max(0.);
    let rebound = (rebound_age / 0.004).min(1.) * (1. - rebound_age / 0.044).max(0.).powi(2);
    let phase = rebound_age * pitch * 1.21;
    first + 0.67 * rebound * (phase.sin() + 0.44 * (phase * 2.38).sin())
}
