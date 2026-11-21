//! Bounded shelf snapshots from the same SVG instances and terrain as export.
use super::{ForestTree, svg::PreviewPart};
use resvg::tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};
use std::collections::HashMap;

use crate::{
    document::{AtlasStyle, ContentBounds, Document, Point},
    library::Camera,
};
pub const WIDTH: u32 = 512;
pub const HEIGHT: u32 = 320;

pub fn thumbnail(document: &Document, camera: Camera) -> Result<Vec<u8>, String> {
    super::generation::bounded(super::generation::Limits::INTERACTIVE, None, || {
        let scene = super::SceneCache::default().render(document, None);
        thumbnail_from_scene(&scene, camera)
    })
    .map_err(|e| e.to_string())?
}

/// Reuse the ready authoritative scene when checkpointing the active atlas.
pub fn thumbnail_from_scene(scene: &super::Scene, camera: Camera) -> Result<Vec<u8>, String> {
    let _span = crate::performance::Span::new("thumbnail_generation");
    let scale = (WIDTH as f32 / camera.width).max(HEIGHT as f32 / camera.height);
    let width = WIDTH as f32 / scale;
    let height = HEIGHT as f32 / scale;
    let bounds = ContentBounds {
        min: Point::new(camera.center.x - width / 2., camera.center.y - height / 2.),
        max: Point::new(camera.center.x + width / 2., camera.center.y + height / 2.),
    };
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).ok_or("Could not allocate thumbnail")?;
    let transform = Transform::from_scale(scale, scale);
    let options = usvg::Options::default();
    // Large forests reuse the same rasterized specimens, as on the native canvas.
    let raster_trees = scene.forest_trees().nth(511).is_some();
    let mut specimens = HashMap::new();
    super::svg::render_preview_parts(scene, bounds, raster_trees, scale, |part| {
        match part {
            PreviewPart::Svg(svg) => {
                let tree = usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
                resvg::render(&tree, transform, &mut pixmap.as_mut());
            }
            PreviewPart::Tree(tree) => {
                draw_tree(
                    &mut pixmap,
                    &mut specimens,
                    tree,
                    bounds,
                    scale,
                    scene.style(),
                );
            }
        }
        Ok(())
    })?;
    let mut bytes = pixmap.take();
    for pixel in bytes.chunks_exact_mut(4) {
        let a = u32::from(pixel[3]);
        if a > 0 {
            for channel in &mut pixel[..3] {
                *channel = (u32::from(*channel) * 255 + a / 2)
                    .checked_div(a)
                    .unwrap_or(0)
                    .min(255) as u8;
            }
        }
        pixel.swap(0, 2);
    }
    Ok(bytes)
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dense_forest_preview_renders_at_shelf_resolution() {
        let document = crate::performance_fixtures::fixture("dense-forest", 16).unwrap();
        let pixels = thumbnail(&document, Camera::for_document(&document)).unwrap();
        assert_eq!(pixels.len(), (WIDTH * HEIGHT * 4) as usize);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[0] < 150));
    }

    #[test]
    fn prepared_scene_thumbnail_matches_cold_thumbnail_for_saved_camera() {
        let document = Document::demo();
        let scene = super::super::SceneCache::default().render(&document, None);
        let mut camera = Camera::for_document(&document);
        camera.center = Point::new(-120., 75.);
        assert_eq!(
            thumbnail(&document, camera).unwrap(),
            thumbnail_from_scene(&scene, camera).unwrap()
        );
    }
}
