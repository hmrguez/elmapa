//! Replay this view prototype. Cues animate the retained, final scene.
#[cfg(test)]
mod tests;
mod timeline;

use super::{
    Canvas, WorldContent,
    motion::{drop_transform, mountain_transform, path_transform, tree_transform},
};
use crate::app::{landmark_render, river_material::RiverMaterial, state::Studio};
use bevy::{
    asset::embedded_asset,
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
    #[texture(1)]
    #[sampler(2)]
    texture: Option<Handle<Image>>,
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
    Wash(Handle<ColorMaterial>, Option<Handle<InkReveal>>),
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
            Self::Wash(..) => 0.64,
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
            Entrance::Wash(original, wash) => {
                let mut entity = self.commands.entity(cue.entity);
                if settled {
                    if wash.take().is_some() {
                        entity
                            .remove::<MeshMaterial2d<InkReveal>>()
                            .insert(MeshMaterial2d(original.clone()));
                    }
                } else if let Some(handle) = wash {
                    if let Some(mut material) = self.washes.get_mut(handle) {
                        material.wash.x = age / 0.64;
                    }
                } else if let Some(material) = self.ink.get(original) {
                    let handle = self.washes.add(InkReveal {
                        wash: Vec4::new(age / 0.64, material.color.alpha(), cue.span.0, cue.span.1),
                        texture: material.texture.clone(),
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
    time: Res<Time<Real>>,
) {
    if !studio.replay_view && canvas.view_replay.is_none() {
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
    if let Some(mut replay) = canvas.view_replay.take() {
        if !available
            || !studio.replay_view
            || frame != replay.frame
            || !studio.scene.snapshot().same_snapshot(&replay.scene)
        {
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
            studio.stop_view_replay();
            return;
        }

        let elapsed = (now - replay.started) as f32;
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
        if replay.moving.is_empty() && replay.queued.is_empty() {
            graphics.visibility(&canvas, Visibility::Inherited);
            studio.stop_view_replay();
        } else {
            canvas.view_replay = Some(replay);
        }
        return;
    }
    if !studio.replay_view {
        return;
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
