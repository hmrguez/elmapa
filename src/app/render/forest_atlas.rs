//! Cache the forest sprite atlas and coalesce resolution changes.
use super::{
    forest_wind::{ForestWind, Specimen, WindParams},
    rgba_image, straight_alpha,
};
use crate::app::viewport::{Bounds, Viewport};
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, block_on, poll_once},
};
use elmapa::{
    document::ContentBounds,
    scene::{FOREST_SPECIMEN_COUNT, Scene},
};

#[derive(Clone, Copy, Default)]
pub(super) struct TreeSprite {
    pub(super) rect: Rect,
    pub(super) extent: Vec2,
    pub(super) offset: Vec2,
}

impl TreeSprite {
    fn specimen(self, atlas_size: Vec2) -> Specimen {
        let center = Vec2::new(self.offset.x, -self.offset.y);
        let half = self.extent * 0.5;
        let min = center - half;
        let max = center + half;
        Specimen {
            bounds: Vec4::new(min.x, min.y, max.x, max.y),
            uv: Vec4::new(
                self.rect.min.x / atlas_size.x,
                self.rect.min.y / atlas_size.y,
                self.rect.max.x / atlas_size.x,
                self.rect.max.y / atlas_size.y,
            ),
        }
    }
}

struct Atlas {
    image: Image,
    sprites: [TreeSprite; FOREST_SPECIMEN_COUNT],
    levels: [u32; FOREST_SPECIMEN_COUNT],
}
fn resolution(extent: f32) -> u32 {
    (extent.ceil().clamp(32., 2048.) as u32)
        .next_power_of_two()
        .min(2048)
}
fn tree_atlas(levels: [u32; FOREST_SPECIMEN_COUNT]) -> Atlas {
    // Group similar LODs so a zoomed-in specimen does not enlarge every cell.
    // With up to four 2048-pixel tiles per row and four rows, each axis fits 8192.
    let mut variants: [usize; FOREST_SPECIMEN_COUNT] = std::array::from_fn(|i| i);
    variants.sort_by_key(|&variant| std::cmp::Reverse(levels[variant]));
    let width: u32 = variants[..4].iter().map(|&variant| levels[variant]).sum();
    let mut origins = [UVec2::ZERO; FOREST_SPECIMEN_COUNT];
    let mut height = 0;
    for row in variants.chunks(4) {
        let mut x = 0;
        for &variant in row {
            origins[variant] = UVec2::new(x, height);
            x += levels[variant];
        }
        height += levels[row[0]];
    }
    let mut pixels = vec![0; (width * height * 4) as usize];
    let mut sprites = [TreeSprite::default(); FOREST_SPECIMEN_COUNT];
    for variant in 0..FOREST_SPECIMEN_COUNT {
        let size = levels[variant];
        let mut tile = resvg::tiny_skia::Pixmap::new(size, size).expect("tree tile");
        resvg::render(
            elmapa::scene::forest_specimen_tree(variant),
            resvg::tiny_skia::Transform::from_scale(size as f32 / 100., size as f32 / 100.),
            &mut tile.as_mut(),
        );
        let mut data = tile.take();
        straight_alpha(&mut data);
        let (mut left, mut top, mut right, mut bottom) = (size, size, 0, 0);
        for (i, pixel) in data.chunks_exact(4).enumerate() {
            if pixel[3] != 0 {
                let x = i as u32 % size;
                let y = i as u32 / size;
                left = left.min(x);
                top = top.min(y);
                right = right.max(x + 1);
                bottom = bottom.max(y + 1);
            }
        }
        left = left.saturating_sub(2);
        top = top.saturating_sub(2);
        right = (right + 2).min(size);
        bottom = (bottom + 2).min(size);
        let origin = origins[variant];
        for y in 0..size {
            let from = (y * size * 4) as usize;
            let to = (((origin.y + y) * width + origin.x) * 4) as usize;
            pixels[to..to + (size * 4) as usize]
                .copy_from_slice(&data[from..from + (size * 4) as usize]);
        }
        sprites[variant] = TreeSprite {
            rect: Rect::from_corners(
                (origin + UVec2::new(left, top)).as_vec2(),
                (origin + UVec2::new(right, bottom)).as_vec2(),
            ),
            extent: Vec2::new((right - left) as f32, (bottom - top) as f32) * 100. / size as f32,
            offset: Vec2::new(
                (left + right) as f32 / 2. - size as f32 / 2.,
                (top + bottom) as f32 / 2. - size as f32 / 2.,
            ) * 100.
                / size as f32,
        };
    }
    Atlas {
        image: rgba_image(width, height, pixels),
        sprites,
        levels,
    }
}
#[derive(Default)]
pub(super) struct ForestAtlas {
    job: Option<Task<Atlas>>,
    material: Option<Handle<ForestWind>>,
    sprites: [TreeSprite; FOREST_SPECIMEN_COUNT],
    levels: [u32; FOREST_SPECIMEN_COUNT],
    lod_view: Option<(Viewport, Vec2, f32)>,
}

impl ForestAtlas {
    pub(super) fn initialize(&mut self) {
        self.job = Some(
            AsyncComputeTaskPool::get().spawn(async { tree_atlas([128; FOREST_SPECIMEN_COUNT]) }),
        );
    }

    pub(super) fn ready(&self) -> bool {
        self.material.is_some() && self.job.is_none()
    }

    pub(super) fn material(&self) -> Option<&Handle<ForestWind>> {
        self.material.as_ref()
    }

