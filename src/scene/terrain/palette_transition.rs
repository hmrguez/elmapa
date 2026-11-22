//! A world-aligned pigment field for neighboring finishes of one terrain brush.
use super::materials::Contours;
use crate::{brush_materials, document::ContentBounds, scene::Shape};
use resvg::tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};

const STEP: f32 = 3.;
const RADIUS: usize = 3;
const APRON: f32 = 24.;

#[derive(Debug, PartialEq)]
pub struct TerrainPalette {
    pub bounds: ContentBounds,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl TerrainPalette {
    pub fn pigment(shape: &Shape) -> Option<u32> {
        match shape {
            Shape::RegionSurface {
                material,
                fill,
                granulated,
                ..
            } if material.is_some() || *granulated => {
                Some(material.map_or(*fill, |id| brush_materials::artwork_catalog()[id].color))
            }
            _ => None,
        }
    }

    pub(super) fn build(
        bounds: ContentBounds,
        surface: &Contours,
        finishes: &[Shape],
        fallback: u32,
    ) -> Option<Self> {
        let bounds = bounds.padded(APRON);
        let width = ((bounds.max.x - bounds.min.x) / STEP).round() as u32;
        let height = ((bounds.max.y - bounds.min.y) / STEP).round() as u32;
        let mut pixmap = Pixmap::new(width, height)?;
        let mut draw = |path: PathBuilder, hex: u32| {
            if let Some(path) = path.finish() {
                let [_, r, g, b] = hex.to_be_bytes();
                let paint = Paint {
                    shader: resvg::tiny_skia::Shader::SolidColor(Color::from_rgba8(r, g, b, 255)),
                    anti_alias: false,
                    ..Paint::default()
                };
                pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::EvenOdd,
                    Transform::identity(),
                    None,
                );
            }
        };
        let add_ring = |path: &mut PathBuilder, ring: &mut dyn Iterator<Item = [f32; 2]>| {
            let mut points =
                ring.map(|[x, y]| [(x - bounds.min.x) / STEP, (y - bounds.min.y) / STEP]);
            if let Some([x, y]) = points.next() {
                path.move_to(x, y);
                for [x, y] in points {
                    path.line_to(x, y);
                }
                path.close();
            }
        };
        let mut path = PathBuilder::new();
        for ring in surface {
            add_ring(
                &mut path,
                &mut ring.iter().map(|&[x, y]| [x as f32, y as f32]),
            );
        }
        draw(path, fallback);
        for finish in finishes {
            let Shape::RegionSurface { contours, .. } = finish else {
                continue;
            };
            let mut path = PathBuilder::new();
            for ring in contours {
                add_ring(&mut path, &mut ring.iter().map(|p| [p.x, p.y]));
            }
            draw(path, Self::pigment(finish).expect("terrain finish"));
        }

        let mut pixels = pixmap.take();
        let mut scratch = pixels.clone();
        let mut changed = false;
        // Two box passes per axis give a soft ramp. Each window stops at a
        // coverage gap, so pigment cannot travel through the coast or paper.
        for _ in 0..2 {
            changed |= blur(
                &pixels,
                &mut scratch,
                (width as usize, height as usize),
                false,
            );
            changed |= blur(
                &scratch,
                &mut pixels,
                (width as usize, height as usize),
                true,
            );
        }
        changed.then_some(Self {
            bounds,
            width,
            height,
            pixels,
        })
    }
}

fn blur(source: &[u8], target: &mut [u8], (width, height): (usize, usize), vertical: bool) -> bool {
    let (lines, length, line_stride, step) = if vertical {
        (width, height, 4, width * 4)
    } else {
        (height, width, width * 4, 4)
    };
    let mut changed = false;
    for line in 0..lines {
        let offset = line * line_stride;
        let at = |point| offset + point * step;
        let mut end = 0;
        while end < length {
            if source[at(end) + 3] == 0 {
                end += 1;
                continue;
            }
            let start = end;
            while end < length && source[at(end) + 3] != 0 {
                end += 1;
            }
            let mut sum = [0_i32; 3];
            let mut left = start;
            let mut right = start;
            for point in start..end {
                let limit = (point + RADIUS + 1).min(end);
                while right < limit {
                    for (i, channel) in sum.iter_mut().enumerate() {
                        *channel += i32::from(source[at(right) + i]);
                    }
                    right += 1;
                }
                if left < point.saturating_sub(RADIUS) {
                    for (i, channel) in sum.iter_mut().enumerate() {
                        *channel -= i32::from(source[at(left) + i]);
                    }
                    left += 1;
                }
                let count = (right - left) as i32;
                for (i, channel) in sum.into_iter().enumerate() {
                    let color = ((channel + count / 2) / count) as u8;
                    changed |= source[at(point) + i] != color;
                    target[at(point) + i] = color;
                }
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::Point,
        scene::terrain::{materials::points, regions::rectangle},
    };

    fn bounds(x0: f32, x1: f32) -> ContentBounds {
        ContentBounds {
            min: Point::new(x0, -48.),
            max: Point::new(x1, 48.),
        }
    }

    fn finish(area: ContentBounds) -> Shape {
        let material = brush_materials::catalog()
            .iter()
            .position(|ink| ink.id == "alpine-meadow");
        Shape::RegionSurface {
            contours: rectangle(area).iter().map(|ring| points(ring)).collect(),
            material,
            fill: 0xffffff,
            granulated: false,
            opacity: 1.,
        }
    }

    fn red(palette: &TerrainPalette, x: f32) -> u8 {
        let column = ((x - palette.bounds.min.x) / STEP) as usize;
        let row = (-palette.bounds.min.y / STEP) as usize;
        palette.pixels[4 * (row * palette.width as usize + column)]
    }

    #[test]
    fn a_finish_boundary_blends_continuously_across_tiles() {
        let surface = rectangle(bounds(-90., 90.));
        let finish = finish(bounds(-90., 0.));
        let palette = |tile| {
            TerrainPalette::build(tile, &surface, std::slice::from_ref(&finish), 0xffffff)
                .expect("adjacent pigments should blend")
        };
        let left = palette(bounds(-48., 0.));
        let right = palette(bounds(0., 48.));
        for x in [-4.5, -1.5, 1.5, 4.5] {
            assert_eq!(red(&left, x), red(&right, x));
        }
        let shades = [-28.5, -7.5, 7.5, 28.5].map(|x| red(if x < 0. { &left } else { &right }, x));
        assert_eq!(
            shades[0],
            (TerrainPalette::pigment(&finish).unwrap() >> 16) as u8
        );
        assert_eq!(shades[3], 255);
        assert!(shades.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn a_gap_blocks_blending_between_finishes() {
        let left = bounds(-90., -3.);
        let surface = [rectangle(left), rectangle(bounds(3., 90.))].concat();
        assert!(
            TerrainPalette::build(bounds(-48., 48.), &surface, &[finish(left)], 0xffffff).is_none()
        );
    }
}
