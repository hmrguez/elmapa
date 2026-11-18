//! Shape text once, then share vector geometry with canvas, previews and export.
use super::Shape;
use crate::document::{Label, LabelFont, Point};
use lyon::{
    math::point,
    path::Path,
    tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers},
};
use std::sync::{Arc, Mutex, OnceLock};

fn options() -> &'static usvg::Options<'static> {
    static OPTIONS: OnceLock<usvg::Options<'static>> = OnceLock::new();
    OPTIONS.get_or_init(|| {
        let mut options = usvg::Options::default();
        #[cfg(target_os = "macos")]
        for path in [
            "/System/Library/Fonts/Supplemental/Baskerville.ttc",
            "/System/Library/Fonts/Supplemental/Didot.ttc",
            "/System/Library/Fonts/Avenir Next.ttc",
        ] {
            let _ = options.fontdb_mut().load_font_file(path);
        }
        // Supply coverage for names outside the Latin presets without replacing
        // the selected font for supported glyphs.
        options.fontdb_mut().load_system_fonts();
        options
    })
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn paths(group: &usvg::Group, out: &mut Vec<Path>) {
    use usvg::tiny_skia_path::PathSegment;
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => paths(g, out),
            usvg::Node::Text(t) => paths(t.flattened(), out),
            usvg::Node::Path(p) => {
                let mut builder = Path::builder();
                let convert = |mut q: usvg::tiny_skia_path::Point| {
                    p.abs_transform().map_point(&mut q);
                    point(q.x, q.y)
                };
                for segment in p.data().segments() {
                    match segment {
                        PathSegment::MoveTo(q) => {
                            builder.begin(convert(q));
                        }
                        PathSegment::LineTo(q) => {
                            builder.line_to(convert(q));
                        }
                        PathSegment::QuadTo(a, q) => {
                            builder.quadratic_bezier_to(convert(a), convert(q));
                        }
                        PathSegment::CubicTo(a, b, q) => {
                            builder.cubic_bezier_to(convert(a), convert(b), convert(q));
                        }
                        PathSegment::Close => {
                            builder.end(true);
                        }
                    }
                }
                out.push(builder.build());
            }
            _ => {}
        }
    }
}
fn text_tree(text: &str, definitions: &str) -> Option<usvg::Tree> {
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="65536" height="1024">{definitions}{text}</svg>"#
    );
    usvg::Tree::from_str(&svg, options()).ok()
}

fn font_candidates(font: LabelFont) -> &'static [&'static str] {
    match font {
        LabelFont::Baskerville => &["Baskerville", "Georgia", "Liberation Serif", "DejaVu Serif"],
        LabelFont::Didot => &[
            "Didot",
            "Times New Roman",
            "DejaVu Serif",
            "Liberation Serif",
        ],
        LabelFont::Avenir => &["Avenir Next", "Segoe UI", "DejaVu Sans", "Liberation Sans"],
    }
}

/// Load the same installed font family used to shape the map's vector lettering.
pub fn label_font_bytes(font: LabelFont) -> Option<Vec<u8>> {
    use usvg::fontdb::{Family, Query};
    let database = &options().fontdb;
    let families: Vec<_> = font_candidates(font)
        .iter()
        .map(|name| Family::Name(name))
        .collect();
    let id = database.query(&Query {
        families: &families,
        ..Default::default()
    })?;
    database.with_face_data(id, |bytes, _| bytes.to_vec())
}

fn text_attributes(label: &Label) -> String {
    format!(
        r#"xml:space="preserve" font-family="{}" font-size="{}" letter-spacing="{}""#,
        font_candidates(label.font).join(", "),
        label.size,
        label.spacing
    )
}

