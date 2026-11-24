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
    grains: f32,
    resonance: f32,
    decay: f32,
}

impl Profile {
    const fn new(low: f32, high: f32, grains: f32, resonance: f32, decay: f32) -> Self {
        Self {
            low,
            high,
            grains,
            resonance,
            decay,
        }
    }
}

pub(super) fn profile(id: &str) -> Option<Profile> {
    // Passband, accents per second, accent pitch and accent decay in seconds.
    let profile = match id {
        "parchment" => Profile::new(160., 1850., 54., 380., 0.009),
        "ochre-sands" => Profile::new(240., 2750., 65., 930., 0.006),
        "lichen-heath" => Profile::new(100., 1250., 42., 330., 0.012),
        "weathered-shale" => Profile::new(190., 2300., 39., 1200., 0.011),
        "dune-sand" => Profile::new(430., 3900., 85., 1550., 0.004),
        "alpine-meadow" => Profile::new(210., 2200., 45., 670., 0.008),
        "ocean" => Profile::new(65., 1250., 19., 240., 0.029),
        "shallow-ocean" => Profile::new(170., 2500., 34., 490., 0.021),
        "lake" => Profile::new(95., 1650., 21., 315., 0.027),
        "oasis-water" => Profile::new(130., 1850., 26., 400., 0.025),
        "glacial-water" => Profile::new(180., 2650., 31., 575., 0.022),
        "mixed-woodland" => Profile::new(320., 3000., 33., 640., 0.015),
        "fir-grove" => Profile::new(410., 3550., 49., 1000., 0.009),
        "oak-grove" => Profile::new(240., 2600., 27., 590., 0.017),
        "desert-scrub" => Profile::new(520., 3900., 43., 1280., 0.011),
        "engraved-peaks" => Profile::new(150., 1800., 28., 350., 0.022),
        "alpine-crags" => Profile::new(260., 2400., 33., 910., 0.017),
        "ochre-escarpments" => Profile::new(180., 2800., 44., 510., 0.012),
        "weathered-highlands" => Profile::new(120., 1550., 23., 290., 0.025),
        "river" => Profile::new(115., 1750., 42., 450., 0.025),
        "river-ocean" => Profile::new(80., 1500., 37., 330., 0.028),
        "river-shallow-ocean" => Profile::new(245., 2550., 47., 580., 0.020),
        "river-lake" => Profile::new(110., 1700., 35., 380., 0.025),
        "river-oasis-water" => Profile::new(150., 2050., 40., 500., 0.023),
        "river-glacial-water" => Profile::new(200., 2850., 54., 640., 0.018),
        "trail" => Profile::new(310., 3400., 50., 720., 0.007),
        "eraser" => Profile::new(210., 2950., 55., 360., 0.006),
        "select" => Profile::new(130., 1500., 0., 410., 0.018),
        "text" => Profile::new(580., 4400., 77., 850., 0.004),
        "landmark" => Profile::new(120., 2150., 0., 280., 0.027),
        "regions" => Profile::new(350., 2900., 64., 620., 0.007),
        "pan" => Profile::new(250., 2800., 25., 570., 0.013),
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
    let seconds = if brush { 1.8 } else { 0.38 };
    let band_high = 1. - (-TAU * profile.high / rate).exp();
    let band_low = 1. - (-TAU * profile.low / rate).exp();
    let grain_decay = (-1. / (profile.decay * rate)).exp();
    let tap = profile.grains == 0.;
    let mut high = 0.;
    let mut low = 0.;
    let mut grain = 0.;
    let mut phase = 0.;
    let mut pitch = 0.;
    let mut flutter = 0.;
    let sample_count = (seconds * rate) as usize;
    let mut samples = Vec::with_capacity(sample_count);
    for i in 0..sample_count {
        let t = i as f32 / rate;
        let noise = rng.random_range(-1.0..=1.0);
        high += (noise - high) * band_high;
        low += (high - low) * band_low;
        flutter += (noise - flutter) * 0.005;
        if tap && i == 0 || rng.random::<f32>() < profile.grains / rate {
            phase = 0.;
            grain = rng.random_range(0.45..=1.);
            pitch = TAU * profile.resonance * rng.random_range(0.7..=1.35) / rate;
        }
        let texture = (high - low) * (0.48 + 2.8 * flutter).clamp(0.15, 0.85);
        let accent = grain * (0.11 * noise + 0.15 * f32::sin(phase));
        let envelope = if brush {
            0.86 + 0.14 * (t * TAU * 1.13).sin()
        } else if tap {
            (1. - (-t / 0.002).exp()) * (-t / 0.032).exp()
        } else {
            (t / 0.035).min(1.) * ((seconds - t) / 0.23).clamp(0., 1.).powi(2)
        };
        samples.push((texture + accent) * envelope);
        grain *= grain_decay;
        phase += pitch;
    }
    if brush {
        let overlap = RATE as usize / 12;
        let tail = samples.len() - overlap;
        for i in 0..overlap {
            let mix = i as f32 / overlap as f32;
            samples[i] = samples[tail + i] * (1. - mix) + samples[i] * mix;
        }
        samples.truncate(tail);
    }
    SamplesBuffer::new(1, RATE, samples)
}
