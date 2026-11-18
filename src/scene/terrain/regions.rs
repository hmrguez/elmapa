//! Resolve land, water and shoreline artwork with an apron around each tile.
use super::{
    LAND, SEA,
    contours::RegionGrid,
    materials::{self, Contours, Masks},
};
use crate::{
    document::{Brush, ContentBounds},
    scene::{Shape, generation, geometry::softened},
};
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::single::SingleFloatOverlay,
    mesh::float::{
        outline::offset::OutlineOffset,
        stroke::offset::StrokeOffset,
        style::{LineJoin, OutlineStyle, StrokeStyle},
    },
};
use std::sync::Arc;

pub(in crate::scene) const SHORE_INK: u32 = 0x777d65;
pub(in crate::scene) const SHORE_WASH: [(f64, u32, f32); 2] =
    [(17., 0xaaba9e, 0.28), (8., 0xf5ecd0, 0.62)];

pub(in crate::scene) fn contours(grid: &RegionGrid) -> Contours {
    grid.contours_ref()
        .iter()
        .map(|ring| materials::coordinates(&softened(ring)))
        .collect()
}

pub(in crate::scene) fn rectangle(bounds: ContentBounds) -> Contours {
    let [x0, y0, x1, y1] = [bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y].map(f64::from);
    vec![vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]]
}

pub(in crate::scene) fn build(
    grid: &RegionGrid,
    brush: Brush,
    masks: &Masks,
    coast: Option<&Arc<RegionGrid>>,
    bounds: ContentBounds,
) -> (Contours, Vec<Vec<Shape>>) {
    let subject = contours(grid);
    if subject.is_empty() || generation::stopped() {
        return (Vec::new(), Vec::new());
    }
    super::super::work::update(|work| work.region_batches += 1);

    let coast = coast.map_or_else(Vec::new, |land| contours(land));
    let regions = subject.overlay_as::<i64>(&coast, OverlayRule::Difference, FillRule::EvenOdd);
    if regions.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let fallback = if brush == Brush::Land { LAND } else { SEA };
    let (finishes, mut details) = materials::partition(&regions, masks, fallback);
    let surface: Contours = regions.into_iter().flatten().collect();
    let frame = rectangle(bounds);
    if brush == Brush::Land {
        for (width, color, opacity) in SHORE_WASH {
            if generation::stopped() {
                break;
            }
            let expanded: Contours = surface
                .outline_as::<i64>(&OutlineStyle::new(width))
                .into_iter()
                .flatten()
                .collect();
            let water_side: Contours = expanded
                .overlay_as::<i64>(&surface, OverlayRule::Difference, FillRule::EvenOdd)
                .into_iter()
                .flatten()
                .collect();
            materials::surface(&mut details, water_side, None, color, opacity);
        }
        // Expand ink before clipping so a real shoreline crossing the tile
        // edge keeps its full width and receives no artificial line cap.
        let ink = surface
            .stroke_as::<i64>(ink_style(1.5), true)
            .into_iter()
            .flatten()
            .collect();
        materials::surface(&mut details, ink, None, SHORE_INK, 0.78);
    }
    let finishes = finishes.into_iter().filter_map(|shape| {
        let mut batch = Vec::new();
        clip_shape(&mut batch, shape, &frame);
        (!batch.is_empty()).then_some(batch)
    });
    let mut clipped_details = Vec::new();
    for shape in details {
        clip_shape(&mut clipped_details, shape, &frame);
    }
    let batches = finishes
        .chain((!clipped_details.is_empty()).then_some(clipped_details))
        .collect();
    (clip(&surface, &frame), batches)
}

fn ink_style(width: f64) -> StrokeStyle<[f64; 2]> {
    StrokeStyle::new(width).line_join(LineJoin::Miter(std::f64::consts::FRAC_PI_6))
}

fn clip_shape(out: &mut Vec<Shape>, shape: Shape, frame: &Contours) {
    let (contours, material, fill, opacity) = match shape {
        Shape::Polygon {
            points,
            fill,
            opacity,
        } => (vec![materials::coordinates(&points)], None, fill, opacity),
        Shape::RegionSurface {
            contours,
            material,
            fill,
            opacity,
        } => (
            contours
                .iter()
                .map(|ring| materials::coordinates(ring))
                .collect(),
            material,
            fill,
            opacity,
        ),
        Shape::Line {
            points,
            color,
            opacity,
            width,
        } => (
            materials::coordinates(&points)
                .stroke_as::<i64>(ink_style(f64::from(width)), false)
                .into_iter()
                .flatten()
                .collect(),
            None,
            color,
            opacity,
        ),
        Shape::Sprite { .. } => unreachable!("terrain tiles contain vector geometry"),
    };
    materials::surface(out, clip(&contours, frame), material, fill, opacity);
}

fn clip(contours: &Contours, frame: &Contours) -> Contours {
    contours
        .overlay_as::<i64>(frame, OverlayRule::Intersect, FillRule::EvenOdd)
        .into_iter()
        .flatten()
        .collect()
}
