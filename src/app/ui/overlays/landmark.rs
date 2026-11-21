//! Selection frame and action orbit for a placed landmark.
use super::orbit::{Orbit, OrbitResize, action_orbit};
use crate::app::{
    state::{Action, Studio},
    ui::widgets::{Builder, color, row},
};
use bevy::prelude::*;

#[derive(Component)]
pub struct LandmarkOrbit;
#[derive(Component)]
pub struct LandmarkSelection;
#[derive(Component)]
pub struct LandmarkNamePrompt;
pub(super) fn build(b: &mut Builder, root: Entity, s: &Studio) {
    let Some(landmark) = s.name_landmark() else {
        return;
    };
    let frame = b.node(
        root,
        Node {
            position_type: PositionType::Absolute,
            border: UiRect::all(px(1.)),
            border_radius: BorderRadius::all(px(8.)),
            ..default()
        },
    );
    b.commands
        .entity(frame)
        .insert((LandmarkSelection, BorderColor::all(color(b.p.accent, 0.45))));
    if s.text.landmark_name {
        return;
    }
    if landmark.name.is_none() {
        let prompt = b.node(
            root,
            Node {
                position_type: PositionType::Absolute,
                width: px(108.),
                height: px(23.),
                justify_content: JustifyContent::Center,
                ..row(6.)
            },
        );
        b.commands.entity(prompt).insert(LandmarkNamePrompt);
        b.icon(prompt, "text", b.p.accent, 12.);
        b.text(prompt, "Type a name", 12., b.p.text, true);
        b.hit(prompt, Some(Action::LandmarkName));
    }
    let orbit = action_orbit(
        b,
        root,
        &s.landmark_actions(),
        &s.landmarks.radial,
        &format!("landmark/{:?}", s.landmarks.selected),
        Action::LandmarkEnter,
        Action::LandmarkBack,
    );
    b.commands.entity(orbit).insert(LandmarkOrbit);
}
pub fn landmark_selection_follow(
    studio: Res<Studio>,
    time: Res<Time<Real>>,
    mut resize: Local<OrbitResize>,
    mut frames: Query<&mut Node, (With<LandmarkSelection>, Without<LandmarkNamePrompt>)>,
    mut orbits: Query<&mut Orbit, With<LandmarkOrbit>>,
    mut prompts: Query<&mut Node, (With<LandmarkNamePrompt>, Without<LandmarkSelection>)>,
) {
    if orbits.is_empty() {
        *resize = default();
    }
    let Some(mut p) = studio.current_landmark() else {
        return;
    };
    let bounds = elmapa::scene::landmark_bounds(&p);
    let view = studio.viewport.transform(studio.bounds);
    let a = view.origin + Vec2::new(bounds.min.x, bounds.min.y) * view.scale;
    let size = Vec2::new(bounds.width(), bounds.height()) * view.scale;
    for mut frame in &mut frames {
        frame.left = px(a.x - 6.);
        frame.top = px(a.y - 6.);
        frame.width = px(size.x + 12.);
        frame.height = px(size.y + 12.);
    }
    if !prompts.is_empty() {
        let label = Studio::landmark_name_label(&p);
        let anchor = view.origin + Vec2::new(label.position.x, label.position.y) * view.scale;
        for mut node in &mut prompts {
            node.left = px(anchor.x - 54.);
            node.top = px(anchor.y - 11.5);
            node.display = if studio.landmarks.moving.is_some() {
                Display::None
            } else {
                Display::Flex
            };
        }
    }
    let target_size = p.size;
    for mut orbit in &mut orbits {
        orbit.hidden = studio.landmarks.moving.is_some();
        p.size = resize.displayed_size(&orbit, target_size, time.elapsed());
        let bounds = if p.size == target_size {
            bounds
        } else {
            elmapa::scene::landmark_bounds(&p)
        };
        let a = view.origin + Vec2::new(bounds.min.x, bounds.min.y) * view.scale;
        let size = Vec2::new(bounds.width(), bounds.height()) * view.scale;
        orbit.center = a + size / 2.;
        orbit.radius = (size / 2. + Vec2::new(42., 46.)).max(Vec2::new(84., 76.));
        orbit.fit_single_ring(studio.bounds.size);
    }
}
