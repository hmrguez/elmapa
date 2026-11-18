//! Placement transforms and transient shader accents for each landmark part.
use super::{
    render::{Canvas, WorldContent},
    state::Studio,
};
use bevy::{
    asset::embedded_asset,
    ecs::system::SystemParam,
    prelude::*,
    reflect::TypePath,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin},
};
use elmapa::{
    document::ContentBounds,
    scene::{
        LandmarkMotion, LandmarkPart, LandmarkStep, Shape, landmark_definition, landmark_pose,
    },
};
use std::{collections::HashMap, sync::Arc};

const ARRIVAL_LIFETIME: f32 = 1.1;
const UNFURL_SHADER: u32 = 4;

pub struct LandmarkPlugin;
impl Plugin for LandmarkPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "landmark_arrival.wgsl");
        app.add_plugins(Material2dPlugin::<ArrivalMaterial>::default());
    }
}

#[derive(Clone, Copy, Debug, ShaderType)]
struct ArrivalInk {
    progress: f32,
    opacity: f32,
    seed: f32,
    direction: f32,
    motion: u32,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct ArrivalMaterial {
    #[uniform(0)]
    ink: ArrivalInk,
}

impl Material2d for ArrivalMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://elmapa/app/landmark_arrival.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(Component)]
pub struct Assembling {
    pub part: LandmarkPart,
    pub material: Handle<ColorMaterial>,
    pub opacity: f32,
    pub started: f32,
}
pub fn transform(part: &LandmarkPart, elapsed: f32, reduced: bool, z: f32) -> Transform {
    let step = &landmark_definition(&part.landmark.kind).parts[part.index];
    let [sx, sy, y, angle, _] = landmark_pose(step, elapsed, reduced);
    let pivot = Vec3::new(part.pivot.x, -part.pivot.y, 0.);
    let rotation = Quat::from_rotation_z(-angle);
    let scale = Vec3::new(sx, sy, 1.);
    Transform {
        translation: pivot - rotation * (scale * pivot)
            + Vec3::new(
                0.,
                -y * part.landmark.size / landmark_definition(&part.landmark.kind).width,
                z,
            ),
        rotation,
        scale,
    }
}
pub fn alpha(part: &LandmarkPart, elapsed: f32, reduced: bool) -> f32 {
    landmark_pose(
        &landmark_definition(&part.landmark.kind).parts[part.index],
        elapsed,
        reduced,
    )[4]
}
pub fn assemble(
    mut commands: Commands,
    mut studio: ResMut<Studio>,
    time: Res<Time<Real>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut parts: Query<(Entity, &Assembling, &mut Transform)>,
) {
    let now = time.elapsed_secs();
    for (entity, part, mut pose) in &mut parts {
        // Clearing arrival clocks (undo/redo) settles surviving parts immediately.
        let reduced = studio.reduced
            || !studio
                .landmarks
                .arrivals
                .contains_key(&part.part.landmark.id);
        let elapsed = now - part.started;
        *pose = transform(&part.part, elapsed, reduced, pose.translation.z);
        if let Some(mut m) = materials.get_mut(&part.material) {
            m.color = Color::srgba(
                1.,
                1.,
                1.,
                part.opacity * alpha(&part.part, elapsed, reduced),
            );
        }
        let step = &landmark_definition(&part.part.landmark.kind).parts[part.part.index];
        if reduced || elapsed >= step.delay + step.duration {
            commands.entity(entity).remove::<Assembling>();
        }
    }
    if studio.reduced {
        studio.landmarks.arrivals.clear();
    }
    studio
        .landmarks
        .arrivals
        .retain(|_, start| start.is_none_or(|s| now - s < ARRIVAL_LIFETIME));
}
struct Accent {
    motion: u32,
    start: f32,
    end: f32,
    aspect: f32,
}

impl Accent {
    fn new(step: &LandmarkStep) -> Self {
        let (motion, onset, tail, aspect) = match step.motion {
            LandmarkMotion::Rise => (0, 0.04, 0.08, 0.63),
            LandmarkMotion::Drop => (1, 0.60, 0.21, 0.45),
            LandmarkMotion::Settle => (2, 0.22, 0.13, 0.40),
            LandmarkMotion::Turn => (3, 0.06, 0.04, 0.85),
            LandmarkMotion::Unfurl => (UNFURL_SHADER, 0.03, 0.05, 1.10),
            LandmarkMotion::Stamp => (5, 0.24, 0.13, 0.56),
        };
        Self {
            motion,
            start: step.delay + step.duration * onset,
            end: (step.delay + step.duration + tail).min(ARRIVAL_LIFETIME),
            aspect,
        }
    }

    fn progress(&self, age: f32) -> Option<f32> {
        (self.start..self.end)
            .contains(&age)
            .then(|| (age - self.start) / (self.end - self.start))
    }

