//! Group visible artwork into chronological replay cues.
use super::{Canvas, Cue, Entrance, paint_order};
use crate::app::render::{
    forest_atlas::TreeSprite, motion::tree_transform, retained::TerrainMaterial,
};
use bevy::prelude::{Assets, Entity, Image, Vec3};
use elmapa::{
    document::{Brush, ContentBounds, Document, Point, Stroke},
    scene::{ForestTree, Scene, Shape, region_stroke_covers},
};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum Source {
    Stroke(u64, usize),
    Object(u64),
    Region(u64, Brush, usize),
    OrderedRegion(u64, Brush),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Chronology {
    LegacyStroke(u64, usize),
    Recorded(u64),
    Unattributed,
}

impl Chronology {
    fn stroke(layer: u64, index: usize, stroke: &Stroke) -> Self {
        stroke
            .creation_id
            .map_or(Self::LegacyStroke(layer, index), Self::Recorded)
    }
}

struct Group {
    bounds: ContentBounds,
    painter_order: usize,
    cues: Vec<Cue>,
}

fn batch_bounds(batch: &[Shape]) -> Option<ContentBounds> {
    batch
        .iter()
        .filter_map(Shape::bounds)
        .reduce(ContentBounds::union)
}

fn tree_bounds(tree: &ForestTree, sprite: TreeSprite) -> ContentBounds {
    let pose = tree_transform(tree, 0., 1.);
    let horizontal = (pose.rotation * Vec3::X).truncate().abs();
    let vertical = (pose.rotation * Vec3::Y).truncate().abs();
    let half = (horizontal * sprite.extent.x + vertical * sprite.extent.y) * (tree.scale * 0.5);
    let center = pose.transform_point(Vec3::new(sprite.offset.x, -sprite.offset.y, 0.));
    ContentBounds {
        min: Point::new(center.x - half.x, -center.y - half.y),
        max: Point::new(center.x + half.x, -center.y + half.y),
    }
}

fn cue_source(
    scene: &Scene,
    batch: &Arc<Vec<Shape>>,
    canvas: &Canvas,
) -> Option<(Source, Entity, ContentBounds, Entrance)> {
    if let Some(tree) = scene.forest_tree(batch) {
        let retained = canvas.trees.get(&tree.id)?;
        let sprite = canvas.forest_atlas.sprite(tree.variant);
        return Some((
            Source::Stroke(tree.id.0, tree.id.1),
            retained.entity,
            tree_bounds(tree, sprite),
            Entrance::Tree(tree.clone()),
        ));
    }
    let retained = canvas.meshes.get(&(Arc::as_ptr(batch) as usize))?;
    let (source, entrance) = match &retained.material {
        TerrainMaterial::River(handle) => {
            let reach = scene.river_reach(batch)?;
            (
                Source::Stroke(reach.id.0, reach.id.1),
                Entrance::River(reach.clone(), handle.clone()),
            )
        }
        TerrainMaterial::Tile { handle, .. } => {
            let (layer, brush, tile) = scene.region(batch)?;
            (
                Source::Region(layer, brush, tile),
                Entrance::Wash {
                    original: handle.clone(),
                    material: None,
                    order: None,
                },
            )
        }
        TerrainMaterial::Ink(handle) => {
            if let Some(peak) = scene.mountain_part(batch) {
                (
                    Source::Stroke(peak.id.0, peak.id.1),
                    Entrance::Mountain(peak.clone()),
                )
            } else if let Some(section) = scene.path_part(batch) {
                (
                    Source::Stroke(section.id.0, section.id.1),
                    Entrance::Trail(section.clone()),
                )
            } else if let Some(label) = scene.label(batch).filter(|label| label.opacity > 0.) {
                (Source::Object(label.id), Entrance::Label(label.position))
            } else if let Some(part) = scene.landmark_part(batch) {
                (
                    Source::Object(part.landmark.id),
                    Entrance::Landmark(part.clone(), handle.clone(), scene.batch_opacity(batch)),
                )
            } else {
                return None;
            }
        }
    };
    Some((source, retained.entity, batch_bounds(batch)?, entrance))
}

fn clip(bounds: ContentBounds, frame: ContentBounds) -> ContentBounds {
    ContentBounds {
        min: Point::new(bounds.min.x.max(frame.min.x), bounds.min.y.max(frame.min.y)),
        max: Point::new(bounds.max.x.min(frame.max.x), bounds.max.y.min(frame.max.y)),
    }
}

pub(super) fn timeline(
    scene: &Scene,
    canvas: &Canvas,
    doc: &Document,
    frame: ContentBounds,
    images: &mut Assets<Image>,
) -> VecDeque<Cue> {
    let mut groups = HashMap::<Source, Group>::new();
    let mut surface_orders = HashMap::new();
    let mut budget = paint_order::SAMPLE_BUDGET;
    for (order, batch) in scene.batches().enumerate() {
        let Some((mut source, entity, bounds, mut entrance)) =
            cue_source(scene, batch, canvas).filter(|(_, _, bounds, _)| bounds.intersects(frame))
        else {
            continue;
        };
        if let Source::Region(layer, brush, _) = source
            && let Entrance::Wash { order, .. } = &mut entrance
            && let Some((field, _)) = surface_orders.entry((layer, brush)).or_insert_with(|| {
                let layer = doc.layer(layer)?;
                layer.terrain_replay.as_ref()?;
                paint_order::prepare(layer, brush, frame, images, &mut budget)
            })
        {
            source = Source::OrderedRegion(layer, brush);
            *order = Some(field.clone());
        }
        let group = groups.entry(source).or_insert_with(|| Group {
            bounds,
            painter_order: order,
            cues: Vec::new(),
        });
        group.bounds = group.bounds.union(bounds);
        group.cues.push(Cue {
            entity,
            z: order as f32 * 0.0001,
            start: 0.,
            entrance,
            span: (0., 0.),
        });
    }

    // Merged paint has no single source. Prefer a stroke reaching the center
    // of the visible contour, with a bounds fallback for thin or curved regions.
    let mut paint = HashMap::<_, Vec<_>>::new();
    if groups
        .keys()
        .any(|source| matches!(source, Source::Region(..)))
    {
        for layer in doc
            .layers
            .iter()
            .filter(|layer| layer.visible && layer.opacity > 0.)
        {
            for (index, stroke) in layer.strokes.iter().enumerate().filter(|(_, stroke)| {
                matches!(stroke.brush, Brush::Land | Brush::Water) && stroke.strength > 0.
            }) {
                if let Some(bounds) = stroke.bounds().filter(|bounds| bounds.intersects(frame)) {
                    paint.entry((layer.id, stroke.brush)).or_default().push((
                        bounds,
                        Chronology::stroke(layer.id, index, stroke),
                        stroke,
                    ));
                }
            }
        }
    }
    let mut ordered: Vec<_> = groups
        .into_iter()
        .map(|(source, group)| {
            let chronology = match source {
                Source::Object(id) => Chronology::Recorded(id),
                Source::Stroke(layer, index) => doc
                    .layer(layer)
                    .and_then(|layer| layer.strokes.get(index))
                    .map_or(Chronology::Unattributed, |stroke| {
                        Chronology::stroke(layer, index, stroke)
                    }),
                Source::OrderedRegion(layer, brush) => surface_orders[&(layer, brush)]
                    .as_ref()
                    .and_then(|(_, index)| {
                        let stroke = doc.layer(layer)?.strokes.get(*index)?;
                        Some(Chronology::stroke(layer, *index, stroke))
                    })
                    .unwrap_or(Chronology::Unattributed),
                Source::Region(layer, brush, _) => {
                    let visible = clip(group.bounds, frame);
                    let center = Point::new(
                        (visible.min.x + visible.max.x) * 0.5,
                        (visible.min.y + visible.max.y) * 0.5,
                    );
                    let mut candidates = paint
                        .get(&(layer, brush))
                        .into_iter()
                        .flatten()
                        .filter(|(bounds, _, _)| bounds.intersects(visible))
                        .peekable();
                    let fallback = candidates.peek().map(|(_, order, _)| *order);
                    candidates
                        .find(|(_, _, stroke)| region_stroke_covers(stroke, center))
                        .map(|(_, order, _)| *order)
                        .or(fallback)
                        .unwrap_or(Chronology::Unattributed)
                }
            };
            (
                chronology,
                group,
                matches!(source, Source::OrderedRegion(..)),
            )
        })
        .collect();
    ordered.sort_by(|(a, first, _), (b, second, _)| {
        a.cmp(b)
            .then_with(|| first.painter_order.cmp(&second.painter_order))
    });
    let spacing = (4.8 / ordered.len().saturating_sub(1).max(1) as f32).min(0.22);
    let mut cues = Vec::new();
    let mut hold = 0.0;
    for (index, (_, mut group, waits_for_wash)) in ordered.into_iter().enumerate() {
        let start = 0.12 + index as f32 * spacing + hold;
        if waits_for_wash {
            hold += group.cues[0].entrance.duration();
        }
        let min_step = group
            .cues
            .iter()
            .map(|c| c.entrance.step())
            .min()
            .unwrap_or(0);
        let max_step = group
            .cues
            .iter()
            .map(|c| c.entrance.step())
            .max()
            .unwrap_or(0);
        let step_time = (0.18 / (max_step - min_step).max(1) as f32).min(0.024);
        let visible = clip(group.bounds, frame);
        let wash_origin = visible.min.x * 0.8 + visible.min.y * 0.6;
        let wash_span = (visible.width() * 0.8 + visible.height() * 0.6).max(1.);
        let river_span = group
            .cues
            .iter()
            .filter_map(|cue| {
                let Entrance::River(reach, _) = &cue.entrance else {
                    return None;
                };
                Some((reach.distance, reach.distance + reach.length))
            })
            .reduce(|(first, last), (start, end)| (first.min(start), last.max(end)));
        for cue in &mut group.cues {
            cue.start = start + (cue.entrance.step() - min_step) as f32 * step_time;
            cue.span = river_span.unwrap_or((wash_origin, wash_span));
        }
        cues.extend(group.cues);
    }
    cues.sort_by(|a, b| a.start.total_cmp(&b.start));
    cues.into()
}