fn straight_tree(text: &str, attributes: &str) -> Option<usvg::Tree> {
    text_tree(
        &format!(r#"<text x="0" y="512" {attributes}>{text}</text>"#),
        "",
    )
}

fn advance(tree: &usvg::Tree) -> Option<f32> {
    tree.root().children().iter().find_map(|node| {
        if let usvg::Node::Text(text) = node {
            Some(text.bounding_box().width())
        } else {
            None
        }
    })
}

fn label_tree(label: &Label) -> Option<usvg::Tree> {
    let attributes = text_attributes(label);
    let text = escape(&label.text);
    let straight = straight_tree(&text, &attributes)?;
    if label.curvature == 0. {
        return Some(straight);
    }

    // Measure the shaped advance, including kerning, spaces and fallback glyphs.
    let width = advance(&straight)?;
    // Approximate a circular arc with a symmetric cubic, with the requested sweep
    // over the measured advance. Tangent extensions keep end clusters on the path
    // even when their glyph width is greater than their advance.
    let half_angle = f64::from(label.curvature).to_radians() / 2.;
    let radius = f64::from(width.max(label.size)) / (2. * half_angle);
    let (sin, cos) = half_angle.sin_cos();
    let x = radius * sin;
    let handle = (4. / 3.) * radius * (half_angle / 2.).tan();
    let control_x = x - handle * cos;
    let control_y = -handle * sin;
    let padding = f64::from(label.size);
    let end_x = x + padding * cos;
    let end_y = padding * sin;
    let defs = format!(
        r#"<defs><path id="baseline" d="M {} {end_y} L {} 0 C {} {control_y} {control_x} {control_y} {x} 0 L {end_x} {end_y}"/></defs>"#,
        -end_x, -x, -control_x
    );
    text_tree(
        &format!(
            r##"<text {attributes} text-anchor="middle"><textPath xlink:href="#baseline" startOffset="50%">{text}</textPath></text>"##
        ),
        &defs,
    )
}

/// A caret in world coordinates and its tilt relative to the upright label.
pub fn label_caret(label: &Label, byte: usize) -> (Point, f32) {
    let Some(prefix) = label.text.get(..byte) else {
        return (label.position, label.rotation);
    };
    let attributes = text_attributes(label);
    let measure = |text: &str| {
        straight_tree(&escape(text), &attributes)
            .as_ref()
            .and_then(advance)
            .unwrap_or(0.)
    };
    let width = measure(&label.text);
    if width <= 0. {
        return (label.position, label.rotation);
    }
    let t = (measure(prefix) / width).clamp(0., 1.);
    let half = label.curvature.to_radians() / 2.;
    let tangent = (2. * t - 1.) * half;
    let (x, y) = if half.abs() < 0.0001 {
        ((t - 0.5) * width, 0.)
    } else {
        let radius = width.max(label.size) / (2. * half);
        (
            radius * tangent.sin(),
            radius * ((1. + half.cos()) / 2. - tangent.cos()),
        )
    };
    let (sin, cos) = label.rotation.to_radians().sin_cos();
    (
        Point::new(
            label.position.x + x * cos - y * sin,
            label.position.y + x * sin + y * cos,
        ),
        label.rotation + tangent.to_degrees(),
    )
}

fn local_shapes(label: &Label) -> Vec<Shape> {
    let Some(tree) = label_tree(label) else {
        return Vec::new();
    };
    let mut outlines = Vec::new();
    paths(tree.root(), &mut outlines);
    let mut buffers: VertexBuffers<Point, u32> = VertexBuffers::new();
    let mut tessellator = FillTessellator::new();
    for path in outlines {
        if tessellator
            .tessellate_path(
                &path,
                &FillOptions::default().with_tolerance(0.08),
                &mut BuffersBuilder::new(&mut buffers, |v: FillVertex<'_>| {
                    Point::new(v.position().x, v.position().y)
                }),
            )
            .is_err()
        {
            return Vec::new();
        }
    }
    let Some(first) = buffers.vertices.first().copied() else {
        return Vec::new();
    };
    let (min, max) = buffers
        .vertices
        .iter()
        .fold((first, first), |(min, max), p| {
            (
                Point::new(min.x.min(p.x), min.y.min(p.y)),
                Point::new(max.x.max(p.x), max.y.max(p.y)),
            )
        });
    let center = Point::new((min.x + max.x) / 2., (min.y + max.y) / 2.);
    buffers
        .indices
        .chunks_exact(3)
        .map(|triangle| Shape::Polygon {
            points: triangle
                .iter()
                .map(|&i| {
                    let p = buffers.vertices[i as usize];
                    Point::new(p.x - center.x, p.y - center.y)
                })
                .collect(),
            fill: label.ink.color(),
            opacity: label.opacity,
        })
        .collect()
}
type CachedLabels = Vec<(Label, Arc<Vec<Shape>>)>;
/// Bounded cache also makes exact bounds / hit tests independent of font shaping.
/// Position and angle are applied after shaping, so dragging reuses glyph data.
pub fn label_shapes(label: &Label) -> Arc<Vec<Shape>> {
    static CACHE: OnceLock<Mutex<CachedLabels>> = OnceLock::new();
    static WORLD: OnceLock<Mutex<CachedLabels>> = OnceLock::new();
    let world = WORLD.get_or_init(Default::default);
    if let Some((_, shapes)) = world
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(l, _)| l == label)
    {
        return shapes.clone();
    }
    let mut key = label.clone();
    key.id = 0;
    key.position = Point::new(0., 0.);
    key.rotation = 0.;
    let cache = CACHE.get_or_init(Default::default);
    let found = cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .find(|(l, _)| l == &key)
        .map(|(_, s)| s.clone());
    let local = found.unwrap_or_else(|| {
        let shapes = Arc::new(local_shapes(&key));
        let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
        if cache.len() >= 32 {
            cache.remove(0);
        }
        cache.push((key, shapes.clone()));
        shapes
    });
    let (sin, cos) = label.rotation.to_radians().sin_cos();
    let shapes = Arc::new(
        local
            .iter()
            .map(|shape| {
                let Shape::Polygon {
                    points,
                    fill,
                    opacity,
                } = shape
                else {
                    unreachable!()
                };
                Shape::Polygon {
                    points: points
                        .iter()
                        .map(|p| {
                            Point::new(
                                label.position.x + p.x * cos - p.y * sin,
                                label.position.y + p.x * sin + p.y * cos,
                            )
                        })
                        .collect(),
                    fill: *fill,
                    opacity: *opacity,
                }
            })
            .collect(),
    );
    let mut world = world.lock().unwrap_or_else(|e| e.into_inner());
    if world.len() >= 32 {
        world.remove(0);
    }
    world.push((label.clone(), Arc::clone(&shapes)));
    shapes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{Document, LabelFont},
        editor::Editor,
        scene::{SceneCache, to_svg},
    };
    #[test]
    fn font_outlines_keep_counters_and_export_without_installed_font_dependencies() {
        let mut editor = Editor::blank();
        let id = editor
            .add_label(Label {
                text: "O".into(),
                size: 100.,
                position: Point::new(100., 100.),
                ..Label::default()
            })
            .unwrap();
        let shapes = label_shapes(editor.label(id).unwrap());
        assert!(!shapes.is_empty());
        let center_is_ink = shapes.iter().any(|s| {
            let Shape::Polygon { points, .. } = s else {
                unreachable!()
            };
            let p = Point::new(100., 100.);
            let cross = |a: Point, b: Point| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
            let signs = [
                cross(points[0], points[1]),
                cross(points[1], points[2]),
                cross(points[2], points[0]),
            ];
            signs.iter().all(|v| *v >= 0.) || signs.iter().all(|v| *v <= 0.)
        });
        assert!(!center_is_ink, "The counter of O must stay open");
        let svg = to_svg(editor.document());
        assert!(svg.contains("polygon"));
        assert!(!svg.contains("<text"));
        let mut bytes = Vec::new();
        crate::scene::write_svg(editor.document(), &mut bytes).unwrap();
        assert_eq!(bytes, svg.as_bytes());
        let mut label = editor.label(id).unwrap().clone();
        label.text = "Éire <&> 東京".into();
        label.rotation = 35.;
        for font in LabelFont::ALL {
            label.font = font;
            assert!(editor.update_label(label.clone()));
            let scene = SceneCache::default().render(editor.document(), None);
            assert!(scene.shapes().count() > 20);
            assert!(label.contains(label.position, 2.));
            let bounds = label.bounds();
            for point in scene.shapes().flat_map(Shape::points) {
                assert!(
                    point.x >= bounds.min.x
                        && point.x <= bounds.max.x
                        && point.y >= bounds.min.y
                        && point.y <= bounds.max.y
                );
            }
        }
    }
    #[test]
    fn curved_baselines_arch_in_both_directions_and_share_export_geometry() {
        let mut editor = Editor::blank();
        let id = editor
            .add_label(Label {
                text: "HHHHHHHHH".into(),
                size: 48.,
                spacing: 4.,
                ..Label::default()
            })
            .unwrap();
        let mut label = editor.label(id).unwrap().clone();
        let straight_height = label.bounds().height();
        let straight_svg = to_svg(editor.document());
        for curvature in [-100., 100.] {
            label.curvature = curvature;
            assert!(editor.update_label(label.clone()));
            let bounds = label.bounds();
            assert!(bounds.height() > straight_height * 2.);
            let middle_top = label_shapes(&label)
                .iter()
                .flat_map(Shape::points)
                .filter(|p| p.x.abs() < label.size / 8.)
                .map(|p| p.y)
                .reduce(f32::min)
                .unwrap();
            if curvature > 0. {
                assert!((middle_top - bounds.min.y).abs() < 1.);
            } else {
                assert!(middle_top - bounds.min.y > straight_height);
            }
            assert_ne!(to_svg(editor.document()), straight_svg);
        }
    }
    #[test]
    fn labels_follow_layer_visibility_opacity_order_and_reuse_unchanged_meshes() {
        let mut e = Editor::blank();
        e.add_label(Label::default()).unwrap();
        let mut cache = SceneCache::default();
        let first = cache.render(e.document(), None);
        assert!(cache.committed_scene() == first);
        let batch = first.batches().find(|b| first.label(b).is_some()).unwrap();
        let again = cache.render(e.document(), None);
        assert!(again.batches().any(|b| Arc::ptr_eq(b, batch)));
        e.set_layer_opacity(1, 0.4);
        let scene = cache.render(e.document(), None);
        assert_eq!(scene.batch_opacity(batch), 0.4);
        let upper = e.add_layer();
        let id = e
            .add_label(Label {
                text: "Upper".into(),
                ..Label::default()
            })
            .unwrap();
        let scene = cache.render(e.document(), None);
        assert_eq!(scene.label(scene.batches().last().unwrap()).unwrap().id, id);
        e.toggle_layer(upper);
        let scene = cache.render(e.document(), None);
        assert_eq!(
            scene.batches().filter(|b| scene.label(b).is_some()).count(),
            1
        );
        e.toggle_layer(1);
        assert_eq!(cache.render(e.document(), None).shapes().count(), 0);
        assert_eq!(to_svg(e.document()), to_svg(&Document::blank()));
    }
}
