//! Original SVG artwork and declarative, per-part placement choreography.
use super::Shape;
use super::landmark_packs::{Artwork, landmark_catalog};
use crate::document::{Landmark, LandmarkKind, Point};
use lyon::{
    math::point,
    path::Path,
    tessellation::{
        BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, StrokeOptions,
        StrokeTessellator, StrokeVertex, VertexBuffers,
    },
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LandmarkMotion {
    Rise,
    Drop,
    Settle,
    Turn,
    Unfurl,
    Stamp,
}
#[derive(Debug, Deserialize)]
pub struct LandmarkStep {
    pub id: String,
    pub delay: f32,
    pub duration: f32,
    pub motion: LandmarkMotion,
    pub pivot: [f32; 2],
}
#[derive(Debug)]
pub struct LandmarkDefinition {
    pub kind: LandmarkKind,
    pub name: String,
    pub description: String,
    pub parts: Vec<LandmarkStep>,
    pub svg: String,
    pub icon: String,
    pub pack: usize,
    pub width: f32,
    pub anchor: Point,
}
pub fn landmark_registry() -> &'static [LandmarkDefinition] {
    &landmark_catalog().items
}
pub fn landmark_definition(kind: &LandmarkKind) -> &'static LandmarkDefinition {
    &landmark_registry()[landmark_catalog().ordinal(kind)]
}
pub fn landmark_svg(kind: &LandmarkKind) -> &'static str {
    &landmark_definition(kind).svg
}
#[derive(Debug, Clone)]
pub struct LandmarkPart {
    pub landmark: Landmark,
    pub index: usize,
    pub pivot: Point,
}
#[derive(Clone)]
pub struct LandmarkBatch {
    pub part: LandmarkPart,
    pub shapes: Arc<Vec<Shape>>,
}
/// (x scale, y scale, world y offset, rotation radians, opacity).
pub fn landmark_pose(step: &LandmarkStep, elapsed: f32, reduced: bool) -> [f32; 5] {
    if reduced || elapsed >= step.delay + step.duration {
        return [1., 1., 0., 0., 1.];
    }
    let t = ((elapsed - step.delay) / step.duration).clamp(0., 1.);
    let ease = 1. - (1. - t).powi(3);
    let settle = (t * std::f32::consts::PI).sin() * (1. - t) * 0.025;
    let alpha = (t / 0.24).min(1.);
    match step.motion {
        LandmarkMotion::Rise => [1. + settle, 0.08 + 0.92 * ease + settle, 0., 0., alpha],
        LandmarkMotion::Drop => [1. + settle, 1. - settle, -12. * (1. - ease), 0., alpha],
        LandmarkMotion::Settle => [
            0.94 + 0.06 * ease,
            0.94 + 0.06 * ease,
            -3. * (1. - ease),
            0.,
            alpha,
        ],
        LandmarkMotion::Turn => [1., 1., 0., -0.24 * (1. - ease), alpha],
        LandmarkMotion::Unfurl => [1., 0.10 + 0.90 * ease, 0., 0., alpha],
        LandmarkMotion::Stamp => {
            let start_scale = 0.55;
            let overshoot = 2.4;
            let remaining = 1. - t;
            let scale = 1.
                + (1. - start_scale)
                    * remaining.powi(2)
                    * (overshoot - (1. + overshoot) * remaining);
            [scale, scale, 0., 0., alpha]
        }
    }
}
fn ink(paint: &usvg::Paint) -> Result<u32, String> {
    let usvg::Paint::Color(c) = paint else {
        return Err("Landmarks require solid pigments".into());
    };
    Ok((u32::from(c.red) << 16) | (u32::from(c.green) << 8) | u32::from(c.blue))
}
fn visit(group: &usvg::Group, alpha: f32, shapes: &mut Vec<Shape>) -> Result<(), String> {
    if group.clip_path().is_some() || group.mask().is_some() || !group.filters().is_empty() {
        return Err("Unsupported SVG group effect".into());
    }
    let alpha = alpha * group.opacity().get();
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => visit(g, alpha, shapes)?,
            usvg::Node::Path(p) => {
                use usvg::tiny_skia_path::PathSegment;
                let mut b = Path::builder();
                let mut open = false;
                let data = p
                    .data()
                    .clone()
                    .transform(p.abs_transform())
                    .ok_or("Invalid SVG transform")?;
                for segment in data.segments() {
                    match segment {
                        PathSegment::MoveTo(q) => {
                            if open {
                                b.end(false);
                            }
                            b.begin(point(q.x, q.y));
                            open = true;
                        }
                        PathSegment::LineTo(q) => {
                            b.line_to(point(q.x, q.y));
                        }
                        PathSegment::QuadTo(a, q) => {
                            b.quadratic_bezier_to(point(a.x, a.y), point(q.x, q.y));
                        }
                        PathSegment::CubicTo(a, c, q) => {
                            b.cubic_bezier_to(point(a.x, a.y), point(c.x, c.y), point(q.x, q.y));
                        }
                        PathSegment::Close => {
                            b.end(true);
                            open = false;
                        }
                    }
                }
                if open {
                    b.end(false);
                }
                let path = b.build();
                let mut emit = |buffers: VertexBuffers<Point, u32>, color, opacity| {
                    for tri in buffers.indices.chunks_exact(3) {
                        shapes.push(Shape::Polygon {
                            points: tri.iter().map(|&i| buffers.vertices[i as usize]).collect(),
                            fill: color,
                            opacity,
                        });
                    }
                };
                if let Some(fill) = p.fill() {
                    let mut buffers = VertexBuffers::new();
                    FillTessellator::new()
                        .tessellate_path(
                            &path,
                            &FillOptions::default().with_tolerance(0.12).with_fill_rule(
                                if fill.rule() == usvg::FillRule::EvenOdd {
                                    FillRule::EvenOdd
                                } else {
                                    FillRule::NonZero
                                },
                            ),
                            &mut BuffersBuilder::new(&mut buffers, |v: FillVertex<'_>| {
                                Point::new(v.position().x, v.position().y)
                            }),
                        )
                        .map_err(|e| format!("Invalid landmark fill: {e:?}"))?;
                    emit(buffers, ink(fill.paint())?, alpha * fill.opacity().get());
                }
                if let Some(stroke) = p.stroke() {
                    let mut buffers = VertexBuffers::new();
                    StrokeTessellator::new()
                        .tessellate_path(
                            &path,
                            &StrokeOptions::default()
                                .with_line_width(stroke.width().get())
                                .with_line_cap(lyon::tessellation::LineCap::Round)
                                .with_line_join(lyon::tessellation::LineJoin::Round)
                                .with_tolerance(0.12),
                            &mut BuffersBuilder::new(&mut buffers, |v: StrokeVertex<'_, '_>| {
                                Point::new(v.position().x, v.position().y)
                            }),
                        )
                        .map_err(|e| format!("Invalid landmark stroke: {e:?}"))?;
                    emit(
                        buffers,
                        ink(stroke.paint())?,
                        alpha * stroke.opacity().get(),
                    );
                }
            }
            _ => return Err("Landmark artwork must be vector paths".into()),
        }
    }
    Ok(())
}
pub(super) fn parse_artwork(svg: &str, parts: &[LandmarkStep]) -> Result<Artwork, String> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|e| e.to_string())?;
    // Transparent wrapper groups are allowed; all visible paths must belong to one part.
    fn collect<'a>(
        group: &'a usvg::Group,
        alpha: f32,
        parts: &[LandmarkStep],
        out: &mut Vec<(&'a usvg::Group, f32)>,
    ) -> Result<(), String> {
        for node in group.children() {
            match node {
                usvg::Node::Group(g) if parts.iter().any(|p| p.id == g.id()) => {
                    out.push((g, alpha * group.opacity().get()))
                }
                usvg::Node::Group(g) => collect(g, alpha * group.opacity().get(), parts, out)?,
                _ => return Err("All landmark artwork must be inside named part groups".into()),
            }
        }
        Ok(())
    }
    let mut groups = Vec::new();
    collect(tree.root(), 1., parts, &mut groups)?;
    if groups
        .iter()
        .map(|(g, _)| g.id())
        .ne(parts.iter().map(|p| p.id.as_str()))
    {
        return Err("SVG part groups must match manifest drawing order".into());
    }
    let mut geometry = Vec::new();
    for (group, alpha) in groups {
        let mut shapes = Vec::new();
        visit(group, alpha, &mut shapes)?;
        if shapes.is_empty() || shapes.len() > 100_000 {
            return Err("Empty or oversized landmark part".into());
        }
        geometry.push(Arc::new(shapes));
    }
    let bounds = geometry
        .iter()
        .flat_map(|p| p.iter())
        .filter_map(Shape::bounds)
        .reduce(crate::document::ContentBounds::union)
        .ok_or("Empty artwork")?;
    let size = tree.size();
    if bounds.min.x < 0.
        || bounds.min.y < 0.
        || bounds.max.x > size.width()
        || bounds.max.y > size.height()
    {
        return Err("Artwork escapes its SVG viewport".into());
    }
    Ok(Artwork {
        parts: geometry,
        bounds,
    })
}
pub fn landmark_batches(landmark: &Landmark) -> Vec<LandmarkBatch> {
    landmark_batches_in(landmark, crate::document::AtlasStyle::Classic)
}

