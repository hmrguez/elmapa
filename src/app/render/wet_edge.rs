//! Draw the drying pigment in its paint layer, alongside the retained terrain.
use super::WorldContent;
use crate::app::state::Studio;
use bevy::{
    asset::embedded_asset,
    ecs::system::SystemParam,
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin},
};

const DRY_SECONDS: f32 = 0.45;

pub struct WetEdgePlugin;

impl Plugin for WetEdgePlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "wet_edge.wgsl");
        app.add_plugins(Material2dPlugin::<WetPigment>::default());
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct WetPigment {
    #[uniform(0)]
    strength: f32,
}

impl Material2d for WetPigment {
    fn fragment_shader() -> ShaderRef {
        "embedded://elmapa/app/render/wet_edge.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

pub(in crate::app) struct WetMeshes {
    pub paint: (u64, u64),
    pub meshes: Vec<(usize, Mesh)>,
}

#[derive(Default)]
pub struct PaintedEdge {
    paint: Option<(u64, u64)>,
    pieces: Vec<Entity>,
    released_at: Option<f32>,
    pigment: Handle<WetPigment>,
}

#[derive(SystemParam)]
pub struct EdgeGraphics<'w, 's> {
    commands: Commands<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<WetPigment>>,
}

pub fn sync(
    mut graphics: EdgeGraphics,
    mut studio: ResMut<Studio>,
    mut edge: Local<PaintedEdge>,
    time: Res<Time<Real>>,
) {
    let now = time.elapsed_secs();
    if edge
        .released_at
        .is_some_and(|start| now - start >= DRY_SECONDS)
        && studio.wet_release == edge.paint
    {
        studio.wet_release = None;
    }
    let paint = studio.wet_paint();
    if paint.is_none() && studio.wet_release.is_some() {
        studio.wet_release = None;
    }
    if edge.paint != paint {
        for entity in edge.pieces.drain(..) {
            graphics.commands.entity(entity).despawn();
        }
        edge.paint = paint;
        edge.released_at = None;
    }
    if paint.is_some() && studio.stroke.is_none() {
        edge.released_at.get_or_insert(now);
    }
    if studio.scene.needs_sync() {
        return;
    }
    if studio.scene.wet.is_some()
        && let Some(wet) = studio
            .scene
            .wet
            .take()
            .filter(|wet| Some(wet.paint) == paint)
    {
        for entity in edge.pieces.drain(..) {
            graphics.commands.entity(entity).despawn();
        }
        if edge.pigment == Handle::default() && !wet.meshes.is_empty() {
            edge.pigment = graphics.materials.add(WetPigment { strength: 1. });
        }
        let pigment = edge.pigment.clone();
        for (order, mesh) in wet.meshes {
            edge.pieces.push(
                graphics
                    .commands
                    .spawn((
                        WorldContent,
                        Mesh2d(graphics.meshes.add(mesh)),
                        MeshMaterial2d(pigment.clone()),
                        Transform::from_xyz(0., 0., order as f32 * 0.0001 + 0.00004),
                    ))
                    .id(),
            );
        }
    }
    if let Some(material) = graphics.materials.get(&edge.pigment) {
        let strength = if paint.is_some() {
            edge.released_at
                .map_or(1., |start| (1. - (now - start) / DRY_SECONDS).max(0.))
        } else {
            0.
        };
        if strength != material.strength {
            graphics
                .materials
                .get_mut(&edge.pigment)
                .expect("retained wet pigment")
                .strength = strength;
        }
    }
}
