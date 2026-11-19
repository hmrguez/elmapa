//! Replay this view prototype. Cues animate the retained, final scene.
mod paint_order;
#[cfg(test)]
mod tests;
mod timeline;
pub(in crate::app) mod video;

use super::{
    Canvas, WorldCamera, WorldContent,
    motion::{drop_transform, mountain_transform, path_transform, tree_transform},
};
use crate::app::{
    landmark_render,
    river_material::RiverMaterial,
    state::{Studio, ViewReplay},
};
use bevy::{
    asset::embedded_asset,
    camera::RenderTarget,
    ecs::system::SystemParam,
    prelude::*,
    reflect::TypePath,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin},
};
use elmapa::{
    document::{ContentBounds, Point},
    scene::{
        ForestAnimation, ForestTree, LandmarkPart, MountainAnimation, MountainPart, PathAnimation,
        PathPart, RiverAnimation, RiverReach, Scene,
    },
};
use paint_order::PaintOrder;
use std::collections::{HashMap, VecDeque};
use timeline::timeline;

pub struct InkRevealPlugin;

impl Plugin for InkRevealPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "ink_reveal.wgsl");
        app.add_plugins(Material2dPlugin::<InkReveal>::default());
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct InkReveal {
    /// Progress, layer opacity, leading corner and span along the wash direction.
    #[uniform(0)]
    wash: Vec4,
    /// World-space origin and reciprocal extent of the shared arrival texture.
    #[uniform(0)]
    placement: Vec4,
    #[texture(1)]
    #[sampler(2)]
    texture: Option<Handle<Image>>,
    #[texture(3)]
    #[sampler(4)]
    arrival: Option<Handle<Image>>,
}

impl Material2d for InkReveal {
    fn fragment_shader() -> ShaderRef {
        "embedded://elmapa/app/render/ink_reveal.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

enum Entrance {
    Tree(ForestTree),
    Mountain(MountainPart),
    Trail(PathPart),
    River(RiverReach, Handle<RiverMaterial>),
    Label(Point),
    Landmark(LandmarkPart, Handle<ColorMaterial>, f32),
    Wash {
        original: Handle<ColorMaterial>,
        material: Option<Handle<InkReveal>>,
        order: Option<PaintOrder>,
    },
}

impl Entrance {
    fn step(&self) -> usize {
        match self {
            Self::Tree(tree) => tree.id.2,
            Self::Mountain(peak) => peak.id.3,
            Self::Trail(section) => section.id.3,
            _ => 0,
        }
    }

    fn duration(&self) -> f32 {
        match self {
            Self::Tree(..) | Self::Mountain(_) => 0.33,
            Self::Trail(_) => 0.36,
            Self::River(..) => 0.58,
            Self::Label(_) => 0.30,
            Self::Landmark(part, ..) => landmark_render::replay_duration(part),
            Self::Wash {
                order: Some(order), ..
            } => order.duration,
            Self::Wash { .. } => 0.64,
        }
    }
}

struct Cue {
    entity: Entity,
    z: f32,
    start: f32,
    entrance: Entrance,
    // River distance range, or projected world-space range for a wash.
    span: (f32, f32),
}

pub(super) struct Playback {
    scene: Scene,
    frame: ContentBounds,
    started: f64,
    queued: VecDeque<Cue>,
    moving: Vec<Cue>,
    landmark_ages: HashMap<landmark_render::PartKey, f32>,
}

impl Canvas {
    pub(in crate::app) fn replay_landmark_ages(
        &self,
    ) -> Option<&HashMap<landmark_render::PartKey, f32>> {
        self.view_replay
            .as_ref()
            .map(|replay| &replay.landmark_ages)
    }

    fn artwork(&self) -> impl Iterator<Item = Entity> + '_ {
        self.trees
            .values()
            .map(|tree| tree.entity)
            .chain(self.meshes.values().map(|mesh| mesh.entity))
    }
}

#[derive(SystemParam)]
pub struct PlaybackGraphics<'w, 's> {
    commands: Commands<'w, 's>,
    ink: ResMut<'w, Assets<ColorMaterial>>,
    rivers: ResMut<'w, Assets<RiverMaterial>>,
    washes: ResMut<'w, Assets<InkReveal>>,
    images: ResMut<'w, Assets<Image>>,
    entities: Query<'w, 's, (&'static mut Transform, &'static mut Visibility), With<WorldContent>>,
}

impl PlaybackGraphics<'_, '_> {
    fn pose(&mut self, cue: &mut Cue, age: f32, now: f32, settled: bool) {
        let Ok((mut transform, mut visible)) = self.entities.get_mut(cue.entity) else {
            return;
        };
        if *visible != Visibility::Inherited {
            *visible = Visibility::Inherited;
        }
        let z = cue.z;
        let duration = cue.entrance.duration();
        match &mut cue.entrance {
            Entrance::Tree(tree) => {
                *transform = tree_transform(tree, z, ForestAnimation::replay_factor(tree, age));
            }
            Entrance::Mountain(peak) => {
                *transform = mountain_transform(peak, z, MountainAnimation::entrance(peak, age));
            }
            Entrance::Trail(section) => {
                *transform = path_transform(section, z, PathAnimation::entrance(age));
            }
            Entrance::River(reach, handle) => {
                if let Some(mut material) = self.rivers.get_mut(handle) {
                    let flow = RiverAnimation::replay_pose(reach, cue.span.0, cue.span.1, age);
                    material.set_flow(flow, now);
                }
            }
            Entrance::Label(center) => *transform = drop_transform(*center, age, z),
            Entrance::Landmark(part, handle, opacity) => {
                *transform = landmark_render::transform(part, age, false, z);
                if let Some(mut material) = self.ink.get_mut(handle) {
                    material
                        .color
                        .set_alpha(*opacity * landmark_render::alpha(part, age, false));
                }
            }
            Entrance::Wash {
                original,
                material: wash,
                order,
            } => {
                let mut entity = self.commands.entity(cue.entity);
                if settled {
                    if wash.take().is_some() {
                        entity
                            .remove::<MeshMaterial2d<InkReveal>>()
                            .insert(MeshMaterial2d(original.clone()));
                    }
                } else if let Some(handle) = wash {
                    if let Some(mut material) = self.washes.get_mut(handle) {
                        material.wash.x = age / duration;
                    }
                } else if let Some(material) = self.ink.get(original) {
                    let handle = self.washes.add(InkReveal {
                        wash: Vec4::new(
                            age / duration,
                            material.color.alpha(),
                            cue.span.0,
                            cue.span.1,
                        ),
                        placement: order.as_ref().map_or(Vec4::ZERO, |order| order.position),
                        texture: material.texture.clone(),
                        arrival: order.as_ref().map(|order| order.texture.clone()),
                    });
                    entity
                        .remove::<MeshMaterial2d<ColorMaterial>>()
                        .insert(MeshMaterial2d(handle.clone()));
                    *wash = Some(handle);
                }
            }
        }
    }