    fn layout(&self, part: &LandmarkPart, shapes: &[Shape]) -> (Transform, f32) {
        let bounds = shapes
            .iter()
            .filter_map(Shape::bounds)
            .reduce(ContentBounds::union);
        let size = part.landmark.size;
        let span = bounds.map_or(0.2 * size, |b| b.width().max(0.34 * b.height()));
        let width = (span * 1.65).clamp(0.15 * size, 1.1 * size);
        let height = width * self.aspect;
        let direction = bounds.map_or(-1., |b| ((b.min.y + b.max.y) * 0.5 - part.pivot.y).signum());
        let shift = if self.motion == UNFURL_SHADER {
            direction * height * 0.28
        } else {
            0.
        };
        let pose = Transform::from_xyz(part.pivot.x, -part.pivot.y - shift, 0.)
            .with_scale(Vec3::new(width, height, 1.));
        (pose, direction)
    }
}

pub(super) fn replay_duration(part: &LandmarkPart) -> f32 {
    let step = &landmark_definition(&part.landmark.kind).parts[part.index];
    Accent::new(step).end.max(0.7)
}

pub(super) type PartKey = (u64, usize);

struct PartEffect {
    entity: Entity,
    material: Handle<ArrivalMaterial>,
    source: Arc<Vec<Shape>>,
    pose: Transform,
    direction: f32,
    order: usize,
    used: bool,
}

#[derive(Default)]
pub(super) struct ArrivalEffects {
    quad: Option<Handle<Mesh>>,
    active: HashMap<PartKey, PartEffect>,
}

#[derive(SystemParam)]
pub(super) struct ArrivalGraphics<'w, 's> {
    commands: Commands<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<ArrivalMaterial>>,
}

pub(super) fn accents(
    graphics: ArrivalGraphics,
    studio: Res<Studio>,
    canvas: Res<Canvas>,
    time: Res<Time<Real>>,
    mut effects: Local<ArrivalEffects>,
) {
    let ArrivalGraphics {
        mut commands,
        mut meshes,
        mut materials,
    } = graphics;
    let ArrivalEffects { quad, active } = &mut *effects;
    let now = time.elapsed_secs();
    let replay = canvas.replay_landmark_ages();
    if !studio.reduced
        && !studio.home
        && !studio.installing
        && studio.opening.is_none()
        && replay.map_or_else(
            || !studio.landmarks.arrivals.is_empty(),
            |ages| !ages.is_empty(),
        )
    {
        let scene = studio.scene.snapshot();
        for (order, batch) in scene.batches().enumerate() {
            let Some(part) = scene.landmark_part(batch) else {
                continue;
            };
            let key = (part.landmark.id, part.index);
            let age = match replay {
                Some(ages) => ages.get(&key).copied(),
                None => studio
                    .landmarks
                    .arrivals
                    .get(&key.0)
                    .and_then(|start| start.map(|start| now - start)),
            };
            let Some(age) = age else {
                continue;
            };
            let opacity = scene.batch_opacity(batch);
            if opacity <= 0. {
                continue;
            }
            let step = &landmark_definition(&part.landmark.kind).parts[part.index];
            let accent = Accent::new(step);
            let Some(progress) = accent.progress(age) else {
                continue;
            };
            let effect = active.entry(key).or_insert_with(|| {
                let (mut pose, direction) = accent.layout(part, batch);
                pose.translation.z = accent_depth(order);
                let material = materials.add(ArrivalMaterial {
                    ink: ArrivalInk {
                        progress,
                        opacity,
                        seed: ((key.0 % 997 + key.1 as u64 * 37) % 997) as f32 * 0.19,
                        direction,
                        motion: accent.motion,
                    },
                });
                let entity = commands
                    .spawn((
                        WorldContent,
                        Mesh2d(
                            quad.get_or_insert_with(|| meshes.add(Rectangle::new(1., 1.)))
                                .clone(),
                        ),
                        MeshMaterial2d(material.clone()),
                        pose,
                    ))
                    .id();
                PartEffect {
                    entity,
                    material,
                    source: batch.clone(),
                    pose,
                    direction,
                    order,
                    used: true,
                }
            });
            effect.used = true;
            let changed = !Arc::ptr_eq(&effect.source, batch);
            if changed {
                (effect.pose, effect.direction) = accent.layout(part, batch);
                effect.source = batch.clone();
            }
            if changed || effect.order != order {
                effect.order = order;
                effect.pose.translation.z = accent_depth(order);
                commands.entity(effect.entity).insert(effect.pose);
            }
            if let Some(mut material) = materials.get_mut(&effect.material) {
                material.ink.progress = progress;
                material.ink.opacity = opacity;
                material.ink.direction = effect.direction;
                material.ink.motion = accent.motion;
            }
        }
    }
    active.retain(|_, effect| {
        if std::mem::take(&mut effect.used) {
            return true;
        }
        commands.entity(effect.entity).despawn();
        materials.remove(&effect.material);
        false
    });
}

fn accent_depth(order: usize) -> f32 {
    // Draw between the part and its predecessor so later artwork can occlude it.
    (order as f32 - 0.35) * 0.0001
}

#[cfg(test)]
mod tests {
    use super::*;
    use elmapa::{
        document::{Landmark, LandmarkKind},
        scene::landmark_batches,
    };
    #[test]
    fn every_part_gets_a_timed_accent_and_replay_waits_for_its_tail() {
        let mut motions = [false; 6];
        for definition in elmapa::scene::landmark_registry() {
            let landmark = Landmark {
                kind: definition.kind.clone(),
                ..default()
            };
            for batch in landmark_batches(&landmark) {
                let step = &definition.parts[batch.part.index];
                let accent = Accent::new(step);
                motions[accent.motion as usize] = true;
                assert!(accent.start > step.delay);
                assert!(accent.start < step.delay + step.duration);
                assert!(accent.end > step.delay + step.duration);
                assert!(accent.progress(step.delay).is_none());
                assert_eq!(accent.progress(accent.start), Some(0.));
                let mid = (accent.start + accent.end) * 0.5;
                assert!((accent.progress(mid).unwrap() - 0.5).abs() < 0.0001);
                assert!(accent.progress(accent.end).is_none());
                assert!(replay_duration(&batch.part) >= accent.end);
                assert!(replay_duration(&batch.part) <= ARRIVAL_LIFETIME);
                if matches!(step.motion, LandmarkMotion::Drop) {
                    let pose = landmark_pose(step, accent.start, false);
                    assert!(pose[2].abs() < 1., "impact starts near the final pivot");
                }
            }
        }
        assert!(motions.into_iter().all(|used| used));
    }

