use bevy::prelude::*;
use elmapa::document::{ContentBounds, Point as MapPoint};
#[derive(Clone, Copy)]
pub struct Bounds {
    pub size: Vec2,
}
impl Bounds {
    pub fn center(self) -> Vec2 {
        self.size / 2.
    }
}
fn point(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}
fn px(x: f32) -> f32 {
    x
}

/// World coordinates stay independent of document extents as new terrain is added.
#[derive(Clone, Copy, PartialEq)]
pub struct Viewport {
    pub zoom: f32,
    pub pan: MapPoint,
    center: MapPoint,
    pending_fit: Option<ContentBounds>,
}

impl Viewport {
    pub fn camera(self, bounds: Bounds) -> elmapa::library::Camera {
        let transform = self.transform(bounds);
        elmapa::library::Camera {
            center: transform.map_point(bounds.center()),
            width: bounds.size.x / transform.scale,
            height: bounds.size.y / transform.scale,
        }
    }

    pub fn restore(camera: elmapa::library::Camera, bounds: Bounds) -> Self {
        let zoom = (bounds.size.x / camera.width)
            .min(bounds.size.y / camera.height)
            .clamp(0.0001, 32.);
        Self {
            zoom,
            pan: MapPoint::new(0., 0.),
            center: camera.center,
            pending_fit: None,
        }
    }

    pub fn new(frame: ContentBounds) -> Self {
        Self {
            zoom: 1.,
            pan: MapPoint::new(0., 0.),
            center: MapPoint::new(0., 0.),
            pending_fit: Some(frame),
        }
    }

    pub fn reset(&mut self, frame: ContentBounds) {
        *self = Self::new(frame);
    }

    // Initial layout supplies the real canvas size. Painting can resolve a copy
    // on that first frame; input and subsequent renders retain the fitted camera.
    pub fn resolve(&mut self, bounds: Bounds) {
        if bounds.size.x <= px(0.) || bounds.size.y <= px(0.) {
            return;
        }
        if let Some(frame) = self.pending_fit.take() {
            self.center = MapPoint::new(
                (frame.min.x + frame.max.x) / 2.,
                (frame.min.y + frame.max.y) / 2.,
            );
            self.zoom = ((bounds.size.x / frame.width().max(1.))
                .min(bounds.size.y / frame.height().max(1.))
                * 0.78)
                .clamp(0.0001, 32.);
        }
    }

    pub fn zoom_by(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(0.0001, 32.);
    }

    pub fn transform(mut self, bounds: Bounds) -> MapTransform {
        self.resolve(bounds);
        MapTransform {
            origin: point(
                bounds.center().x + px(self.pan.x - self.center.x * self.zoom),
                bounds.center().y + px(self.pan.y - self.center.y * self.zoom),
            ),
            scale: self.zoom,
        }
    }

    pub fn zoom_at(&mut self, pointer: Vec2, factor: f32, bounds: Bounds) {
        self.resolve(bounds);
        let before = self.transform(bounds).map_point(pointer);
        self.zoom_by(factor);
        let transform = self.transform(bounds);
        let after = transform.map_point(pointer);
        self.pan.x += (after.x - before.x) * transform.scale;
        self.pan.y += (after.y - before.y) * transform.scale;
    }
}

#[derive(Clone, Copy)]
pub struct MapTransform {
    pub origin: Vec2,
    pub scale: f32,
}

impl MapTransform {
    pub fn map_point(self, point: Vec2) -> MapPoint {
        MapPoint::new(
            (point.x - self.origin.x) / self.scale,
            (point.y - self.origin.y) / self.scale,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bounds() -> Bounds {
        Bounds {
            size: Vec2::new(1440., 940.),
        }
    }
    #[test]
    fn pointer_anchored_zoom_and_camera_restore_preserve_the_world() {
        let mut view = Viewport::new(elmapa::document::Document::demo().content_bounds());
        view.resolve(bounds());
        view.pan = MapPoint::new(-157., 83.);
        let pointer = Vec2::new(312., 217.);
        let before = view.transform(bounds()).map_point(pointer);
        view.zoom_at(pointer, 1.25, bounds());
        let after = view.transform(bounds()).map_point(pointer);
        assert!((before.x - after.x).abs() < 0.001 && (before.y - after.y).abs() < 0.001);
        let camera = view.camera(bounds());
        let restored = Viewport::restore(camera, bounds());
        let again = restored.camera(bounds());
        assert!((again.center.x - camera.center.x).abs() < 0.001);
        assert!((again.width - camera.width).abs() < 0.001);
    }
    #[test]
    fn fitting_negative_artwork_does_not_limit_panning_or_resize_zoom() {
        let mut view = Viewport::new(ContentBounds {
            min: MapPoint::new(-10000., -20000.),
            max: MapPoint::new(-9000., -19000.),
        });
        view.resolve(bounds());
        let zoom = view.zoom;
        let center = view.camera(bounds()).center;
        assert_eq!(center, MapPoint::new(-9500., -19500.));
        view.pan.x += 200000.;
        view.resolve(Bounds {
            size: Vec2::new(800., 600.),
        });
        assert_eq!(view.zoom, zoom);
    }
}
