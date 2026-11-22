//! Rasterize bounded batches of the shared cartographic geometry.
use super::{ForestTree, Scene, SceneCache, svg::RasterPart};
use crate::document::{AtlasStyle, ContentBounds, Document};
use resvg::tiny_skia::{Color, FilterQuality, Pixmap, PixmapPaint, Transform};
use std::{collections::HashMap, io::Write};

const PNG_LONG_EDGE: f32 = 4096.;

/// Write a PNG framed to the same visible artwork as SVG export.
pub(crate) fn write_png(document: &Document, mut writer: impl Write) -> Result<(), String> {
    let bounds = document.content_bounds();
    let longest = bounds.width().max(bounds.height());
    let scale = PNG_LONG_EDGE / longest;
    let pixels = |extent: f32| (extent / longest * PNG_LONG_EDGE).ceil() as u32;
    let mut pixmap = Pixmap::new(pixels(bounds.width()), pixels(bounds.height()))
        .ok_or("Could not allocate PNG image")?;
    // Keep the paper opaque through the fractional edge pixel on the short side.
    let [_, red, green, blue] = document.style.paper().to_be_bytes();
    pixmap.fill(Color::from_rgba8(red, green, blue, 255));

    let scene = SceneCache::default().render(document, None);
    render(&mut pixmap, &scene, bounds, scale)?;
    if super::generation::is_stopped() {
        return Ok(());
    }
    let data = pixmap.encode_png().map_err(|e| e.to_string())?;
    writer.write_all(&data).map_err(|e| e.to_string())
}

pub(super) fn render(
    pixmap: &mut Pixmap,
    scene: &Scene,
    bounds: ContentBounds,
    scale: f32,
) -> Result<(), String> {
    let transform = Transform::from_scale(scale, scale);
    let options = usvg::Options::default();
    // Large forests reuse the same rasterized specimens, as on the native canvas.
    let raster_trees = scene.forest_trees().nth(511).is_some();
    let mut specimens = HashMap::new();
    super::svg::render_raster_parts(scene, bounds, raster_trees, scale, |part| {
        match part {
            RasterPart::Svg(svg) => {
                if super::generation::is_stopped() {
                    return Ok(());
                }
                let tree = usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
                resvg::render(&tree, transform, &mut pixmap.as_mut());
            }
            RasterPart::Tree(tree) => {
                draw_tree(pixmap, &mut specimens, tree, bounds, scale, scene.style());
            }
        }
        Ok(())
    })
}

fn draw_tree(
    target: &mut Pixmap,
    specimens: &mut HashMap<(usize, u32), Pixmap>,
    tree: &ForestTree,
    bounds: ContentBounds,
    scale: f32,
    style: AtlasStyle,
) {
    let extent = ContentBounds {
        min: tree.center,
        max: tree.center,
    }
    .padded(73. * tree.scale);
    if !extent.intersects(bounds) {
        return;
    }
    let size = ((200. * tree.scale * scale).ceil().clamp(16., 1024.) as u32).next_power_of_two();
    let specimen = specimens.entry((tree.variant, size)).or_insert_with(|| {
        let mut pixmap = Pixmap::new(size, size).expect("bounded specimen pixmap");
        resvg::render(
            super::forest_specimen_tree(tree.variant, style),
            Transform::from_scale(size as f32 / 100., size as f32 / 100.),
            &mut pixmap.as_mut(),
        );
        pixmap
    });
    let sprite_scale = 100. * tree.scale * scale / size as f32;
    let half = size as f32 / 2.;
    let transform = Transform::from_translate(-half, -half)
        .post_scale(sprite_scale, sprite_scale)
        .post_rotate(tree.rotation)
        .post_translate(
            (tree.center.x - bounds.min.x) * scale,
            (tree.center.y - bounds.min.y) * scale,
        );
    let paint = PixmapPaint {
        opacity: tree.opacity,
        quality: FilterQuality::Bilinear,
        ..PixmapPaint::default()
    };
    target.draw_pixmap(0, 0, specimen.as_ref(), &paint, transform, None);
}