    #[test]
    fn assembly_finishes_at_export_geometry_and_keeps_fixed_pivots() {
        for def in elmapa::scene::landmark_registry() {
            let kind = &def.kind;
            for batch in landmark_batches(&Landmark {
                kind: kind.clone(),
                ..default()
            }) {
                let part = &batch.part;
                assert_eq!(
                    transform(part, 2., false, 3.),
                    Transform::from_xyz(0., 0., 3.)
                );
                assert_eq!(
                    transform(part, 0., true, 3.),
                    Transform::from_xyz(0., 0., 3.)
                );
                assert_eq!(alpha(part, 0., false), 0.);
                assert_eq!(alpha(part, 0., true), 1.);
                if matches!(
                    landmark_definition(kind).parts[part.index].motion,
                    elmapa::scene::LandmarkMotion::Rise
                        | elmapa::scene::LandmarkMotion::Unfurl
                        | elmapa::scene::LandmarkMotion::Stamp
                ) {
                    for elapsed in [0., 0.1, 0.2, 0.4, 0.6] {
                        let pivot = Vec3::new(part.pivot.x, -part.pivot.y, 0.);
                        let placed = transform(part, elapsed, false, 0.).transform_point(pivot);
                        assert!(placed.distance(pivot) < 0.001);
                    }
                }
            }
        }
    }
    #[test]
    fn reduced_motion_finishes_components_and_prevents_replaying_after_toggle() {
        let mut app = App::new();
        let part = landmark_batches(&Landmark {
            id: 2,
            kind: LandmarkKind::from("castle"),
            ..default()
        })
        .remove(1)
        .part;
        let mut studio = Studio {
            reduced: true,
            ..default()
        };
        studio.landmarks.arrivals.insert(2, Some(0.));
        let mut materials = Assets::<ColorMaterial>::default();
        let material = materials.add(ColorMaterial::default());
        let entity = app
            .world_mut()
            .spawn((
                Assembling {
                    part: part.clone(),
                    material: material.clone(),
                    opacity: 0.4,
                    started: 0.,
                },
                Transform::default(),
            ))
            .id();
        app.insert_resource(studio)
            .insert_resource(materials)
            .init_resource::<Time<Real>>()
            .add_systems(Update, assemble);
        app.update();
        assert!(app.world().get::<Assembling>(entity).is_none());
        assert_eq!(
            app.world().get::<Transform>(entity).unwrap(),
            &Transform::default()
        );
        assert_eq!(
            app.world()
                .resource::<Assets<ColorMaterial>>()
                .get(&material)
                .unwrap()
                .color
                .alpha(),
            0.4
        );
        assert!(
            app.world()
                .resource::<Studio>()
                .landmarks
                .arrivals
                .is_empty()
        );
    }
}
