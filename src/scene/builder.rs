use super::Shape;
#[cfg(test)]
use super::terrain::{
    forest, linear_stroke,
    mountains::{mountain_base, mountain_chunks},
};
use crate::document::Document;
#[cfg(test)]
use crate::document::{Brush, Layer};

pub fn build_scene(doc: &Document) -> Vec<Shape> {
    super::SceneCache::default()
        .render(doc, None)
        .shapes()
        .cloned()
        .collect()
}

#[cfg(test)]
pub(super) fn build_layer(layer: &Layer) -> Vec<Shape> {
    let mut out = Vec::new();
    if layer.visible && layer.opacity > 0.0 {
        out.extend(super::cache::build_terrain(layer));
        out.extend(build_lines(layer, Brush::River));
        let peaks = layer
            .strokes
            .iter()
            .enumerate()
            .filter(|(_, stroke)| stroke.brush == Brush::Mountain)
            .flat_map(|(index, stroke)| {
                super::work::update(|work| work.line_strokes += 1);
                mountain_chunks(layer, index, stroke)
            })
            .filter_map(|flanks| Some((mountain_base(&flanks[0])?.y, flanks)));
        let trees = forest::trees(layer)
            .into_iter()
            .map(|tree| (tree.center.y, forest::tree_shapes(&tree)));
        out.extend(relief_order(peaks, trees).flatten());
        out.extend(build_lines(layer, Brush::Path));
        for landmark in &layer.landmarks {
            for batch in super::landmark_batches(landmark) {
                out.extend(batch.shapes.iter().cloned().map(|mut s| {
                    if let Shape::Polygon { opacity, .. } = &mut s {
                        *opacity *= layer.opacity;
                    }
                    s
                }));
            }
        }
        for label in &layer.labels {
            out.extend(super::label_shapes(label).iter().cloned().map(|mut shape| {
                if let Shape::Polygon { opacity, .. } = &mut shape {
                    *opacity *= layer.opacity;
                }
                shape
            }));
        }
    }
    out
}

#[cfg(test)]
fn build_lines(layer: &Layer, brush: Brush) -> impl Iterator<Item = Shape> + '_ {
    layer
        .strokes
        .iter()
        .enumerate()
        .filter(move |(_, stroke)| stroke.brush == brush)
        .flat_map(|(index, stroke)| {
            let mut out = Vec::new();
            super::work::update(|work| work.line_strokes += 1);
            linear_stroke(&mut out, layer, index, stroke);
            out
        })
}

/// Merge the forest's existing trunk order with the peaks' ground order.
/// Both flanks share one depth, and tied trees draw first.
pub(super) fn relief_order<T>(
    peaks: impl Iterator<Item = (f32, [T; 2])>,
    trees: impl Iterator<Item = (f32, T)>,
) -> impl Iterator<Item = T> {
    let mut peaks: Vec<_> = peaks.collect();
    peaks.sort_by(|(a, _), (b, _)| a.total_cmp(b));
    let mut flanks = peaks
        .into_iter()
        .flat_map(|(base, pair)| pair.map(|flank| (base, flank)))
        .peekable();
    let mut trees = trees.peekable();
    std::iter::from_fn(move || {
        let next = match (trees.peek(), flanks.peek()) {
            (Some((root, _)), Some((base, _))) if root <= base => trees.next(),
            (_, Some(_)) => flanks.next(),
            _ => trees.next(),
        };
        next.map(|(_, batch)| batch)
    })
}