pub fn landmark_batches_in(
    landmark: &Landmark,
    style: crate::document::AtlasStyle,
) -> Vec<LandmarkBatch> {
    let ordinal = landmark_catalog().ordinal(&landmark.kind);
    let def = landmark_definition(&landmark.kind);
    let scale = landmark.size / def.width;
    let world = |p: Point| {
        Point::new(
            landmark.position.x + (p.x - def.anchor.x) * scale,
            landmark.position.y + (p.y - def.anchor.y) * scale,
        )
    };
    landmark_catalog().artwork[ordinal]
        .parts
        .iter()
        .enumerate()
        .map(|(index, local)| {
            let step = &landmark_definition(&landmark.kind).parts[index];
            LandmarkBatch {
                part: LandmarkPart {
                    landmark: landmark.clone(),
                    index,
                    pivot: world(Point::new(step.pivot[0], step.pivot[1])),
                },
                shapes: Arc::new(
                    local
                        .iter()
                        .map(|s| {
                            let Shape::Polygon {
                                points,
                                fill,
                                opacity,
                            } = s
                            else {
                                unreachable!()
                            };
                            Shape::Polygon {
                                points: points.iter().copied().map(world).collect(),
                                fill: if style.is_classic() {
                                    *fill
                                } else {
                                    super::palette::landmark(*fill)
                                },
                                opacity: *opacity,
                            }
                        })
                        .collect(),
                ),
            }
        })
        .collect()
}

