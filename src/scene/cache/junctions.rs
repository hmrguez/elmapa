//! Clip rivers against the visible terrain and open the ink at river mouths.
use super::{Batch, MaterialLayer, REGIONS, Scene};
use crate::{
    document::{Brush, ContentBounds},
    scene::{
        RiverReach, Shape, generation,
        terrain::{
            materials::{Contours, coordinates, surface},
            regions::shore_ink,
        },
    },
};
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::single::SingleFloatOverlay,
    mesh::float::{stroke::offset::StrokeOffset, style::StrokeStyle},
};
use std::{collections::HashMap, mem, sync::Arc};

#[derive(Default)]
pub(super) struct Junctions {
    rivers: HashMap<usize, RiverCut>,
    shores: HashMap<usize, ShoreCut>,
}

struct Surface<'a> {
    brush: Brush,
    bounds: ContentBounds,
    contours: &'a Arc<Contours>,
}

struct RiverCut {
    source: Batch,
    source_reach: RiverReach,
    terrain: Vec<Arc<Contours>>,
    draw_on_backdrop: bool,
    batch: Batch,
    reach: Arc<RiverReach>,
    mouth: Option<Arc<Mouth>>,
}

struct Mouth {
    bounds: ContentBounds,
    channels: Contours,
}

struct ShoreCut {
    source: Batch,
    mouths: Vec<Arc<Mouth>>,
    batch: Batch,
}

impl Junctions {
    pub(super) fn apply<'a>(
        &mut self,
        scene: &mut Scene,
        layers: impl Iterator<Item = &'a MaterialLayer>,
    ) {
        if scene.rivers.is_empty() {
            return;
        }
        let layers: Vec<_> = layers
            .filter(|layer| layer.source.visible && layer.source.opacity > 0.)
            .collect();
        let layer_order: HashMap<_, _> = layers
            .iter()
            .enumerate()
            .map(|(order, layer)| (layer.source.id, order))
            .collect();
        let surfaces: Vec<_> = layers
            .into_iter()
            .flat_map(|layer| {
                REGIONS
                    .into_iter()
                    .enumerate()
                    .rev()
                    .flat_map(|(index, brush)| {
                        layer
                            .terrain
                            .regions(index)
                            .filter_map(move |(coord, region)| {
                                (!region.surface.is_empty()).then_some(Surface {
                                    brush,
                                    bounds: coord.bounds(),
                                    contours: &region.surface,
                                })
                            })
                    })
            })
            .collect();
        if surfaces.is_empty() {
            return;
        }
        // Keep freehand river sketches on an otherwise unpainted map. Once
        // islands define a coast, the backdrop around them is open water.
        let draw_on_backdrop = !surfaces.iter().any(|surface| surface.brush == Brush::Land);
        let mut old_rivers = mem::take(&mut self.rivers);
        let mut old_shores = mem::take(&mut self.shores);
        let mut mouths = Vec::new();
        let mut replacements = HashMap::new();
        for batch in scene.batches() {
            let Some(reach) = scene.river_reach(batch).filter(|_| !batch.is_empty()) else {
                continue;
            };
            if generation::stopped() {
                return;
            }
            let key = Arc::as_ptr(batch) as usize;
            let bounds = reach
                .course
                .iter()
                .fold(batch_bounds(batch).unwrap(), |bounds, &q| {
                    bounds
                        .union(ContentBounds { min: q, max: q }.padded(reach.half_width * 1.5 + 9.))
                });
            let terrain: Vec<_> = surfaces
                .iter()
                .filter(|surface| surface.bounds.intersects(bounds))
                .collect();
            let retained = old_rivers.remove(&key);
            let cut = retained
                .filter(|cut| {
                    Arc::ptr_eq(&cut.source, batch)
                        && cut.source_reach == *reach
                        && cut.draw_on_backdrop == draw_on_backdrop
                        && cut.terrain.len() == terrain.len()
                        && cut
                            .terrain
                            .iter()
                            .zip(&terrain)
                            .all(|(old, surface)| Arc::ptr_eq(old, surface.contours))
                })
                .unwrap_or_else(|| RiverCut::new(batch, reach, bounds, &terrain, draw_on_backdrop));
            if let Some(mouth) = &cut.mouth {
                mouths.push((layer_order[&reach.id.0], mouth.clone()));
            }
            if !Arc::ptr_eq(batch, &cut.batch) {
                replacements.insert(key, cut.batch.clone());
            }
            self.rivers.insert(key, cut);
        }
        let shore_ink = shore_ink(scene.style());
        for batch in scene.batches() {
            if mouths.is_empty() {
                break;
            }
            let Some((layer, Brush::Land, _)) = scene.region(batch) else {
                continue;
            };
            if !batch.iter().any(|shape| is_shore_ink(shape, shore_ink)) {
                continue;
            }
            if generation::stopped() {
                return;
            }
            let bounds = batch_bounds(batch).unwrap();
            let nearby: Vec<_> = mouths
                .iter()
                .filter(|(order, mouth)| {
                    *order >= layer_order[&layer] && mouth.bounds.intersects(bounds)
                })
                .map(|(_, mouth)| mouth)
                .collect();
            if nearby.is_empty() {
                continue;
            }
            let key = Arc::as_ptr(batch) as usize;
            let cut = old_shores
                .remove(&key)
                .filter(|cut| {
                    Arc::ptr_eq(&cut.source, batch)
                        && cut.mouths.len() == nearby.len()
                        && cut
                            .mouths
                            .iter()
                            .zip(&nearby)
                            .all(|(old, mouth)| Arc::ptr_eq(old, mouth))
                })
                .unwrap_or_else(|| ShoreCut {
                    source: batch.clone(),
                    batch: open_shore(batch, &nearby, shore_ink),
                    mouths: nearby.into_iter().cloned().collect(),
                });
            if !Arc::ptr_eq(batch, &cut.batch) {
                replacements.insert(key, cut.batch.clone());
            }
            self.shores.insert(key, cut);
        }
        Arc::make_mut(&mut scene.batches).retain_mut(|batch| {
            let key = Arc::as_ptr(batch) as usize;
            let Some(replacement) = replacements.remove(&key) else {
                return true;
            };
            let opacity = Arc::make_mut(&mut scene.opacity).remove(&key);
            let region = Arc::make_mut(&mut scene.regions).remove(&key);
            let rivers = Arc::make_mut(&mut scene.rivers);
            rivers.remove(&key);
            if replacement.is_empty() {
                return false;
            }
            let new_key = Arc::as_ptr(&replacement) as usize;
            if let Some(opacity) = opacity {
                Arc::make_mut(&mut scene.opacity).insert(new_key, opacity);
            }
            if let Some(region) = region {
                Arc::make_mut(&mut scene.regions).insert(new_key, region);
            }
            if let Some(cut) = self.rivers.get(&key) {
                rivers.insert(new_key, cut.reach.clone());
            }
            *batch = replacement;
            true
        });
    }
}

