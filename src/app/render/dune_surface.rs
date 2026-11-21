use elmapa::brush_materials::artwork_catalog;

pub(super) fn profile(texture: Option<usize>) -> Option<u32> {
    // Keep the profile number in sync with terrain_tile.wgsl.
    (artwork_catalog().get(texture?)?.id == "dune-sand").then_some(103)
}
