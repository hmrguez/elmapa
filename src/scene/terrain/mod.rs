pub(in crate::scene) mod contours;
pub(in crate::scene) mod field;
pub(in crate::scene) mod forest;
pub(in crate::scene) mod linear;
pub(in crate::scene) mod materials;
pub(in crate::scene) mod mountains;
pub(in crate::scene) mod regions;
pub(in crate::scene) mod rivers;
pub(in crate::scene) mod roads;
mod water_ripples;

pub(super) const SEA: u32 = 0xe6e5d3;
pub(super) const LAND: u32 = 0xf4ecd5;

#[cfg(test)]
pub(super) use {contours::contours, field::region_field};

pub(in crate::scene) mod field_index;
