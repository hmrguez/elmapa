//! A periodic pigment density field shared by the canvas and static artwork.
use super::{noise::hash, paper};
use crate::document::AtlasStyle;
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use std::sync::OnceLock;

pub const WORLD_SIZE: f32 = 1024.;
pub const TEXTURE_SIZE: u32 = 512;
// Solid marks sharing a mesh with a wash sample the center of the neutral texel.
pub const NEUTRAL_UV: [f32; 2] = [0.5 / TEXTURE_SIZE as f32; 2];

fn smooth_noise(x: f32, y: f32, period: i64, salt: u64) -> f32 {
    let (left, top) = (x.floor() as i64, y.floor() as i64);
    let smooth = |t: f32| t * t * (3. - 2. * t);
    let (u, v) = (smooth(x - left as f32), smooth(y - top as f32));
    let cell =
        |x: i64, y: i64| hash(paper::fibre_seed(x.rem_euclid(period), y.rem_euclid(period)) ^ salt);
    let lerp = |a, b, t| a * (1. - t) + b * t;
    lerp(
        lerp(cell(left, top), cell(left + 1, top), u),
        lerp(cell(left, top + 1), cell(left + 1, top + 1), u),
        v,
    )
}

fn density(x: u32, y: u32) -> f32 {
    let (u, v) = (
        (x as f32 + 0.5) / TEXTURE_SIZE as f32,
        (y as f32 + 0.5) / TEXTURE_SIZE as f32,
    );
    let paper_cells = WORLD_SIZE / paper::CELL;
    let pooling = smooth_noise(u * paper_cells, v * paper_cells, paper_cells as i64, 0);
    let smaller_pools = smooth_noise(u * 64., v * 64., 64, 0x6bc81f0);
    let grain = hash(paper::fibre_seed(i64::from(x), i64::from(y)) ^ 0xf409b).powi(3);
    0.40 * pooling.powi(2) + 0.17 * smaller_pools + grain * (0.23 + 0.20 * pooling)
}

fn levels() -> &'static [Vec<f32>] {
    static LEVELS: OnceLock<Vec<Vec<f32>>> = OnceLock::new();
    LEVELS.get_or_init(|| {
        let mut size = TEXTURE_SIZE as usize;
        let mut levels = vec![
            (0..TEXTURE_SIZE)
                .flat_map(|y| (0..TEXTURE_SIZE).map(move |x| density(x, y)))
                .collect::<Vec<_>>(),
        ];
        while size > 1 {
            let previous = levels.last().expect("base density");
            let next = size / 2;
            levels.push(
                (0..next)
                    .flat_map(|y| {
                        let row = y * 2 * size;
                        (0..next).map(move |x| {
                            let p = row + x * 2;
                            (previous[p]
                                + previous[p + 1]
                                + previous[p + size]
                                + previous[p + size + 1])
                                * 0.25
                        })
                    })
                    .collect(),
            );
            size = next;
        }
        levels
    })
}

fn opacity(style: AtlasStyle, density: f32) -> u8 {
    let strength = if style.is_classic() { 9. } else { 14. };
    (density * strength).round() as u8
}

/// Pack the mip chain as straight RGBA multipliers for authored sRGB artwork.
pub fn texture_rgba(style: AtlasStyle) -> Vec<u8> {
    let mut pixels: Vec<_> = levels()
        .iter()
        .flatten()
        .flat_map(|&density| {
            let shade = 255 - opacity(style, density);
            [shade, shade, shade, 255]
        })
        .collect();
    pixels[..3].fill(255);
    pixels
}

pub(super) fn svg_overlay(style: AtlasStyle) -> &'static [u8] {
    static CLASSIC: OnceLock<Vec<u8>> = OnceLock::new();
    static WOODLAND: OnceLock<Vec<u8>> = OnceLock::new();
    let cache = match style {
        AtlasStyle::Classic => &CLASSIC,
        AtlasStyle::Woodland => &WOODLAND,
    };
    cache.get_or_init(|| {
        // Retain pigment pooling in portable artwork and drop the finest grain.
        const MIP: usize = 2;
        let size = TEXTURE_SIZE >> MIP;
        let pixels: Vec<_> = levels()[MIP]
            .iter()
            .flat_map(|&density| [0, 0, 0, opacity(style, density)])
            .collect();
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(&pixels, size, size, ExtendedColorType::Rgba8)
            .expect("encode pigment density into memory");
        png
    })
}