    pub(super) fn sprite(&self, variant: usize) -> TreeSprite {
        self.sprites[variant]
    }

    pub(super) fn poll(
        &mut self,
        images: &mut Assets<Image>,
        materials: &mut Assets<ForestWind>,
        wind_enabled: bool,
    ) -> bool {
        let enabled = u32::from(wind_enabled);
        if let Some(handle) = &self.material
            && let Some(mut material) = materials.get_mut(handle)
            && material.params.enabled != enabled
        {
            material.params.enabled = enabled;
        }
        let Some(mut job) = self.job.take() else {
            return false;
        };
        let Some(atlas) = block_on(poll_once(&mut job)) else {
            self.job = Some(job);
            return false;
        };
        self.sprites = atlas.sprites;
        self.levels = atlas.levels;
        let atlas_size = atlas.image.size().as_vec2();
        let material = ForestWind {
            params: WindParams {
                enabled,
                specimens: self.sprites.map(|sprite| sprite.specimen(atlas_size)),
            },
            atlas: images.add(atlas.image),
        };
        if let Some(handle) = &self.material {
            *materials.get_mut(handle).expect("retained forest material") = material;
        } else {
            self.material = Some(materials.add(material));
        }
        true
    }

    pub(super) fn request(
        &mut self,
        viewport: Viewport,
        bounds: Bounds,
        scale_factor: f32,
        scene: &Scene,
        scene_changed: bool,
    ) {
        let lod_view = (viewport, bounds.size, scale_factor);
        if self.job.is_some() || !scene_changed && self.lod_view == Some(lod_view) {
            return;
        }
        self.lod_view = Some(lod_view);
        let view = viewport.transform(bounds);
        let frame = ContentBounds {
            min: view.map_point(Vec2::ZERO),
            max: view.map_point(bounds.size),
        };
        let mut extents = [0_f32; FOREST_SPECIMEN_COUNT];
        for tree in scene.forest_trees().filter(|tree| {
            ContentBounds {
                min: tree.center,
                max: tree.center,
            }
            .padded(73. * tree.scale)
            .intersects(frame)
        }) {
            extents[tree.variant] =
                extents[tree.variant].max(100. * view.scale * tree.scale * scale_factor * 1.025);
        }
        let levels = std::array::from_fn(|i| {
            let old = self.levels[i];
            let extent = extents[i];
            if old == 0 || extent < old as f32 * 0.35 || extent > old as f32 * 1.10 {
                resolution(extent)
            } else {
                old
            }
        });
        if levels != self.levels {
            self.job = Some(AsyncComputeTaskPool::get().spawn(async move { tree_atlas(levels) }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_conversion_preserves_translucent_specimen_edges() {
        let mut bytes = vec![64, 32, 16, 128, 0, 0, 0, 0];
        straight_alpha(&mut bytes);
        assert_eq!(&bytes[..4], &[128, 64, 32, 128]);
        assert_eq!(&bytes[4..], &[0, 0, 0, 0]);
    }
    #[test]
    fn atlas_preserves_exact_specimen_pixels_cropping_and_world_pivots() {
        let atlas = tree_atlas([128; FOREST_SPECIMEN_COUNT]);
        let data = atlas.image.data.as_ref().unwrap();
        for variant in 0..FOREST_SPECIMEN_COUNT {
            let mut reference = resvg::tiny_skia::Pixmap::new(128, 128).unwrap();
            resvg::render(
                elmapa::scene::forest_specimen_tree(variant),
                resvg::tiny_skia::Transform::from_scale(1.28, 1.28),
                &mut reference.as_mut(),
            );
            let mut reference = reference.take();
            straight_alpha(&mut reference);
            let sprite = atlas.sprites[variant];
            let origin = UVec2::new(variant as u32 % 4 * 128, variant as u32 / 4 * 128);
            let left = (sprite.rect.min.x - origin.x as f32) as usize;
            let top = (sprite.rect.min.y - origin.y as f32) as usize;
            let atlas_size = atlas.image.size().as_vec2();
            let specimen = sprite.specimen(atlas_size);
            let texels = specimen.uv * atlas_size.extend(atlas_size.x).extend(atlas_size.y);
            assert!(texels.xy().abs_diff_eq(sprite.rect.min, 0.0001));
            assert!(texels.zw().abs_diff_eq(sprite.rect.max, 0.0001));
            let to_world = |p: Vec2| (p - origin.as_vec2()) * (100. / 128.) - 50.;
            let top_left = to_world(sprite.rect.min);
            let bottom_right = to_world(sprite.rect.max);
            assert_eq!(
                specimen.bounds,
                Vec4::new(top_left.x, -bottom_right.y, bottom_right.x, -top_left.y)
            );
            assert!((sprite.extent.x - sprite.rect.width() * 100. / 128.).abs() < 0.00001);
            assert!(
                (sprite.offset.x - ((left as f32 + sprite.rect.width() / 2.) - 64.) * 100. / 128.)
                    .abs()
                    < 0.00001
            );
            for y in 0..128_usize {
                for x in 0..128_usize {
                    let from = (y * 128 + x) * 4;
                    let to = ((origin.y as usize + y) * 512 + origin.x as usize + x) * 4;
                    assert_eq!(&reference[from..from + 4], &data[to..to + 4]);
                    if x < left
                        || y < top
                        || x >= left + sprite.rect.width() as usize
                        || y >= top + sprite.rect.height() as usize
                    {
                        assert_eq!(reference[from + 3], 0);
                    }
                }
            }
        }
    }
}
