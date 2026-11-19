//! Parse the shared forest vectors for geometry consumers.
use crate::{
    document::Point,
    scene::{
        Shape,
        geometry::p,
        shape::{line, polygon},
    },
};
use std::sync::OnceLock;
use usvg::{Node, Paint, tiny_skia_path::PathSegment};

pub(super) fn specimens(
    style: crate::document::AtlasStyle,
) -> &'static [Vec<Shape>; super::FOREST_SPECIMEN_COUNT] {
    static SPECIMENS: OnceLock<[Vec<Shape>; super::FOREST_SPECIMEN_COUNT]> = OnceLock::new();
    static WOODLAND: OnceLock<[Vec<Shape>; super::FOREST_SPECIMEN_COUNT]> = OnceLock::new();
    let specimens = if style.is_classic() {
        &SPECIMENS
    } else {
        &WOODLAND
    };
    specimens.get_or_init(|| {
        std::array::from_fn(|variant| {
            let tree = super::forest_specimen_tree(variant, style);
            let mut shapes = Vec::new();
            visit(tree.root(), 1., &mut shapes);
            // usvg resolves the root viewBox to positive viewport coordinates.
            for shape in &mut shapes {
                let points = match shape {
                    Shape::Polygon { points, .. } | Shape::Line { points, .. } => points,
                    Shape::RegionSurface { .. } | Shape::Sprite { .. } => {
                        unreachable!("forest specimens contain vector shapes only")
                    }
                };
                for q in points {
                    q.x -= 50.;
                    q.y -= 50.;
                }
            }
            shapes
        })
    })
}

fn color(paint: &Paint) -> u32 {
    let Paint::Color(c) = paint else {
        unreachable!("specimens use solid ink")
    };
    (u32::from(c.red) << 16) | (u32::from(c.green) << 8) | u32::from(c.blue)
}

fn visit(group: &usvg::Group, opacity: f32, shapes: &mut Vec<Shape>) {
    let opacity = opacity * group.opacity().get();
    for node in group.children() {
        match node {
            Node::Group(group) => visit(group, opacity, shapes),
            Node::Path(path) => {
                let data = path
                    .data()
                    .clone()
                    .transform(path.abs_transform())
                    .expect("tree transform");
                let mut points = Vec::new();
                let flush = |points: &mut Vec<Point>, shapes: &mut Vec<Shape>| {
                    if let Some(fill) = path.fill() {
                        polygon(
                            shapes,
                            points.clone(),
                            color(fill.paint()),
                            opacity * fill.opacity().get(),
                        );
                    }
                    if let Some(stroke) = path.stroke() {
                        line(
                            shapes,
                            points.clone(),
                            color(stroke.paint()),
                            stroke.width().get(),
                            opacity * stroke.opacity().get(),
                        );
                    }
                    points.clear();
                };
                for segment in data.segments() {
                    match segment {
                        PathSegment::MoveTo(q) => {
                            flush(&mut points, shapes);
                            points.push(p(q.x, q.y));
                        }
                        PathSegment::LineTo(q) => points.push(p(q.x, q.y)),
                        PathSegment::QuadTo(b, c) => {
                            let a = *points.last().expect("curve origin");
                            for i in 1..=4 {
                                let t = i as f32 / 4.;
                                let u = 1. - t;
                                points.push(p(
                                    u * u * a.x + 2. * u * t * b.x + t * t * c.x,
                                    u * u * a.y + 2. * u * t * b.y + t * t * c.y,
                                ));
                            }
                        }
                        PathSegment::CubicTo(b, c, d) => {
                            let a = *points.last().expect("curve origin");
                            for i in 1..=4 {
                                let t = i as f32 / 4.;
                                let u = 1. - t;
                                points.push(p(
                                    u * u * u * a.x
                                        + 3. * u * u * t * b.x
                                        + 3. * u * t * t * c.x
                                        + t * t * t * d.x,
                                    u * u * u * a.y
                                        + 3. * u * u * t * b.y
                                        + 3. * u * t * t * c.y
                                        + t * t * t * d.y,
                                ));
                            }
                        }
                        PathSegment::Close => {
                            if let Some(first) = points.first().copied() {
                                points.push(first);
                            }
                        }
                    }
                }
                flush(&mut points, shapes);
            }
            _ => unreachable!("specimens contain only vector paths"),
        }
    }
}