impl RiverCut {
    fn new(
        source: &Batch,
        reach: &RiverReach,
        bounds: ContentBounds,
        terrain: &[&Surface<'_>],
        draw_on_backdrop: bool,
    ) -> Self {
        let source_reach = reach.clone();
        let mut reach = reach.clone();
        let mut mouth = None;
        let batch = match land_mask(bounds, terrain, draw_on_backdrop) {
            LandMask::Dry => source.clone(),
            LandMask::Wet => Arc::default(),
            LandMask::Shore(mask) => {
                reach.spray = false;
                let clipped: Vec<_> = source
                    .iter()
                    .flat_map(|shape| clip_river(shape, &mask))
                    .collect();
                if !clipped.is_empty() {
                    let channels: Contours = source
                        .iter()
                        .filter_map(|shape| match shape {
                            Shape::Polygon { points, fill, .. } if *fill == reach.channel_fill => {
                                Some(coordinates(points))
                            }
                            _ => None,
                        })
                        .collect();
                    mouth = Some(Arc::new(Mouth {
                        bounds: batch_bounds(source).unwrap(),
                        channels,
                    }));
                }
                Arc::new(clipped)
            }
        };
        Self {
            source: source.clone(),
            source_reach,
            terrain: terrain
                .iter()
                .map(|surface| surface.contours.clone())
                .collect(),
            draw_on_backdrop,
            batch,
            reach: Arc::new(reach),
            mouth,
        }
    }
}

fn batch_bounds(batch: &[Shape]) -> Option<ContentBounds> {
    batch
        .iter()
        .filter_map(Shape::bounds)
        .reduce(ContentBounds::union)
}

enum LandMask {
    Dry,
    Wet,
    Shore(Contours),
}

fn land_mask(bounds: ContentBounds, terrain: &[&Surface<'_>], mut land: bool) -> LandMask {
    // The highest surface that covers the reach completely occludes every
    // surface below it. Only boundaries within this reach need boolean work.
    let mut edges = Vec::new();
    for surface in terrain.iter().rev() {
        let (hits_edge, inside) = coverage(bounds, surface.contours);
        if hits_edge {
            edges.push(*surface);
        } else if inside {
            land = surface.brush == Brush::Land;
            break;
        }
    }
    if edges.is_empty() {
        return if land { LandMask::Dry } else { LandMask::Wet };
    }
    let [x0, y0] = [f64::from(bounds.min.x), f64::from(bounds.min.y)];
    let [x1, y1] = [f64::from(bounds.max.x), f64::from(bounds.max.y)];
    let frame = vec![vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]];
    let mut mask = if land { frame.clone() } else { Vec::new() };
    for surface in edges.into_iter().rev() {
        let rule = if surface.brush == Brush::Land {
            OverlayRule::Union
        } else {
            OverlayRule::Difference
        };
        let local = overlay(&frame, surface.contours, OverlayRule::Intersect);
        mask = overlay(&mask, &local, rule);
    }
    if mask.is_empty() {
        LandMask::Wet
    } else if overlay(&frame, &mask, OverlayRule::Difference).is_empty() {
        // An edge of a terrain tile is not a shoreline.
        LandMask::Dry
    } else {
        LandMask::Shore(mask)
    }
}

fn coverage(bounds: ContentBounds, contours: &Contours) -> (bool, bool) {
    let x = f64::from(bounds.min.x);
    let y = f64::from(bounds.min.y);
    let mut inside = false;
    for ring in contours {
        for (&[ax, ay], &[bx, by]) in ring.iter().zip(ring.iter().cycle().skip(1)) {
            if ax.min(bx) <= f64::from(bounds.max.x)
                && ax.max(bx) >= x
                && ay.min(by) <= f64::from(bounds.max.y)
                && ay.max(by) >= y
            {
                return (true, false);
            }
            if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
                inside = !inside;
            }
        }
    }
    (false, inside)
}

