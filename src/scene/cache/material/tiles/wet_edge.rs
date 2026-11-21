//! The newly painted side of the resolved pigment perimeter, clipped per tile.
use super::{Field, REGIONS, TerrainTiles, brush_index, regions, texture};
use crate::{
    document::{AtlasStyle, Brush, Stroke},
    scene::{
        Shape, generation,
        terrain::materials::{self, Contours},
    },
};
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::single::SingleFloatOverlay,
    mesh::float::{stroke::offset::StrokeOffset, style::StrokeStyle},
};
use std::sync::Arc;

pub struct WetEdge {
    pub region: (u64, Brush, usize),
    pub shapes: Vec<Shape>,
}

fn overlay(a: &Contours, b: &Contours, rule: OverlayRule) -> Contours {
    a.overlay_as::<i64>(b, rule, FillRule::EvenOdd)
        .into_iter()
        .flatten()
        .collect()
}

fn pigment(field: &Field, coast: Option<&Field>, finish: Option<usize>) -> Contours {
    let contour = |field: &Field| {
        field
            .coverage
            .as_ref()
            .map_or_else(Vec::new, |grid| regions::contours(grid))
    };
    let mut surface = contour(field);
    if let Some(coast) = coast {
        surface = overlay(&surface, &contour(coast), OverlayRule::Difference);
    }
    // Match the finish partition's ordering where sampled contours overlap.
    for (&index, grid) in &field.finishes {
        let mask = regions::contours(grid);
        if finish == Some(index) {
            return overlay(&surface, &mask, OverlayRule::Intersect);
        }
        surface = overlay(&surface, &mask, OverlayRule::Difference);
    }
    if finish.is_some() {
        Vec::new()
    } else {
        surface
    }
}

impl TerrainTiles {
    pub(in crate::scene::cache) fn wet_edges(
        &self,
        prior: &Self,
        stroke: &Stroke,
        layer: u64,
    ) -> Vec<WetEdge> {
        let Some(index) = brush_index(stroke.brush) else {
            return Vec::new();
        };
        let finish = texture(stroke, self.style);
        let color = match (REGIONS[index], self.style) {
            (Brush::Land, AtlasStyle::Classic) => 0x785b3c,
            (Brush::Land, AtlasStyle::Woodland) => 0x6f603e,
            (_, AtlasStyle::Classic) => 0x53695e,
            (_, AtlasStyle::Woodland) => 0x43645c,
        };
        let mut output = Vec::new();
        for (coord, tile) in self.tiles.iter().take_while(|_| !generation::stopped()) {
            let region = &tile.regions[index];
            let batches = &region.batches;
            if batches.is_empty() {
                continue;
            }
            let old = prior.tiles.get(coord);
            if old.is_some_and(|old| {
                if Arc::ptr_eq(old, tile) {
                    return true;
                }
                let previous = &old.regions[index].batches;
                batches.len() == previous.len()
                    && batches.iter().zip(previous).all(|(a, b)| Arc::ptr_eq(a, b))
            }) {
                continue;
            }
            let after = pigment(
                &tile.fields[index],
                (index == 1).then_some(&tile.fields[0]),
                finish,
            );
            let before = old.map_or_else(Vec::new, |old| {
                pigment(
                    &old.fields[index],
                    (index == 1).then_some(&old.fields[0]),
                    finish,
                )
            });
            if after == before {
                continue;
            }
            let change = overlay(&after, &before, OverlayRule::Difference);
            let change = overlay(
                &change,
                &regions::rectangle(coord.bounds()),
                OverlayRule::Intersect,
            );
            if change.is_empty() {
                continue;
            }
            // Build the edge from apron contours so the tile frame and the
            // previous shoreline never become artificial ink lines.
            let mut previous = Vec::new();
            let mut shapes = Vec::new();
            for (width, opacity) in [(5., 0.27), (12., 0.14), (20., 0.055)] {
                let rim = after
                    .stroke_as::<i64>(StrokeStyle::new(width), true)
                    .into_iter()
                    .flatten()
                    .collect();
                let covered = overlay(&change, &rim, OverlayRule::Intersect);
                let band = overlay(&covered, &previous, OverlayRule::Difference);
                materials::surface(&mut shapes, band, None, color, opacity);
                previous = covered;
            }
            if !shapes.is_empty() {
                let source = (layer, stroke.brush, Arc::as_ptr(&region.surface) as usize);
                output.push(WetEdge {
                    region: source,
                    shapes,
                });
            }
        }
        output
    }
}
