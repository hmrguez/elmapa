//! Opening transition from a shelf preview to the retained canvas.
use super::{Canvas, Graphics, WorldContent};
use crate::app::{
    state::Studio,
    ui::{color, cover_rect},
};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

pub struct OpeningEntities {
    image: Entity,
    cover: Entity,
    mesh: Handle<Mesh>,
}
/// Enlarge the cached page and peel the same four-corner cover as the GPUI view.
pub fn opening(
    graphics: Graphics,
    mut studio: ResMut<Studio>,
    canvas: Res<Canvas>,
    time: Res<Time<Real>>,
    mut images: Query<(&mut Sprite, &mut Transform), Without<WorldContent>>,
    mut entities: Local<Option<OpeningEntities>>,
    mut clear: ResMut<ClearColor>,
) {
    let Graphics {
        mut commands,
        mut meshes,
        mut materials,
        ..
    } = graphics;
    let Some(mut started) = studio.opening else {
        if let Some(old) = entities.take() {
            commands.entity(old.image).despawn();
            commands.entity(old.cover).despawn();
        }
        clear.0 = color(studio.editor.document().style.paper(), 1.);
        return;
    };
    clear.0 = color(0xeceee7, 1.);
    if !studio.opening_ready {
        if !studio.scene.is_pending() && canvas.ready() {
            studio.opening_ready = true;
            started = time.elapsed_secs();
            studio.opening = Some(started);
        } else {
            started = time.elapsed_secs();
        }
    }
    let progress = if studio.reduced {
        1.
    } else {
        ((time.elapsed_secs() - started) / 1.15).clamp(0., 1.)
    };
    if progress >= 1. {
        studio.opening = None;
        studio.ui_dirty = true;
        return;
    }
    let eased = 1. - (1. - progress).powi(3);
    let from = studio.opening_from;
    let min = from.min.lerp(Vec2::ZERO, eased);
    let size = from.size().lerp(studio.bounds.size, eased);
    let view = studio.viewport.transform(studio.bounds);
    let center = view.map_point(min + size / 2.);
    let image = Sprite {
        image: studio.opening_image.clone().unwrap_or_default(),
        custom_size: Some(size / view.scale),
        rect: studio.opening_image.as_ref().map(|_| cover_rect(size)),
        color: if studio.opening_image.is_some() {
            Color::WHITE
        } else {
            color(studio.editor.document().style.paper(), 1.)
        },
        ..default()
    };
    let transform = Transform::from_xyz(center.x, -center.y, 150.);
    let fold = (progress / 0.62).min(1.);
    let edge = size.x * (1. - fold * 2.4);
    let lift = size.y * (std::f32::consts::PI * fold).sin() * 0.09;
    let positions = [
        min,
        min + Vec2::new(edge, lift),
        min + Vec2::new(edge, size.y - lift),
        min + Vec2::new(0., size.y),
        min,
        min + Vec2::new(3., 0.),
        min + Vec2::new(3., size.y),
        min + Vec2::new(0., size.y),
    ]
    .map(|p| {
        let p = view.map_point(p);
        [p.x, -p.y, 0.]
    });
    let rgba = color(0xdce9df, 0.9 * (1. - fold))
        .to_linear()
        .to_f32_array();
    let hinge = color(0x304742, 0.22 * (1. - fold))
        .to_linear()
        .to_f32_array();
    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions.to_vec())
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_COLOR,
            [vec![rgba; 4], vec![hinge; 4]].concat(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0., 0.]; 8])
        .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7]));
    if let Some(old) = entities.as_ref() {
        if let Ok((mut sprite, mut old_transform)) = images.get_mut(old.image) {
            *sprite = image;
            *old_transform = transform;
        }
        if let Some(mut old_mesh) = meshes.get_mut(&old.mesh) {
            *old_mesh = mesh;
        }
    } else {
        let image = commands.spawn((image, transform)).id();
        let mesh = meshes.add(mesh);
        let cover = commands
            .spawn((
                Mesh2d(mesh.clone()),
                MeshMaterial2d(materials.add(ColorMaterial::default())),
                Transform::from_xyz(0., 0., 160.),
            ))
            .id();
        *entities = Some(OpeningEntities { image, cover, mesh });
    }
}