fn overlay(subject: &Contours, clip: &Contours, rule: OverlayRule) -> Contours {
    subject
        .overlay_as::<i64>(clip, rule, FillRule::EvenOdd)
        .into_iter()
        .flatten()
        .collect()
}

fn clip_river(shape: &Shape, mask: &Contours) -> Vec<Shape> {
    let (fill, opacity, contours) = match shape {
        Shape::Polygon {
            points,
            fill,
            opacity,
        } => (*fill, *opacity, vec![coordinates(points)]),
        Shape::Line {
            points,
            width,
            color,
            opacity,
        } => {
            // Clip the ink's full width, including a bank meeting the water
            // obliquely. Clipping its centerline would leave ink in the water.
            let ink = coordinates(points)
                .stroke_as::<i64>(StrokeStyle::new(f64::from(*width)), false)
                .into_iter()
                .flatten()
                .collect();
            (*color, *opacity, ink)
        }
        Shape::RegionSurface { .. } | Shape::Sprite { .. } | Shape::ShoreWash(_) => {
            unreachable!("river reaches contain ribbons and ink")
        }
    };
    let mut out = Vec::new();
    surface(
        &mut out,
        overlay(&contours, mask, OverlayRule::Intersect),
        None,
        fill,
        opacity,
    );
    out
}

fn is_shore_ink(shape: &Shape, ink: u32) -> bool {
    matches!(
        shape,
        Shape::Polygon {
            fill,
            ..
        } | Shape::RegionSurface {
            fill,
            ..
        } if *fill == ink
    )
}

