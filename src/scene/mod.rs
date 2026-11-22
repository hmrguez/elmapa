//! Deterministic cartographic geometry shared by the native canvas and export.
mod builder;
mod cache;
mod forest_animation;
mod geometry;
pub mod granulation;
mod mountain_animation;
mod noise;
mod palette;
mod paper;
mod path_animation;
mod render;
mod river_animation;
mod shape;
mod svg;
mod terrain;
pub mod territories;

pub use builder::build_scene;
pub use cache::{Scene, SceneCache, TerrainHit, WetEdge, region_stroke_covers};
pub use forest_animation::ForestAnimation;
pub use mountain_animation::{MountainAnimation, MountainId, MountainPart};
pub use paper::paper_detail;
pub use path_animation::{PathAnimation, PathId, PathPart};
pub use render::SceneRenderer;
pub use river_animation::{RiverAnimation, RiverFlow};
pub use shape::Shape;
pub const PARCHMENT_COLOR: u32 = terrain::LAND;
pub use svg::{to_svg, write_svg};
pub use terrain::forest::{
    FOREST_SPECIMEN_COUNT, ForestTree, TreeId, forest_specimen, forest_specimen_svg,
    forest_specimen_tree, forest_swatch,
};
pub use terrain::palette_transition::TerrainPalette;
pub use terrain::rivers::{CURRENT_INK, RiverReach};
pub use terrain::shore_wash::ShoreWash;

#[cfg(test)]
mod tests;

pub mod thumbnail;

pub mod work;

pub mod generation;

mod labels;
pub use labels::{label_caret, label_font_bytes, label_shapes};

mod landmarks;
pub use landmarks::{
    LandmarkBatch, LandmarkDefinition, LandmarkMotion, LandmarkPart, LandmarkStep,
    landmark_batches, landmark_batches_in, landmark_bounds, landmark_definition, landmark_pose,
    landmark_registry, landmark_svg,
};

mod landmark_packs;
pub use landmark_packs::{LandmarkCatalog, LandmarkPack, landmark_catalog};
