//! Pigment substitutions for linework and annotations whose geometry is shared.
use super::Shape;
use crate::document::AtlasStyle;

pub(super) const UMBER: u32 = 0x5d4b3b;
pub(super) const CURRENT: u32 = 0x5e7977;
pub(super) const CHANNEL: u32 = 0x9cb3aa;
pub(super) const CHANNEL_LIGHT: u32 = 0xb9cac0;
pub(super) const SHORE: u32 = 0x647d73;

pub(super) fn recolor(shapes: &mut [Shape], palette: impl Fn(u32) -> u32) {
    for shape in shapes {
        let ink = match shape {
            Shape::Polygon { fill, .. } | Shape::RegionSurface { fill, .. } => fill,
            Shape::Line { color, .. } => color,
            Shape::Sprite { .. } | Shape::ShoreWash(_) => continue,
        };
        *ink = palette(*ink);
    }
}

pub(super) fn landmark(color: u32) -> u32 {
    match color {
        0x655640 | 0x514332 | 0x574733 | 0x5a4933 => UMBER,
        0x877457 | 0x887455 => 0x806a55,
        0xa7966d => 0xb8a47d,
        0xd5c5a2 => 0xd1bf9d,
        0xecdfc1 | 0xe9dcbc => 0xe7dabe,
        _ => color,
    }
}

pub(super) fn trail(color: u32) -> u32 {
    match color {
        0xd4bc8c => 0xbea17c,
        0xb19a73 => 0xb8a47d,
        0x8b7351 => 0x82603e,
        0xa68a60 => 0x9d7951,
        0x796044 => 0x705338,
        0xebd8ad => 0xd4ba94,
        0xf2e3c2 => 0xead9b7,
        _ => color,
    }
}

pub(super) fn river(color: u32) -> u32 {
    match color {
        0xc2af85 => 0xb8a47d,
        0x75654f => SHORE,
        0xa89a78 => 0x879f94,
        super::terrain::rivers::CURRENT_INK => CURRENT,
        0xd2d4bf => CHANNEL,
        0xe6e7d2 => CHANNEL_LIGHT,
        _ => color,
    }
}

pub(super) fn land(style: AtlasStyle) -> u32 {
    match style {
        AtlasStyle::Classic => super::terrain::LAND,
        AtlasStyle::Woodland => 0xf3e8cd,
    }
}

pub(super) fn water(style: AtlasStyle) -> u32 {
    match style {
        AtlasStyle::Classic => super::terrain::SEA,
        AtlasStyle::Woodland => 0xb2c1b9,
    }
}
