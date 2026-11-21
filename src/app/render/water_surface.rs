//! Match surface motion to the stable Water brush finish in either atlas palette.
use elmapa::brush_materials;

pub(super) fn profile(texture: Option<usize>) -> u32 {
    let Some(material) = texture.and_then(|index| brush_materials::artwork_catalog().get(index))
    else {
        return 0;
    };
    // Keep these selectors in sync with water_surface.wgsl.
    match material.id.as_str() {
        "ocean" => 1,
        "shallow-ocean" => 2,
        "lake" => 3,
        "oasis-water" => 4,
        "glacial-water" => 5,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elmapa::document::AtlasStyle;

    #[test]
    fn explicit_water_finishes_keep_their_own_motion_in_each_palette() {
        for (expected, id) in [
            (1, "ocean"),
            (2, "shallow-ocean"),
            (3, "lake"),
            (4, "oasis-water"),
            (5, "glacial-water"),
            (0, "alpine-meadow"),
        ] {
            let index = brush_materials::catalog()
                .iter()
                .position(|material| material.id == id)
                .unwrap();
            for style in [AtlasStyle::Classic, AtlasStyle::Woodland] {
                assert_eq!(
                    profile(Some(brush_materials::artwork_index(index, style))),
                    expected
                );
            }
        }
        assert_eq!(profile(None), 0);
        assert_eq!(profile(Some(usize::MAX)), 0);
    }
}
