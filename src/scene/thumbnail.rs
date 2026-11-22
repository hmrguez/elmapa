//! Bounded shelf snapshots from the same geometry as export.
use resvg::tiny_skia::Pixmap;

use crate::{
    document::{ContentBounds, Document, Point},
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
    super::raster::render(&mut pixmap, scene, bounds, scale)?;
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
