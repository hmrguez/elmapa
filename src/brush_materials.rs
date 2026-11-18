//! Shared catalog for brushes with a material picker. Documents store stable IDs.
use crate::document::Brush;
use serde::Deserialize;
use std::{collections::HashSet, io::Cursor, num::NonZeroU32, sync::OnceLock};

#[derive(Deserialize)]
struct Manifest {
    materials: Vec<Material>,
}

#[derive(Deserialize)]
pub struct Material {
    pub id: String,
    pub brush: Brush,
    pub name: String,
    #[serde(deserialize_with = "hex_color")]
    pub color: u32,
    #[serde(default)]
    pub tile_size: f32,
    #[serde(default, deserialize_with = "optional_hex_color")]
    pub shoreline_ink: Option<u32>,
    pub forest: Option<ForestStyle>,
    #[serde(default)]
    sprite_frames: Option<NonZeroU32>,
    #[serde(skip)]
    pub sprites: Option<SpriteSheet>,
    #[serde(skip)]
    pub texture: Option<&'static [u8]>,
}

#[derive(Clone, Copy)]
pub struct SpriteSheet {
    pub frames: u32,
    pub frame_width: u32,
    pub frame_height: u32,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForestStyle {
    Fir,
    Oak,
}

fn hex_color<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<u32, D::Error> {
    let value = String::deserialize(decoder)?;
    parse_color(&value)
}

fn optional_hex_color<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<u32>, D::Error> {
    Option::<String>::deserialize(decoder)?
        .as_deref()
        .map(parse_color)
        .transpose()
}

fn parse_color<E: serde::de::Error>(value: &str) -> Result<u32, E> {
    value
        .strip_prefix('#')
        .filter(|hex| hex.len() == 6)
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .ok_or_else(|| serde::de::Error::custom("expected a #rrggbb brush color"))
}

pub fn catalog() -> &'static [Material] {
    static MATERIALS: OnceLock<Vec<Material>> = OnceLock::new();
    MATERIALS.get_or_init(|| {
        let manifest = include_str!("../assets/brush-materials/manifest.json");
        let mut materials = serde_json::from_str::<Manifest>(manifest)
            .expect("valid brush manifest")
            .materials;
        let textures: &[Option<&[u8]>] = include!(concat!(env!("OUT_DIR"), "/brush_textures.rs"));
        assert_eq!(materials.len(), textures.len());

        let mut ids = HashSet::new();
        let mut brushes = HashSet::new();
        for (material, &texture) in materials.iter_mut().zip(textures) {
            assert!(
                valid_id(&material.id) && ids.insert(material.id.clone()),
                "invalid or duplicate brush material id"
            );
            assert!(
                !material.name.trim().is_empty(),
                "empty brush material name"
            );
            let first = brushes.insert(material.brush);
            if let Some(frames) = material.sprite_frames {
                assert_eq!(
                    material.brush,
                    Brush::Mountain,
                    "only mountains use sprite sheets"
                );
                let png = texture.expect("mountain sprite sheet PNG");
                let (width, height) =
                    image::ImageReader::with_format(Cursor::new(png), image::ImageFormat::Png)
                        .into_dimensions()
                        .expect("valid mountain sprite sheet PNG");
                let frames = frames.get();
                assert!(
                    width.is_multiple_of(frames),
                    "sprite frames must have equal width"
                );
                material.sprites = Some(SpriteSheet {
                    frames,
                    frame_width: width / frames,
                    frame_height: height,
                });
            } else if texture.is_some() {
                assert_ne!(
                    material.brush,
                    Brush::Mountain,
                    "mountain textures need sprite frames"
                );
                assert!(
                    (16.0..=4096.0).contains(&material.tile_size),
                    "brush textures need a valid tile_size"
                );
            } else {
                assert!(
                    first || material.forest.is_some(),
                    "only the first material may have a plain finish"
                );
            }
            assert!(
                material.forest.is_none() || (material.brush == Brush::Forest && texture.is_none()),
                "forest sprite styles need a forest material"
            );
            assert!(
                material.shoreline_ink.is_none()
                    || (material.brush == Brush::Water && texture.is_some()),
                "shoreline ink needs a textured water material"
            );
            material.texture = texture;
        }
        materials
    })
}

pub fn for_brush(brush: Brush) -> impl Iterator<Item = (usize, &'static Material)> {
    catalog()
        .iter()
        .enumerate()
        .filter(move |(_, material)| material.brush == brush)
}

pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
