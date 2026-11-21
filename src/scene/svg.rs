use super::granulation;
use super::{
    FOREST_SPECIMEN_COUNT, Scene, SceneCache, SceneRenderer, Shape, forest_specimen_svg, palette,
    paper_detail,
};
use crate::{
    brush_materials,
    document::{AtlasStyle, Brush, ContentBounds, Document, Point},
};
use base64::engine::general_purpose::STANDARD;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

struct SvgRenderer<W> {
    svg: W,
    opacity: f32,
    style: AtlasStyle,
    pigment_patterns: bool,
}

impl<W: std::fmt::Write> SceneRenderer for SvgRenderer<W> {
    fn render_shape(&mut self, shape: &Shape) {
        let layer_opacity = self.opacity;
        match shape {
            Shape::Polygon {
                points,
                fill,
                opacity,
            } => {
                let opacity = opacity * layer_opacity;
                let _ = write!(
                    self.svg,
                    "<polygon fill=\"#{fill:06x}\" opacity=\"{opacity:.3}\" points=\""
                );
                for q in points {
                    let _ = write!(self.svg, "{:.2},{:.2} ", q.x, q.y);
                }
                let _ = self.svg.write_str("\"/>\n");
            }
            Shape::RegionSurface {
                contours,
                material,
                granulated,
                fill,
                opacity,
            } => {
                self.render_surface(
                    contours.iter().map(Vec::as_slice),
                    *material,
                    *granulated,
                    *fill,
                    *opacity,
                );
            }
            Shape::ShoreWash(wash) => {
                let (fill, opacity) = wash.svg_color();
                self.render_surface(
                    wash.contours.iter().map(Vec::as_slice),
                    None,
                    false,
                    fill,
                    opacity,
                );
            }
            Shape::Sprite {
                points,
                material,
                frame,
                opacity,
            } => {
                let opacity = opacity * layer_opacity;
                let width = points[1].x - points[0].x;
                let height = points[3].y - points[0].y;
                let (x, y) = (points[0].x, points[0].y);
                let _ = writeln!(
                    self.svg,
                    "<svg x=\"{x:.4}\" y=\"{y:.4}\" width=\"{width:.4}\" height=\"{height:.4}\" viewBox=\"{frame} 0 1 1\" preserveAspectRatio=\"none\" overflow=\"hidden\" opacity=\"{opacity:.3}\"><use href=\"#mountain-sprites-{material}\"/></svg>",
                );
            }
            Shape::Line {
                points,
                color,
                width,
                opacity,
            } => {
                let opacity = opacity * layer_opacity;
                let _ = write!(
                    self.svg,
                    "<polyline fill=\"none\" stroke=\"#{color:06x}\" stroke-width=\"{width:.2}\" opacity=\"{opacity:.3}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" points=\""
                );
                for q in points {
                    let _ = write!(self.svg, "{:.2},{:.2} ", q.x, q.y);
                }
                let _ = self.svg.write_str("\"/>\n");
            }
        }
    }
}

fn pattern_id(index: usize) -> String {
    let material = &brush_materials::artwork_catalog()[index];
    if material.brush == Brush::Land {
        format!("land-material-{index}")
    } else {
        format!("water-material-{}", material.id)
    }
}

/// Export the same terrain geometry painted by the native canvas.
pub fn to_svg(doc: &Document) -> String {
    to_svg_in_bounds(doc, doc.content_bounds())
}

pub(super) fn to_svg_in_bounds(doc: &Document, bounds: ContentBounds) -> String {
    render_svg(
        &SceneCache::default().render(doc, None),
        bounds,
        String::new(),
    )
}

fn write_svg_open(svg: &mut impl std::fmt::Write, bounds: ContentBounds) {
    let _ = writeln!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\"><title>Elmapa — original procedural terrain</title>",
        bounds.width(),
        bounds.height(),
        bounds.min.x,
        bounds.min.y,
        bounds.width(),
        bounds.height()
    );
}

fn write_background(svg: &mut impl std::fmt::Write, bounds: ContentBounds, style: AtlasStyle) {
    let paper = style.paper();
    let _ = writeln!(
        svg,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#{paper:06x}\"/>",
        bounds.min.x,
        bounds.min.y,
        bounds.width(),
        bounds.height()
    );
}

