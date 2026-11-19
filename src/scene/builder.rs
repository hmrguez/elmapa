use super::Shape;
use crate::document::Document;

pub fn build_scene(doc: &Document) -> Vec<Shape> {
    super::SceneCache::default()
        .render(doc, None)
        .shapes()
        .cloned()
        .collect()
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
