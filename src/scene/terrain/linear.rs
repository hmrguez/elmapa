use super::super::Shape;
use crate::document::{Brush, Layer, Stroke};

#[cfg(test)]
pub(crate) fn linear_stroke(out: &mut Vec<Shape>, layer: &Layer, index: usize, stroke: &Stroke) {
    out.extend(linear_chunks(layer, index, stroke).into_iter().flatten());
}
pub(in crate::scene) fn linear_chunks(
    layer: &Layer,
    index: usize,
    stroke: &Stroke,
) -> Vec<Vec<Shape>> {
    match stroke.brush {
        Brush::River => super::rivers::river_batches(layer, index, stroke)
            .into_iter()
            .map(|b| b.shapes)
            .collect(),
        Brush::Path => super::roads::road_chunks(layer, index, stroke),
        _ => Vec::new(),
    }
}