fn write_definitions(
    svg: &mut impl std::fmt::Write,
    scene: &Scene,
    include_trees: bool,
    pigment_patterns: bool,
) {
    // SVG instances and the native sprite atlas share the same specimen artwork.
    let mut used = [false; FOREST_SPECIMEN_COUNT];
    if include_trees {
        for tree in scene.forest_trees() {
            used[tree.variant] = true;
        }
    }
    let _ = svg.write_str("<defs>");
    for (variant, _) in used.into_iter().enumerate().filter(|(_, used)| *used) {
        let _ = write!(
            svg,
            "<g id=\"forest-{variant}\">{}</g>",
            forest_specimen_svg(variant, scene.style())
        );
    }
    let mut used_materials = vec![false; brush_materials::artwork_catalog().len()];
    let mut pigment_colors = BTreeSet::new();
    for batch in scene.batches() {
        if let Some(index) = batch.first().and_then(Shape::texture_index) {
            used_materials[index] = true;
        }
        if pigment_patterns {
            pigment_colors.extend(batch.iter().filter_map(|shape| match shape {
                Shape::RegionSurface {
                    fill,
                    granulated: true,
                    ..
                } => Some(*fill),
                _ => None,
            }));
        }
    }
    write_granulation(svg, scene.style(), pigment_colors);
    for (index, material) in brush_materials::artwork_catalog().iter().enumerate() {
        let Some(texture) = material.texture.filter(|_| used_materials[index]) else {
            continue;
        };
        let data = base64::display::Base64Display::new(texture, &STANDARD);
        if let Some(sheet) = material.sprites {
            let _ = write!(
                svg,
                "<image id=\"mountain-sprites-{index}\" width=\"{}\" height=\"1\" preserveAspectRatio=\"none\" href=\"data:image/png;base64,{data}\"/>",
                sheet.frames,
            );
        } else {
            let size = material.tile_size;
            let id = pattern_id(index);
            let _ = write!(
                svg,
                "<pattern id=\"{id}\" patternUnits=\"userSpaceOnUse\" width=\"{size}\" height=\"{size}\"><image width=\"{size}\" height=\"{size}\" href=\"data:image/png;base64,{data}\"/></pattern>",
            );
        }
    }
    let _ = svg.write_str("</defs>");
}

fn write_granulation(svg: &mut impl std::fmt::Write, style: AtlasStyle, colors: BTreeSet<u32>) {
    if colors.is_empty() {
        return;
    }
    let size = granulation::WORLD_SIZE;
    let data = base64::display::Base64Display::new(granulation::svg_overlay(style), &STANDARD);
    let _ = write!(
        svg,
        "<image id=\"pigment-granulation\" width=\"{size}\" height=\"{size}\" href=\"data:image/png;base64,{data}\"/>"
    );
    for color in colors {
        let _ = write!(
            svg,
            "<pattern id=\"pigment-{color:06x}\" patternUnits=\"userSpaceOnUse\" width=\"{size}\" height=\"{size}\"><rect width=\"{size}\" height=\"{size}\" fill=\"#{color:06x}\"/><use href=\"#pigment-granulation\"/></pattern>"
        );
    }
}

