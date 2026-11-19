//! Color-grade the authored seamless tiles and aligned mountain frames for
//! the woodland atlas. The original PNGs also serve the classic palette.
use super::Material;
use crate::document::Brush;
use image::{ImageEncoder, Rgba, RgbaImage, codecs::png::PngEncoder};

pub(super) fn texture(material: &Material, pigment: u32) -> Option<Box<[u8]>> {
    let bytes = material.texture?;
    let source = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .expect("embedded brush texture")
        .into_rgba8();
    let new = match material.sprites {
        Some(sheet) => relief(&source, sheet.frame_width, pigment),
        None => tile(&source, material.color, pigment, material.brush),
    };
    let mut png = Vec::new();
    PngEncoder::new(&mut png)
        .write_image(
            new.as_raw(),
            new.width(),
            new.height(),
            image::ExtendedColorType::Rgba8,
        )
        .expect("encode woodland brush texture");
    Some(png.into_boxed_slice())
}

fn rgb(color: u32) -> [f32; 3] {
    [(color >> 16) as u8, (color >> 8) as u8, color as u8].map(f32::from)
}

fn value(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

fn tile(source: &RgbaImage, previous: u32, pigment: u32, brush: Brush) -> RgbaImage {
    let base = rgb(pigment);
    let original = rgb(previous);
    let detail = if brush == Brush::Water { 0.57 } else { 0.72 };
    RgbaImage::from_fn(source.width(), source.height(), |x, y| {
        let [r, g, b, a] = source.get_pixel(x, y).0;
        let delta = value([r.into(), g.into(), b.into()]) - value(original);
        let [r, g, b] = base.map(|channel| (channel + delta * detail).round() as u8);
        Rgba([r, g, b, a])
    })
}

fn relief(source: &RgbaImage, frame: u32, pigment: u32) -> RgbaImage {
    let dark = rgb(0x6d5845);
    let light = rgb(pigment);
    let wash = rgb(0x9c8968);
    let height = source.height();
    RgbaImage::from_fn(source.width(), height, |x, y| {
        let u = (x % frame) as f32 / frame as f32;
        let v = y as f32 / height as f32;
        let [r, g, b, a] = source.get_pixel(x, y).0;
        let tone = ((value([r.into(), g.into(), b.into()]) - 50.) / 197.).clamp(0., 1.);
        let tint = (tone * 0.87 + 0.13 + (0.52 - u) * 0.12).clamp(0., 1.);
        // Uneven feathering follows the lower spurs without a horizontal cut.
        let edge = 0.99 - 0.042 * (u * 18.).sin().powi(2) - 0.028 * (u * 29.).sin();
        let foot = ((edge - v) / 0.16).clamp(0., 1.);
        let cover = f32::from(a) / 255. * foot;
        let center = 0.887 + 0.028 * (u * 12.).sin();
        let skirt =
            (1. - ((u - 0.54) / 0.49).powi(2) - ((v - center) / 0.103).powi(2)).clamp(0., 1.);
        let under = skirt * 0.20 * (1. - cover);
        let opacity = cover + under;
        let [r, g, b] = std::array::from_fn(|i| {
            let stone = dark[i] + (light[i] - dark[i]) * tint;
            ((stone * cover + wash[i] * under) / opacity.max(0.001)).round() as u8
        });
        Rgba([r, g, b, (opacity * 255.).round() as u8])
    })
}