fn open_shore(batch: &Batch, mouths: &[&Arc<Mouth>], color: u32) -> Batch {
    let mut changed = false;
    let mut out = Vec::new();
    for shape in batch.iter() {
        if is_shore_ink(shape, color) {
            let bounds = shape.bounds().unwrap();
            let channels: Contours = mouths
                .iter()
                .filter(|mouth| mouth.bounds.intersects(bounds))
                .flat_map(|mouth| mouth.channels.iter().cloned())
                .collect();
            if !channels.is_empty() {
                let (ink, opacity) = match shape {
                    Shape::Polygon {
                        points, opacity, ..
                    } => (vec![coordinates(points)], *opacity),
                    Shape::RegionSurface {
                        contours, opacity, ..
                    } => (
                        contours.iter().map(|ring| coordinates(ring)).collect(),
                        *opacity,
                    ),
                    _ => unreachable!("shore ink is a clipped fill"),
                };
                if !ink
                    .overlay_as::<i64>(&channels, OverlayRule::Intersect, FillRule::NonZero)
                    .is_empty()
                {
                    changed = true;
                    let remaining = ink
                        .overlay_as::<i64>(&channels, OverlayRule::Difference, FillRule::NonZero)
                        .into_iter()
                        .flatten()
                        .collect();
                    surface(&mut out, remaining, None, color, opacity);
                    continue;
                }
            }
        }
        out.push(shape.clone());
    }
    if changed {
        Arc::new(out)
    } else {
        batch.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{Document, Point, Stroke},
        scene::SceneCache,
    };

    fn stroke(brush: Brush, size: f32, xs: &[f32]) -> Stroke {
        Stroke {
            creation_id: None,
            material: None,
            brush,
            points: xs.iter().map(|&x| Point::new(x, 0.)).collect(),
            size,
            strength: 1.,
            seed: 17,
        }
    }

    fn river_art(scene: &Scene) -> impl Iterator<Item = (&[Shape], &RiverReach)> {
        scene.batches().filter_map(|batch| {
            scene
                .river_reach(batch)
                .map(|reach| (batch.as_slice(), reach))
        })
    }

    fn shore_crossings(scene: &Scene) -> Vec<f32> {
        scene
            .shapes()
            .filter(|shape| is_shore_ink(shape, shore_ink(scene.style())))
            .flat_map(|shape| match shape {
                Shape::Polygon { points, .. } => std::slice::from_ref(points),
                Shape::RegionSurface { contours, .. } => contours,
                _ => &[],
            })
            .flat_map(|points| points.iter().zip(points.iter().cycle().skip(1)))
            .filter(|(a, b)| (a.y > 0.) != (b.y > 0.))
            .map(|(a, b)| a.x - a.y * (b.x - a.x) / (b.y - a.y))
            .collect()
    }

    #[test]
    fn river_opens_the_lake_and_ocean_shores_and_draws_only_on_land() {
        for (course, finish) in [([-160., 420.], None), ([420., -160.], Some("lake"))] {
            let mut doc = Document::blank();
            doc.layers[0].strokes = vec![
                stroke(Brush::Land, 600., &[0.]),
                stroke(Brush::Water, 136., &[140.]),
            ]
            .into();
            let mut cache = SceneCache::default();
            let terrain = cache.render(&doc, None);
            assert_eq!(shore_crossings(&terrain).len(), 8);

            let mut upper = doc.layers[0].clone();
            upper.id = 2;
            upper.strokes = vec![Stroke {
                material: finish.map(Into::into),
                ..stroke(Brush::River, 84., &course)
            }]
            .into();
            doc.layers.push(upper);
            let scene = cache.render(&doc, None);
            let x: Vec<_> = river_art(&scene)
                .flat_map(|(shapes, _)| shapes.iter().flat_map(Shape::points).map(|q| q.x))
                .collect();
            assert!(x.iter().any(|&x| x < 50.));
            assert!(x.iter().any(|&x| (225. ..280.).contains(&x)));
            assert!(x.iter().all(|&x| !(100. ..185.).contains(&x) && x < 325.));
            assert!(river_art(&scene).any(|(_, reach)| !reach.spray));
            assert!(river_art(&scene).any(|(_, reach)| reach.spray));
            if finish.is_some() {
                assert!(river_art(&scene).all(|(_, reach)| reach.channel_fill == 0xa4b9b1));
            }
            let shore = shore_crossings(&scene);
            assert_eq!(shore.len(), 2, "all three river mouths must be open");
            assert!(shore.iter().all(|&x| x < -250.));
            assert!(cache.committed_scene().shapes().eq(scene.shapes()));
        }
    }

    #[test]
    fn an_upper_water_layer_clips_a_live_or_existing_river_and_hiding_it_restores_the_river() {
        let mut doc = Document::blank();
        doc.layers[0].strokes = vec![
            stroke(Brush::Land, 600., &[0.]),
            stroke(Brush::River, 84., &[-160., 160.]),
        ]
        .into();
        let mut upper = doc.layers[0].clone();
        upper.id = 2;
        upper.strokes = Default::default();
        doc.layers.push(upper);
        let mut cache = SceneCache::default();
        let original = cache.render(&doc, None);
        let x_max = |scene: &Scene| {
            river_art(scene)
                .filter_map(|(shapes, _)| batch_bounds(shapes))
                .map(|bounds| bounds.max.x)
                .fold(f32::NEG_INFINITY, f32::max)
        };
        assert!(x_max(&original) > 150.);
        let water = stroke(Brush::Water, 136., &[130.]);
        let live = cache.render(&doc, Some((2, &water)));
        assert!((45. ..90.).contains(&x_max(&live)));
        assert!(cache.committed_scene() == original);
        doc.layers[1].strokes.push(water);
        let released = cache.render(&doc, None);
        assert!(live.batches().eq(released.batches()));
        assert!(
            live.batches()
                .zip(released.batches())
                .all(|(a, b)| Arc::ptr_eq(a, b))
        );
        doc.layers[1].visible = false;
        let restored = cache.render(&doc, None);
        assert!(original.batches().eq(restored.batches()));
    }
}