impl<W: std::fmt::Write> SvgRenderer<W> {
    fn render_surface<'a>(
        &mut self,
        contours: impl IntoIterator<Item = &'a [Point]>,
        material: Option<usize>,
        granulated: bool,
        fill: u32,
        opacity: f32,
    ) {
        let opacity = opacity * self.opacity;
        let fill = if let Some(index) = material {
            format!("url(#{})", pattern_id(index))
        } else if granulated && self.pigment_patterns {
            format!("url(#pigment-{fill:06x})")
        } else {
            format!("#{fill:06x}")
        };
        let _ = write!(
            self.svg,
            "<path fill=\"{fill}\" fill-rule=\"evenodd\" opacity=\"{opacity:.3}\" d=\""
        );
        for ring in contours {
            let _ = self.svg.write_str("M");
            for q in ring {
                let _ = write!(self.svg, "{:.4},{:.4} ", q.x, q.y);
            }
            let _ = self.svg.write_str("Z");
        }
        let _ = self.svg.write_str("\"/>\n");
    }

    fn render_terrain<'a>(
        &mut self,
        batches: impl IntoIterator<Item = &'a Arc<Vec<Shape>>>,
    ) -> usize {
        type Style = (usize, Option<usize>, bool, u32, u32);
        let mut surfaces: BTreeMap<Style, Vec<&[Point]>> = BTreeMap::new();
        for batch in batches {
            for shape in batch.iter() {
                let (contours, material, granulated, fill, opacity) = match shape {
                    Shape::RegionSurface {
                        contours,
                        material,
                        granulated,
                        fill,
                        opacity,
                    } => (contours.as_slice(), *material, *granulated, *fill, *opacity),
                    Shape::Polygon {
                        points,
                        fill,
                        opacity,
                    } => (std::slice::from_ref(points), None, false, *fill, *opacity),
                    Shape::ShoreWash(wash) => {
                        let (fill, opacity) = wash.svg_color();
                        if opacity >= 0.001 {
                            surfaces
                                .entry((2, None, false, fill, opacity.to_bits()))
                                .or_default()
                                .extend(wash.contours.iter().map(Vec::as_slice));
                        }
                        continue;
                    }
                    _ => unreachable!("tiled terrain is clipped into filled contours"),
                };
                let order = if material.is_some() {
                    0
                } else if fill == palette::land(self.style) || fill == palette::water(self.style) {
                    1
                } else {
                    3
                };
                surfaces
                    .entry((order, material, granulated, fill, opacity.to_bits()))
                    .or_default()
                    .extend(contours.iter().map(Vec::as_slice));
            }
        }
        // Adjacent tiles of one pigment share an SVG path. Rasterizers then
        // antialias its outer edge without blending two half-covered tiles.
        let nodes = surfaces.len();
        for ((_, material, granulated, fill, opacity), contours) in surfaces {
            self.render_surface(
                contours,
                material,
                granulated,
                fill,
                f32::from_bits(opacity),
            );
        }
        nodes
    }

    fn render_tree(&mut self, tree: &super::ForestTree) {
        let _ = writeln!(
            self.svg,
            "<use href=\"#forest-{}\" transform=\"translate({} {}) rotate({}) scale({})\" opacity=\"{}\"/>",
            tree.variant, tree.center.x, tree.center.y, tree.rotation, tree.scale, tree.opacity
        );
    }
}

enum Artwork<'a> {
    Batch(&'a Arc<Vec<Shape>>),
    Terrain(Vec<&'a Arc<Vec<Shape>>>),
}

fn artwork(scene: &Scene) -> impl Iterator<Item = Artwork<'_>> {
    let mut batches = scene.batches().peekable();
    std::iter::from_fn(move || {
        let first = batches.next()?;
        let Some((layer, brush, _)) = scene.region(first) else {
            return Some(Artwork::Batch(first));
        };
        let mut tiles = vec![first];
        while batches.peek().is_some_and(|batch| {
            scene
                .region(batch)
                .is_some_and(|(id, tool, _)| (id, tool) == (layer, brush))
        }) {
            tiles.push(batches.next().expect("peeked terrain batch"));
        }
        Some(Artwork::Terrain(tiles))
    })
}

pub(super) fn render_svg<W: std::fmt::Write>(
    scene: &Scene,
    bounds: ContentBounds,
    mut svg: W,
) -> W {
    let _span = crate::performance::Span::new("svg_generation");
    write_svg_open(&mut svg, bounds);
    write_background(&mut svg, bounds, scene.style());
    write_definitions(&mut svg, scene, true, true);
    let mut renderer = SvgRenderer {
        svg,
        opacity: 1.,
        style: scene.style(),
        pigment_patterns: true,
    };
    for item in artwork(scene) {
        if super::generation::is_stopped() {
            break;
        }
        match item {
            Artwork::Terrain(tiles) => {
                renderer.opacity = scene.batch_opacity(tiles[0]);
                renderer.render_terrain(tiles);
            }
            Artwork::Batch(batch) => {
                if let Some(tree) = scene.forest_tree(batch) {
                    renderer.render_tree(tree);
                } else {
                    renderer.opacity = scene.batch_opacity(batch);
                    renderer.render_scene(batch);
                }
            }
        }
    }
    renderer.opacity = 1.;
    renderer.render_scene(&paper_detail(bounds));
    let _ = renderer.svg.write_str("</svg>\n");
    renderer.svg
}

