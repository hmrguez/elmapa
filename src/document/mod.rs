//! Versioned, renderer-independent map data and validation.
mod bounds;
mod demo;
pub mod limits;
mod model;
mod validation;

pub use bounds::ContentBounds;
pub use model::{AtlasStyle, Brush, Document, Layer, LinearStrokeId, Point, Stroke, TerrainReplay};
pub const DOCUMENT_VERSION: u32 = 1;

#[cfg(test)]
mod tests;

mod shared;
pub use shared::SharedVec;

mod chunks;
pub use chunks::SharedChunks;

mod label;
pub use label::{Label, LabelFont, LabelInk};

mod landmark;
pub use landmark::{Landmark, LandmarkKind};

mod regions;
pub use regions::{
    MAX_BORDER_POINTS, RegionAddress, RegionArea, RegionLayer, RegionNode, RegionParent,
    RegionPolygon, RegionStyle,
};