pub fn landmark_bounds(landmark: &Landmark) -> crate::document::ContentBounds {
    let ordinal = landmark_catalog().ordinal(&landmark.kind);
    let b = landmark_catalog().artwork[ordinal].bounds;
    let def = landmark_definition(&landmark.kind);
    let scale = landmark.size / def.width;
    let world = |p: Point| {
        Point::new(
            landmark.position.x + (p.x - def.anchor.x) * scale,
            landmark.position.y + (p.y - def.anchor.y) * scale,
        )
    };
    crate::document::ContentBounds {
        min: world(b.min),
        max: world(b.max),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::Document,
        editor::Editor,
        scene::{SceneCache, build_scene, to_svg},
    };
    use std::collections::HashSet;
    fn ink_layers_at(kind: &str, point: Point) -> Vec<(&'static str, u32)> {
        let kind = LandmarkKind::from(kind);
        let definition = landmark_definition(&kind);
        landmark_batches(&Landmark {
            kind,
            position: Point::new(80., 104.),
            size: 160.,
            ..Default::default()
        })
        .into_iter()
        .filter_map(|batch| {
            let ink = batch.shapes.iter().rev().find_map(|shape| {
                let Shape::Polygon {
                    points,
                    fill,
                    opacity,
                } = shape
                else {
                    return None;
                };
                let [a, b, c] = points.as_slice() else {
                    return None;
                };
                let cross = |u: &Point, v: &Point| {
                    (v.x - u.x) * (point.y - u.y) - (v.y - u.y) * (point.x - u.x)
                };
                let signs = [cross(a, b), cross(b, c), cross(c, a)];
                let inside =
                    signs.iter().all(|s| *s >= -0.00001) || signs.iter().all(|s| *s <= 0.00001);
                (*opacity >= 0.2 && inside).then_some(*fill)
            })?;
            Some((definition.parts[batch.part.index].id.as_str(), ink))
        })
        .collect()
    }
    #[test]
    fn nearer_cottage_hides_rear_house_foundation() {
        let ink = ink_layers_at("town", Point::new(35.05, 90.4));
        assert!(ink.iter().any(|(id, _)| *id == "cottage-west-rear"));
        assert_eq!(
            ink.last(),
            Some(&("cottage-west-middle", 0xaa9877)),
            "A rear house foundation must not cut across the nearer cottage's window"
        );
    }
    #[test]
    fn warehouse_roof_hides_rear_crane_brace() {
        let ink = ink_layers_at("port", Point::new(113.5, 64.7));
        assert!(ink.iter().any(|(id, _)| *id == "crane"));
        assert_eq!(
            ink.last(),
            Some(&("warehouse-east", 0xd6c8aa)),
            "A rear crane brace must not cut across the warehouse roof"
        );
    }
    #[test]
    fn castle_tiers_are_balanced_and_increase_toward_the_center() {
        let batches = landmark_batches(&Landmark {
            kind: LandmarkKind::from("castle"),
            position: Point::new(80., 104.),
            size: 160.,
            ..Default::default()
        });
        let definition = landmark_definition(&LandmarkKind::from("castle"));
        let bounds = |id: &str| {
            let batch = batches
                .iter()
                .find(|b| definition.parts[b.part.index].id == id)
                .unwrap();
            batch
                .shapes
                .iter()
                .filter_map(Shape::bounds)
                .reduce(crate::document::ContentBounds::union)
                .unwrap()
        };
        let keep = bounds("keep");
        let shoulder = bounds("shoulder-left");
        let outer = bounds("tower-left");
        assert!(keep.min.y < shoulder.min.y && shoulder.min.y < outer.min.y);
        assert!(keep.height() > shoulder.height() && shoulder.height() > outer.height());
        let pivot = |id: &str| {
            batches
                .iter()
                .find(|b| definition.parts[b.part.index].id == id)
                .unwrap()
                .part
                .pivot
        };
        for (left_id, right_id) in [
            ("shoulder-left", "shoulder-right"),
            ("tower-left", "tower-right"),
        ] {
            let left = bounds(left_id);
            let right = bounds(right_id);
            assert!((left.height() - right.height()).abs() < 0.001);
            assert!((left.width() - right.width()).abs() < 0.001);
            // Cylinders and square keeps expose different side-face extents;
            // their foundation anchors define the shared architectural axis.
            assert!(((pivot(left_id).x + pivot(right_id).x) / 2. - pivot("keep").x).abs() < 0.001);
        }
    }
    #[test]
    fn every_registered_svg_part_has_geometry_valid_timing_and_an_exact_final_pose() {
        let registry = landmark_registry();
        assert!(!registry.is_empty());
        let mut kinds = HashSet::new();
        for def in registry {
            assert!(kinds.insert(def.kind.clone()));
            assert!(!def.name.is_empty() && !def.description.is_empty());
            let svg = usvg::Tree::from_str(landmark_svg(&def.kind), &usvg::Options::default())
                .expect("landmark SVG");
            // Canvas geometry and rasterized orbit previews must fit the same
            // SVG viewport; stray detail outside it otherwise renders differently.
            let local = landmark_bounds(&Landmark {
                kind: def.kind.clone(),
                position: def.anchor,
                size: def.width,
                ..Default::default()
            });
            assert!(
                local.min.x >= 0.
                    && local.min.y >= 0.
                    && local.max.x <= svg.size().width()
                    && local.max.y <= svg.size().height(),
                "Artwork escapes its SVG viewport: {:?}",
                def.kind
            );
            let landmark = Landmark {
                kind: def.kind.clone(),
                position: Point::new(-400., 250.),
                ..Default::default()
            };
            let batches = landmark_batches(&landmark);
            let bounds = landmark_bounds(&landmark);
            let mut ids = HashSet::new();
            for (part, batch) in def.parts.iter().zip(&batches) {
                assert!(ids.insert(&part.id));
                assert!(part.delay >= 0. && part.duration > 0. && part.delay + part.duration < 1.);
                assert!(!batch.shapes.is_empty(), "{} / {}", def.name, part.id);
                assert_eq!(landmark_pose(part, 2., false), [1., 1., 0., 0., 1.]);
                assert_eq!(landmark_pose(part, 0., true), [1., 1., 0., 0., 1.]);
                assert_eq!(landmark_pose(part, 0., false)[4], 0.);
                if matches!(part.motion, LandmarkMotion::Stamp) {
                    let [sx, sy, y, angle, opacity] =
                        landmark_pose(part, part.delay + part.duration * 0.5, false);
                    assert!((1.06..1.09).contains(&sx));
                    assert_eq!([sy, y, angle, opacity], [sx, 0., 0., 1.]);
                }
                for shape in batch.shapes.iter() {
                    let b = shape.bounds().unwrap();
                    assert!(
                        bounds.min.x <= b.min.x + 0.001
                            && bounds.min.y <= b.min.y + 0.001
                            && bounds.max.x >= b.max.x - 0.001
                            && bounds.max.y >= b.max.y - 0.001
                    );
                }
            }
        }
    }
    #[test]
    fn landmarks_round_trip_follow_layers_and_share_final_geometry_with_export() {
        let mut editor = Editor::blank();
        for (i, def) in landmark_registry().iter().enumerate() {
            editor
                .add_landmark(Landmark {
                    kind: def.kind.clone(),
                    position: Point::new(-300. + i as f32 * 180., -200.),
                    ..Default::default()
                })
                .unwrap();
        }
        let doc = editor.document();
        assert!(doc.validate().is_ok());
        let json = serde_json::to_string(doc).unwrap();
        assert_eq!(&serde_json::from_str::<Document>(&json).unwrap(), doc);
        assert!(!json.contains("duration") && !json.contains("arrivals"));
        let mut cache = SceneCache::default();
        let scene = cache.render(doc, None);
        assert_eq!(
            scene.shapes().cloned().collect::<Vec<_>>(),
            build_scene(doc)
        );
        assert!(scene == cache.committed_scene());
        let again = cache.render(doc, None);
        assert!(
            scene
                .batches()
                .zip(again.batches())
                .all(|(a, b)| Arc::ptr_eq(a, b))
        );
        let exported = to_svg(doc);
        assert_eq!(exported, to_svg(&serde_json::from_str(&json).unwrap()));
        assert!(exported.contains("#f1e5c9"));
        editor.set_layer_opacity(1, 0.4);
        let faded = cache.render(editor.document(), None);
        assert!(faded.batches().all(|b| faded.batch_opacity(b) == 0.4));
        editor.toggle_layer(1);
        assert_eq!(cache.render(editor.document(), None).shapes().count(), 0);
        assert_eq!(to_svg(editor.document()), to_svg(&Document::blank()));
    }
    #[test]
    fn legacy_documents_default_to_no_landmarks_and_identifiers_are_shared() {
        let mut legacy = serde_json::to_value(Document::blank()).unwrap();
        legacy["layers"][0]
            .as_object_mut()
            .unwrap()
            .remove("landmarks");
        let doc: Document = serde_json::from_value(legacy).unwrap();
        assert!(doc.layers[0].landmarks.is_empty());
        let mut editor = Editor::new(doc).unwrap();
        let id = editor.add_landmark(Landmark::default()).unwrap();
        editor.add_label(crate::document::Label::default()).unwrap();
        let mut invalid = editor.document().clone();
        invalid.layers[0].labels[0].id = id;
        assert!(invalid.validate().is_err());
        invalid = editor.document().clone();
        invalid.layers[0].landmarks[0].size = f32::NAN;
        assert!(invalid.validate().is_err());
    }
}
