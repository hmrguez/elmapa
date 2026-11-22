use elmapa::brush_materials::artwork_catalog;

pub(super) fn profile(texture: Option<usize>) -> Option<u32> {
    // Keep the profile numbers in sync with terrain_tile.wgsl.
    match artwork_catalog().get(texture?)?.id.as_str() {
        "lichen-heath" => Some(102),
        _ => None,
    }
}