    fn visibility(&mut self, canvas: &Canvas, visibility: Visibility) {
        for entity in canvas.artwork() {
            if let Ok((_, mut current)) = self.entities.get_mut(entity)
                && *current != visibility
            {
                *current = visibility;
            }
        }
    }
}

pub fn play(
    mut graphics: PlaybackGraphics,
    mut studio: ResMut<Studio>,
    mut canvas: ResMut<Canvas>,
    mut video: ResMut<video::VideoExport>,
    target: Single<&RenderTarget, With<WorldCamera>>,
    time: Res<Time<Real>>,
) {
    if studio.replay_view != Some(ViewReplay::Playing) && canvas.view_replay.is_none() {
        return;
    }
    let now = time.elapsed_secs_f64();
    let view = studio.viewport.transform(studio.bounds);
    let frame = ContentBounds {
        min: view.map_point(Vec2::ZERO),
        max: view.map_point(studio.bounds.size),
    };
    let available =
        !studio.home && !studio.installing && studio.opening.is_none() && !studio.reduced;
    let restart = video.take_restart();
    if let Some(mut replay) = canvas.view_replay.take() {
        let invalid = !available
            || frame != replay.frame
            || !studio.scene.snapshot().same_snapshot(&replay.scene);
        if restart || invalid || studio.replay_view != Some(ViewReplay::Playing) {
            for cue in &mut replay.moving {
                graphics.pose(cue, cue.entrance.duration(), now as f32, true);
            }
            graphics.visibility(
                &canvas,
                if studio.home || studio.installing || studio.opening.is_some() {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                },
            );
            if !available || invalid && !restart {
                studio.stop_view_replay();
            }
            if studio.replay_view != Some(ViewReplay::Playing) {
                return;
            }
        } else {
            let Some(elapsed) = video.elapsed((now - replay.started) as f32) else {
                canvas.view_replay = Some(replay);
                return;
            };
            while replay
                .queued
                .front()
                .is_some_and(|cue| cue.start <= elapsed)
            {
                replay
                    .moving
                    .push(replay.queued.pop_front().expect("ready cue"));
            }
            replay.landmark_ages.clear();
            replay.moving.retain_mut(|cue| {
                let duration = cue.entrance.duration();
                let age = elapsed - cue.start;
                let settled = age >= duration;
                graphics.pose(cue, age.min(duration), now as f32, settled);
                if !settled && let Entrance::Landmark(part, ..) = &cue.entrance {
                    replay
                        .landmark_ages
                        .insert((part.landmark.id, part.index), age);
                }
                !settled
            });
            let finished = replay.moving.is_empty() && replay.queued.is_empty();
            if finished {
                graphics.visibility(&canvas, Visibility::Inherited);
                studio.replay_view = Some(ViewReplay::Finished);
                studio.ui_dirty = true;
            } else {
                canvas.view_replay = Some(replay);
            }
            video.capture(&mut graphics.commands, &target, finished);
            return;
        }
    }
    if !available {
        studio.stop_view_replay();
        return;
    }
    if studio.scene.is_pending() || !canvas.ready() {
        return;
    }
    let queued = timeline(
        studio.scene.snapshot(),
        &canvas,
        studio.editor.document(),
        frame,
        &mut graphics.images,
    );
    if queued.is_empty() {
        studio.status = "No artwork in this view.".into();
        studio.stop_view_replay();
        return;
    }
    graphics.visibility(&canvas, Visibility::Hidden);
    canvas.view_replay = Some(Playback {
        scene: studio.scene.snapshot().clone(),
        frame,
        started: now,
        queued,
        moving: Vec::new(),
        landmark_ages: HashMap::new(),
    });
}
