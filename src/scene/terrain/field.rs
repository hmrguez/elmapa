use super::super::geometry::stroke_distance;
#[cfg(test)]
use super::super::noise::noise;
use crate::document::{Brush, Layer, Point, Stroke};

pub(in crate::scene) fn radius(stroke: &Stroke) -> f32 {
    // Strength controls coverage for region brushes and erasers. The square root
    // keeps low strengths useful while leaving full-strength authored maps intact.
    stroke.size.max(2.0) * 0.5 * stroke.strength.clamp(0.0, 1.0).sqrt()
}

// Land owns the shared coast. Keep the painted water footprint separately so
// it can be clipped to the exact same contour after sampling and smoothing.
pub(in crate::scene) fn cuts_region(paint: Brush, region: Brush) -> bool {
    paint == Brush::Erase || (region == Brush::Land && paint == Brush::Water)
}
pub(super) fn erased(q: Point, layer: &Layer, after: usize) -> bool {
    if super::super::generation::is_stopped() {
        return false;
    }
    if let Some(erased) = QUERY_INDEX.with(|slot| {
        slot.borrow()
            .as_ref()
            .filter(|scope| scope.strokes == layer.strokes.storage_id())
            .map(|scope| scope.index.erased(q, after))
    }) {
        return erased;
    }
    layer
        .strokes
        .iter()
        .skip(after + 1)
        .any(|s| s.brush == Brush::Erase && stroke_distance(q, s) < radius(s))
}
#[cfg(test)]
pub(in crate::scene) fn region_field(q: Point, layer: &Layer, brush: Brush) -> f32 {
    let mut field: f32 = -10_000.0;
    for (i, s) in layer.strokes.iter().enumerate() {
        if s.brush != brush || s.points.is_empty() || s.strength <= 0.0 {
            continue;
        }
        let mut value = radius(s) - stroke_distance(q, s) + noise(q, s.seed) * roughness(s);
        // A cut subtracts from earlier strokes, while later paint survives.
        for e in layer
            .strokes
            .iter()
            .skip(i + 1)
            .filter(|e| cuts_region(e.brush, brush) && e.strength > 0.0)
        {
            let dab = radius(e) - stroke_distance(q, e) + noise(q, e.seed) * roughness(e);
            value = value.min(-dab);
        }
        field = field.max(value);
    }
    field
}

pub(in crate::scene) fn roughness(stroke: &Stroke) -> f32 {
    match stroke.brush {
        Brush::Forest => 13.0,
        Brush::Land | Brush::Water => (stroke.size * 0.07).clamp(4.0, 18.0),
        _ => 0.0,
    }
}

struct EraserScope {
    strokes: usize,
    index: std::sync::Arc<super::field_index::FieldIndex>,
}
thread_local! { static QUERY_INDEX: std::cell::RefCell<Option<EraserScope>> = const { std::cell::RefCell::new(None) }; }
/// The supplied field must index the same ordered erasers as the source layer.
pub(in crate::scene) fn with_index<T>(
    layer: &Layer,
    index: std::sync::Arc<super::field_index::FieldIndex>,
    f: impl FnOnce() -> T,
) -> T {
    struct Guard(Option<EraserScope>);
    impl Drop for Guard {
        fn drop(&mut self) {
            QUERY_INDEX.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let scope = EraserScope {
        strokes: layer.strokes.storage_id(),
        index,
    };
    let guard = Guard(QUERY_INDEX.with(|slot| slot.replace(Some(scope))));
    let output = f();
    drop(guard);
    output
}
