//! Short-lived leaf fall for newly painted forest brushwork.
use super::{Canvas, WorldContent, motion::Growing};
use crate::app::state::Studio;
use bevy::{
    asset::embedded_asset,
    ecs::system::SystemParam,
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin},
};
use elmapa::{document::ContentBounds, scene::TreeId};

const FALL_DURATION: f32 = 0.75;
const EMISSION_INTERVAL: f32 = 0.10;

pub struct TreeLeafPlugin;
impl Plugin for TreeLeafPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "tree_leaves.wgsl");
        app.add_plugins(Material2dPlugin::<LeafMaterial>::default());
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub struct LeafMaterial {
    #[uniform(0)]
    params: Vec4,
}
impl Material2d for LeafMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://elmapa/app/render/tree_leaves.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(Component)]
pub struct LeafFall {
    owner: Entity,
    tree_id: TreeId,
    started: f32,
}

#[derive(Default)]
pub struct Emitter {
    quad: Option<Handle<Mesh>>,
    next_at: f32,
}

#[derive(SystemParam)]
pub struct LeafGraphics<'w, 's> {
    commands: Commands<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<LeafMaterial>>,
}

fn depth(order: usize) -> f32 {
    order as f32 * 0.0001 + 0.00002
}

pub fn fall(
    mut graphics: LeafGraphics,
    mut emitter: Local<Emitter>,
    studio: Res<Studio>,
    canvas: Res<Canvas>,
    time: Res<Time<Real>>,
    new_trees: Query<(Entity, &Growing), Added<Mesh2d>>,
    mut leaves: Query<(
        Entity,
        &LeafFall,
        &MeshMaterial2d<LeafMaterial>,
        &mut Transform,
    )>,
) {
    let now = time.elapsed_secs();
    let enabled = studio.shader_motion()
        && !studio.home
        && !studio.installing
        && studio.opening.is_none()
        && canvas.view_replay.is_none();
    for (entity, fall, handle, mut transform) in &mut leaves {
        let age = now - fall.started;
        let owner = canvas.trees.get(&fall.tree_id).filter(|owner| {
            enabled && age < FALL_DURATION && owner.entity == fall.owner && owner.tree.opacity > 0.
        });
        let Some(owner) = owner else {
            graphics.commands.entity(entity).despawn();
            graphics.materials.remove(&handle.0);
            continue;
        };
        let z = depth(owner.order);
        if transform.translation.z != z {
            transform.translation.z = z;
        }
        if let Some(mut material) = graphics.materials.get_mut(&handle.0) {
            material.params.x = age / FALL_DURATION;
            material.params.y = owner.tree.opacity;
        }
    }
    if !enabled || now < emitter.next_at || new_trees.is_empty() {
        return;
    }

    let view = studio.viewport.transform(studio.bounds);
    let frame = ContentBounds {
        min: view.map_point(Vec2::ZERO),
        max: view.map_point(studio.bounds.size),
    };
    let Some((owner, growth)) = new_trees
        .iter()
        .filter(|(_, growth)| {
            let sprite = canvas.forest_atlas.sprite(growth.tree.variant);
            growth.tree.opacity > 0.
                && ContentBounds {
                    min: growth.tree.center,
                    max: growth.tree.center,
                }
                .padded(sprite.extent.max_element() * growth.tree.scale)
                .intersects(frame)
        })
        .min_by_key(|(_, growth)| growth.tree.id)
    else {
        return;
    };
    let tree = &growth.tree;
    let sprite = canvas.forest_atlas.sprite(tree.variant);
    let extent = sprite.extent * tree.scale;
    let quad = emitter
        .quad
        .get_or_insert_with(|| graphics.meshes.add(Rectangle::new(1., 1.)));
    let phase = ((tree.id.1 % 31 + tree.id.2 % 31 + tree.id.3) % 31) as f32;
    let material = graphics.materials.add(LeafMaterial {
        params: Vec4::new(0., tree.opacity, phase, 0.),
    });
    graphics.commands.spawn((
        WorldContent,
        LeafFall {
            owner,
            tree_id: tree.id,
            started: now,
        },
        Mesh2d(quad.clone()),
        MeshMaterial2d(material),
        Transform::from_xyz(
            tree.center.x + sprite.offset.x * tree.scale,
            -tree.center.y - sprite.offset.y * tree.scale - extent.y * 0.28,
            depth(canvas.trees[&tree.id].order),
        )
        .with_scale((extent * Vec2::new(1.55, 1.25)).extend(1.)),
    ));
    emitter.next_at = now + EMISSION_INTERVAL;
}
