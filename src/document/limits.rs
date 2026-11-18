//! Limits shared by the saved document and its editing entry points.

pub(crate) const MAX_LAYERS: usize = 256;
pub const MAX_STROKES: usize = 100_000;
pub const MAX_STROKE_POINTS: usize = 1_000_000;
pub(crate) const MAX_LABELS: usize = 2048;
pub(crate) const MAX_LANDMARKS: usize = 2048;
pub const MAX_WORLD_COORDINATE: f32 = 1_000_000.;

pub(crate) const MAX_NAME_BYTES: usize = 1024;
pub const MAX_LABEL_TEXT_BYTES: usize = 512;
pub const MIN_LABEL_SIZE: f32 = 8.;
pub const MAX_LABEL_SIZE: f32 = 256.;
pub const MIN_LABEL_SPACING: f32 = -2.;
pub const MAX_LABEL_SPACING: f32 = 24.;
pub const MAX_LABEL_CURVATURE: f32 = 120.;
pub const MIN_LANDMARK_SIZE: f32 = 24.;
pub const MAX_LANDMARK_SIZE: f32 = 640.;

pub(crate) fn valid_name(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= MAX_NAME_BYTES
}
