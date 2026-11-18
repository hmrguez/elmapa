//! Canvas overlays and their frame-by-frame positioning.
mod landmark;
mod orbit;
mod pointer;
mod regions;
mod text;

use super::widgets::Builder;
use crate::app::state::Studio;
use bevy::prelude::*;

pub use landmark::landmark_selection_follow;
pub use orbit::radial_follow;
pub use pointer::{landmark_cursor, tooltips};
pub use regions::region_selection_follow;
pub use text::{TextCard, text_ime, text_selection_follow};

pub(super) use pointer::{Cursor, build_feedback, update_brush_cursor};

pub(super) fn build_selection(
    b: &mut Builder,
    root: Entity,
    studio: &Studio,
    landmark_scroll: f32,
) {
    text::build(b, root, studio);
    landmark::build(b, root, studio);
    regions::build(b, root, studio);
    pointer::brush_orbit(b, root, studio);
    landmark::picker(b, root, studio, landmark_scroll);
    text::placement_style(b, root, studio);
}
