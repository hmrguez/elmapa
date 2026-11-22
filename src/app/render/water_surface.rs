//! Match surface motion to the stable Water brush finish in either atlas palette.
use elmapa::brush_materials;

pub(super) fn profile(texture: Option<usize>) -> u32 {
    let Some(material) = texture.and_then(|index| brush_materials::artwork_catalog().get(index))
    else {
        return 0;
    };
    // Keep these selectors in sync with water_surface.wgsl.
    match material.id.as_str() {
        "lake" => 1,
        "oasis-water" => 2,
        "glacial-water" => 3,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elmapa::document::AtlasStyle;

    #[test]
    fn only_animated_water_finishes_select_a_profile_in_both_palettes() {
        for (expected, id) in [
            (0, "ocean"),
            (0, "shallow-ocean"),
            (1, "lake"),
            (2, "oasis-water"),
            (3, "glacial-water"),
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
