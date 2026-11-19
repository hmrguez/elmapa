use super::super::Shape;
use crate::document::{AtlasStyle, Brush, Layer, Stroke};

pub(in crate::scene) fn linear_chunks(
    layer: &Layer,
    index: usize,
    stroke: &Stroke,
    style: AtlasStyle,
) -> Vec<Vec<Shape>> {
    match stroke.brush {
        Brush::River => super::rivers::river_batches(layer, index, stroke, style)
            .into_iter()
            .map(|b| b.shapes)
            .collect(),
        Brush::Path => {
            let mut chunks = super::roads::road_chunks(layer, index, stroke);
            if style == AtlasStyle::Woodland {
                use crate::scene::palette;
                chunks
                    .iter_mut()
                    .for_each(|chunk| palette::recolor(chunk, palette::trail));
            }
            chunks
        }
        _ => Vec::new(),
    }
}
