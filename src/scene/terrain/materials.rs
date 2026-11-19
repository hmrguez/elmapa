//! Paint finishes partition the same region contours used for shorelines.
use super::{
    super::{Shape, generation, shape::polygon},
    water_ripples,
};
use crate::{brush_materials, document::Point};
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::single::SingleFloatOverlay,
};

pub(in crate::scene) type Contours = Vec<Vec<[f64; 2]>>;
pub(in crate::scene) type Masks = Vec<(usize, Contours)>;

pub(in crate::scene) fn coordinates(points: &[Point]) -> Vec<[f64; 2]> {
    points
        .iter()
        .map(|p| [f64::from(p.x), f64::from(p.y)])
        .collect()
}

pub(super) fn partition(
    regions: &[Contours],
    masks: &Masks,
    fallback: u32,
) -> (Vec<Shape>, Vec<Shape>) {
    let shore: Contours = regions.iter().flatten().cloned().collect();
    let mut available = shore.clone();
    let mut fills = Vec::new();
    let mut ripples = Vec::new();
    for (material, mask) in masks {
        if available.is_empty() || generation::stopped() {
            break;
        }
        let textured: Contours = available
            .overlay_as::<i64>(mask, OverlayRule::Intersect, FillRule::EvenOdd)
            .into_iter()
            .flatten()
            .collect();
        if textured.is_empty() {
            continue;
        }
        available = available
            .overlay_as::<i64>(&textured, OverlayRule::Difference, FillRule::EvenOdd)
            .into_iter()
            .flatten()
            .collect();
        if let Some(ink) = brush_materials::artwork_catalog()[*material].shoreline_ink {
            for region in regions {
                water_ripples::draw(&mut ripples, region, &textured, ink);
            }
        }
        surface(&mut fills, textured, Some(*material), fallback, 1.);
    }
    let mut details = Vec::new();
    surface(&mut details, available, None, fallback, 1.);
    details.extend(ripples);
    (fills, details)
}

pub(in crate::scene) fn points(ring: &[[f64; 2]]) -> Vec<Point> {
    ring.iter()
        .map(|&[x, y]| Point::new(x as f32, y as f32))
        .collect()
}

pub(in crate::scene) fn surface(
    out: &mut Vec<Shape>,
    contours: Contours,
    material: Option<usize>,
    fill: u32,
    opacity: f32,
) {
    if material.is_none()
        && let [ring] = contours.as_slice()
    {
        polygon(out, points(ring), fill, opacity);
    } else if !contours.is_empty() && generation::vertices(contours.iter().map(Vec::len).sum()) {
        out.push(Shape::RegionSurface {
            contours: contours.iter().map(|ring| points(ring)).collect(),
            material,
            fill,
            opacity,
        });
    }
}
