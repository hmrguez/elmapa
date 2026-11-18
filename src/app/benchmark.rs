//! Frame-loop cadence is recorded separately from native drawable presentation.
use bevy::prelude::*;
use std::time::Instant;
#[derive(Resource)]
pub struct Benchmark {
    start: Instant,
    previous: Instant,
}
impl Default for Benchmark {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            previous: Instant::now(),
        }
    }
}
pub fn frame_start(mut benchmark: ResMut<Benchmark>) {
    let now = Instant::now();
    elmapa::performance::duration(
        "bevy_frame_interval",
        now.duration_since(benchmark.previous),
    );
    benchmark.start = now;
    benchmark.previous = now;
}
pub fn frame_end(benchmark: Res<Benchmark>) {
    elmapa::performance::duration("bevy_main_schedule", benchmark.start.elapsed());
}

/// Optional native diagnostic artifact, separate from the bounded timing trace.
pub fn log_layer(_: &mut App) -> Option<bevy::log::BoxedLayer> {
    let path = std::env::var_os("ELMAPA_LOG")?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()?;
    Some(Box::new(
        bevy::log::tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(file)),
    ))
}
