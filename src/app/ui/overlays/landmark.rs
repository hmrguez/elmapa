//! Selection frame and action orbit for a placed landmark.
use super::orbit::{Orbit, action_orbit};
use crate::app::{
    state::{Action, Studio, Tool},
    ui::widgets::{Builder, color},
};
use bevy::prelude::*;

#[derive(Component)]
pub struct LandmarkOrbit;
#[derive(Component)]
pub struct LandmarkSelection;
pub(super) fn build(b: &mut Builder, root: Entity, s: &Studio) {
    if s.tool != Tool::Select || s.current_landmark().is_none() {
        return;
    }
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
    mut frames: Query<&mut Node, With<LandmarkSelection>>,
    mut orbits: Query<&mut Orbit, With<LandmarkOrbit>>,
) {
    let Some(p) = studio.current_landmark() else {
        return;
    };
    let bounds = p.bounds();
    let view = studio.viewport.transform(studio.bounds);
    let a = view.origin + Vec2::new(bounds.min.x, bounds.min.y) * view.scale;
    let size = Vec2::new(bounds.width(), bounds.height()) * view.scale;
    for mut frame in &mut frames {
        frame.left = px(a.x - 6.);
        frame.top = px(a.y - 6.);
        frame.width = px(size.x + 12.);
        frame.height = px(size.y + 12.);
    }
    for mut orbit in &mut orbits {
        orbit.center = a + size / 2.;
        orbit.radius = (size / 2. + Vec2::new(42., 46.)).max(Vec2::new(84., 76.));
        orbit.fit_single_ring(studio.bounds.size);
        orbit.hidden = studio.landmarks.moving.is_some();
    }
}
