//! Native UI composition and systems registered by the app.
mod assets;
mod editor;
mod feedback;
mod overlays;
mod shelf;
mod widgets;

use super::{
    files::Jobs,
    state::{Action, Studio},
};
use bevy::prelude::*;
use widgets::{Builder, Palette};

pub use assets::{setup, snap_text_glyphs};
pub use editor::{ChromeMotion, advance_motion, brush_icon, menu_animation};
pub use feedback::{alpha, hover, hover_follow};
pub use overlays::{
    TextCard, landmark_cursor, landmark_selection_follow, path_selection_follow, radial_follow,
    region_selection_follow, text_ime, text_selection_follow, tooltips,
};
pub use shelf::{PreviewSlot, cover_rect};
pub use widgets::color;

#[cfg(test)]
mod tests;

#[derive(Component)]
pub struct UiRoot;
#[derive(Component, Clone, Copy)]
pub struct Hit {
    pub action: Option<Action>,
    pub order: u32,
}
#[derive(Component)]
pub enum ScrollArea {
    Shelf,
    Layers,
    Packs,
    Landmarks,
}
#[derive(Resource, Default)]
pub struct ScrollOffsets {
    pub shelf: f32,
    pub layers: f32,
    pub packs: f32,
    pub landmarks: f32,
}

pub fn rebuild(
    mut commands: Commands,
    mut studio: ResMut<Studio>,
    assets: Res<assets::UiAssets>,
    jobs: Res<Jobs>,
    motion: Res<ChromeMotion>,
    offsets: Res<ScrollOffsets>,
    roots: Query<Entity, With<UiRoot>>,
) {
    studio.refresh_path_nodes(0);
    if !studio.ui_dirty {
        return;
    }
    studio.ui_dirty = false;
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    let root = commands
        .spawn((
            UiRoot,
            UiTargetCamera(assets.camera),
            Node {
                width: percent(100.),
                height: percent(100.),
                position_type: PositionType::Absolute,
                ..default()
            },
        ))
        .id();
    let mut b = Builder::new(&mut commands, &assets, Palette::new(studio.dark));
    // Build in hit order so overlays take precedence over the editor chrome.
    if studio.home {
        shelf::build(&mut b, root, &studio, &jobs, offsets.shelf, offsets.packs);
    } else if studio.opening.is_none() {
        editor::build(&mut b, root, &studio, motion.progress, offsets.layers);
        if !studio.replay_view {
            overlays::build_selection(&mut b, root, &studio, offsets.landmarks);
        }
    }
    overlays::build_feedback(&mut b, root, &studio);
}

pub fn contains(
    point: Vec2,
    node: &ComputedNode,
    transform: &UiGlobalTransform,
    clip: Option<&CalculatedClip>,
) -> bool {
    let point = point / node.inverse_scale_factor();
    Rect::from_center_size(transform.translation, node.size()).contains(point)
        && clip.is_none_or(|clip| clip.clip.contains(point))
}

pub fn dynamic_text(
    studio: Res<Studio>,
    motion: Res<ChromeMotion>,
    mut zoom: Query<&mut Text, With<editor::ZoomLabel>>,
    mut cursor: Query<&mut Node, With<overlays::Cursor>>,
    mut arrows: Query<&mut UiTransform, With<editor::MenuArrow>>,
) {
    for mut text in &mut zoom {
        let desired = format!("{:.0}%", studio.viewport.zoom * 100.);
        if text.0 != desired {
            text.0 = desired;
        }
    }
    overlays::update_brush_cursor(&studio, &mut cursor);
    for mut transform in &mut arrows {
        transform.rotation = Rot2::radians(motion.progress * std::f32::consts::PI);
    }
}