/// Send preview geometry in painter order, optionally drawing forest instances
/// outside SVG so the parser does not expand each specimen into hundreds of nodes.
pub(super) fn render_preview_parts(
    scene: &Scene,
    bounds: ContentBounds,
    raster_trees: bool,
    pixel_scale: f32,
    mut render: impl FnMut(PreviewPart<'_>) -> Result<(), String>,
) -> Result<(), String> {
    // Keep each parsed fragment comfortably below usvg's one-million-node cap.
    const PART_NODE_BUDGET: usize = 100_000;
    let tree_cost: [usize; FOREST_SPECIMEN_COUNT] = std::array::from_fn(|variant| {
        2 + forest_specimen_svg(variant, scene.style())
            .matches('<')
            .count()
    });
    // resvg rasterizes a full pattern tile before clipping it to the viewport.
    // Use solid pigment for tightly cropped and extremely distant snapshots.
    let pigment_patterns = (8.0..=1024.0).contains(&(granulation::WORLD_SIZE * pixel_scale));
    let mut renderer = SvgRenderer {
        svg: String::new(),
        opacity: 1.,
        style: scene.style(),
        pigment_patterns,
    };
    write_svg_open(&mut renderer.svg, bounds);
    write_definitions(&mut renderer.svg, scene, !raster_trees, pigment_patterns);
    let header = renderer.svg.len();
    write_background(&mut renderer.svg, bounds, scene.style());
    let mut nodes = 1;
    let flush = |svg: &mut String,
                 render: &mut dyn FnMut(PreviewPart<'_>) -> Result<(), String>| {
        if svg.len() > header {
            svg.push_str("</svg>\n");
            render(PreviewPart::Svg(svg))?;
            svg.truncate(header);
        }
        Ok::<_, String>(())
    };
    for item in artwork(scene) {
        match item {
            Artwork::Terrain(tiles) => {
                renderer.opacity = scene.batch_opacity(tiles[0]);
                nodes += renderer.render_terrain(tiles);
            }
            Artwork::Batch(batch) => {
                if let Some(tree) = scene.forest_tree(batch) {
                    if raster_trees {
                        flush(&mut renderer.svg, &mut render)?;
                        render(PreviewPart::Tree(tree))?;
                        nodes = 0;
                    } else {
                        nodes += tree_cost[tree.variant];
                        renderer.render_tree(tree);
                    }
                } else {
                    renderer.opacity = scene.batch_opacity(batch);
                    for shape in batch.iter() {
                        renderer.render_shape(shape);
                        nodes += 1;
                        if nodes >= PART_NODE_BUDGET {
                            flush(&mut renderer.svg, &mut render)?;
                            nodes = 0;
                        }
                    }
                }
            }
        }
        if nodes >= PART_NODE_BUDGET {
            flush(&mut renderer.svg, &mut render)?;
            nodes = 0;
        }
    }
    renderer.opacity = 1.;
    renderer.render_scene(&paper_detail(bounds));
    flush(&mut renderer.svg, &mut render)
}

pub(super) enum PreviewPart<'a> {
    Svg(&'a str),
    Tree(&'a super::ForestTree),
}

/// Stream self-contained SVG without retaining the serialized output string.
/// The caller owns atomic replacement and optional generation cancellation.
pub fn write_svg(doc: &Document, writer: impl std::io::Write) -> Result<(), String> {
    struct Sink<W> {
        writer: W,
        error: Option<std::io::Error>,
        bytes: usize,
    }
    impl<W: std::io::Write> std::fmt::Write for Sink<W> {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            if self.error.is_some() {
                return Err(std::fmt::Error);
            }
            self.bytes = self.bytes.saturating_add(text.len());
            if self.bytes > 256 * 1024 * 1024 {
                self.error = Some(std::io::Error::other(
                    "SVG exceeds the 256 MiB output limit.",
                ));
                return Err(std::fmt::Error);
            }
            if let Err(error) = self.writer.write_all(text.as_bytes()) {
                self.error = Some(error);
                return Err(std::fmt::Error);
            }
            Ok(())
        }
    }
    let scene = SceneCache::default().render(doc, None);
    let mut sink = render_svg(
        &scene,
        doc.content_bounds(),
        Sink {
            writer,
            error: None,
            bytes: 0,
        },
    );
    if let Some(error) = sink.error {
        return Err(error.to_string());
    }
    sink.writer.flush().map_err(|e| e.to_string())?;
    crate::performance::counter("export_bytes", sink.bytes as u64);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flat_pigment_has_tonal_variation_in_the_shelf_preview() {
        use crate::{document::Stroke, library::Camera};

        let mut document = Document::blank();
        document.layers[0].strokes.push(Stroke {
            brush: Brush::Land,
            creation_id: None,
            material: None,
            points: vec![Point::new(-160., 0.), Point::new(360., 0.)].into(),
            size: 460.,
            strength: 1.,
            seed: 471,
        });
        let camera = Camera {
            center: Point::new(192., 0.),
            width: 740.,
            height: 470.,
        };
        let pixels = super::super::thumbnail::thumbnail(&document, camera).unwrap();
        // Sample well inside the land wash, on both sides of a terrain tile join.
        let red = |x: usize, y: usize| pixels[(y * 512 + x) * 4 + 2];
        let mut pigments = BTreeSet::new();
        for y in 140..180 {
            for x in 220..292 {
                pigments.insert(red(x, y));
            }
            assert!(red(255, y).abs_diff(red(256, y)) < 3);
        }
        assert!(pigments.len() > 2);
        assert!(pigments.iter().all(|red| (233..243).contains(red)));

        let close_up = Camera {
            width: 64.,
            height: 40.,
            ..camera
        };
        let pixels = super::super::thumbnail::thumbnail(&document, close_up).unwrap();
        assert_eq!(pixels[(160 * 512 + 256) * 4 + 2], 0xf3);
    }

    #[test]
    fn exported_land_patterns_embed_raster_data_and_render_in_shelf_previews() {
        use crate::document::{Brush, Point, Stroke};
        let mut document = Document::blank();
        let mut stroke = Stroke {
            brush: Brush::Land,
            creation_id: None,
            material: None,
            points: vec![Point::new(-100., 100.)].into(),
            size: 250.,
            strength: 1.,
            seed: 471,
        };
        document.layers[0].strokes.push(stroke.clone());
        let camera = crate::library::Camera::for_document(&document);
        let parchment = super::super::thumbnail::thumbnail(&document, camera).unwrap();
        stroke.material = Some("ochre-sands".into());
        document.layers[0].strokes = vec![stroke].into();
        let svg = to_svg(&document);
        assert_eq!(svg.matches("data:image/png;base64,").count(), 1);
        assert!(svg.contains("patternUnits=\"userSpaceOnUse\""));
        let sand = super::super::thumbnail::thumbnail(&document, camera).unwrap();
        assert_ne!(parchment, sand);
        let center = (super::super::thumbnail::HEIGHT / 2 * super::super::thumbnail::WIDTH
            + super::super::thumbnail::WIDTH / 2) as usize
            * 4;
        // Thumbnails use BGRA. The textured center has a warm, near-monochrome tint.
        assert!((50..=65).contains(&(sand[center + 2] - sand[center])));
        assert!(sand[center] < parchment[center]);
    }
    #[test]
    fn mountain_sprites_share_embedded_artwork_and_render_in_previews() {
        use super::super::thumbnail::thumbnail;
        use crate::document::{Point, Stroke};

        let mut document = Document::blank();
        let camera = crate::library::Camera::for_document(&document);
        let blank = thumbnail(&document, camera).unwrap();
        let finishes = ["alpine-crags", "ochre-escarpments", "weathered-highlands"];
        let mut previews = Vec::new();
        for material in finishes {
            document.layers[0].strokes = vec![Stroke {
                brush: Brush::Mountain,
                creation_id: None,
                material: Some(material.into()),
                points: vec![Point::new(600., 500.), Point::new(980., 550.)].into(),
                size: 180.,
                strength: 1.,
                seed: 471,
            }]
            .into();
            let svg = to_svg(&document);
            assert_eq!(svg.matches("data:image/png;base64,").count(), 1);
            assert!(svg.contains("viewBox=\"0 0 1 1\""));
            assert!(svg.contains("viewBox=\"1 0 1 1\""));
            let painted = thumbnail(&document, camera).unwrap();
            let changed = blank
                .chunks_exact(4)
                .zip(painted.chunks_exact(4))
                .filter(|(before, after)| before[0].abs_diff(after[0]) > 20)
                .count();
            assert!(changed > 300, "{material} is visible in the preview");
            assert!(!previews.contains(&painted));
            previews.push(painted);
        }
    }
    #[test]
    fn streaming_export_matches_string_adapter_and_surfaces_io_errors() {
        for document in [Document::blank(), Document::demo()] {
            let mut bytes = Vec::new();
            write_svg(&document, &mut bytes).unwrap();
            assert_eq!(bytes, to_svg(&document).as_bytes());
        }
        struct Failing;
        impl std::io::Write for Failing {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("expected failure"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert!(
            write_svg(&Document::blank(), Failing)
                .unwrap_err()
                .contains("expected failure")
        );
    }
}
