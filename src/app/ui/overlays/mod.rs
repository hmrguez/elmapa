//! Canvas overlays and their frame-by-frame positioning.
mod landmark;
mod orbit;
mod path;
mod pointer;
mod regions;
mod selection;
mod text;

use super::components::Builder;
use crate::app::state::Studio;
use bevy::prelude::*;

pub use landmark::landmark_selection_follow;
pub use orbit::radial_follow;
pub use path::path_selection_follow;
pub use pointer::{landmark_cursor, tooltips};
pub use regions::region_selection_follow;
pub use selection::selection_follow;
pub use text::{TextCard, text_ime, text_selection_follow};

pub(super) use pointer::{Cursor, build_feedback, update_brush_cursor};

pub(super) fn build_selection(b: &mut Builder, root: Entity, studio: &Studio) {
    selection::build(b, root, studio);
    path::build(b, root, studio);
    text::build(b, root, studio);
    landmark::build(b, root, studio);
    regions::build(b, root, studio);
    b.with_paper(|b| {
        pointer::brush_orbit(b, root, studio);
        text::placement_style(b, root, studio);
    });
}
