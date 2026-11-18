//! Presentation-only selection bubble, retained across toolbar rebuilds.
use super::state::Studio;
use bevy::prelude::*;

const DURATION: f32 = 0.32;

#[derive(Component)]
pub struct BrushBubble;

#[derive(Component)]
pub struct SelectedTool;

type BubbleFilter = (With<BrushBubble>, Without<SelectedTool>);

#[derive(Resource, Default)]
pub struct BrushMotion {
    from: Rect,
    target: Option<Rect>,
    started: f32,
}

impl BrushMotion {
    fn sample(&self, now: f32) -> Rect {
        let Some(target) = self.target else {
            return self.from;
        };
        let t = ((now - self.started) / DURATION).clamp(0., 1.);
        if t == 0. {
            return self.from;
        }
        if t == 1. {
            return target;
        }
        let eased = 1. - (1. - t).powi(3);
        let center = self.from.center().lerp(target.center(), eased);
        let size = self.from.size().lerp(target.size(), eased);
        // Stretch in transit, then recover the resting shape without overshoot.
        let travel = (target.center().x - self.from.center().x).abs();
        let stretch = (std::f32::consts::PI * t).sin().max(0.);
        Rect::from_center_size(
            center,
            size + Vec2::new(travel.min(60.) * 0.22, -3.) * stretch,
        )
    }

    fn advance(&mut self, target: Rect, now: f32, reduced: bool) -> Rect {
        if self.target.is_none() || reduced {
            self.from = target;
            self.target = Some(target);
            self.started = now - DURATION;
        } else if self.target != Some(target) {
            // Retarget from the visible shape even during rapid brush changes.
            self.from = self.sample(now);
            self.target = Some(target);
            self.started = now;
        }
        self.sample(now)
    }
}

pub fn animate(
    studio: Res<Studio>,
    time: Res<Time<Real>>,
    mut motion: ResMut<BrushMotion>,
    selected: Query<(&ComputedNode, &UiGlobalTransform), With<SelectedTool>>,
    mut bubbles: Query<(&mut ComputedNode, &mut UiGlobalTransform), BubbleFilter>,
) {
    if studio.home || studio.opening.is_some() {
        *motion = BrushMotion::default();
        return;
    }
    let Ok((node, transform)) = selected.single() else {
        return;
    };
    if node.size().min_element() <= 0. {
        return;
    }
    let inverse_scale = node.inverse_scale_factor();
    let target = Rect::from_center_size(
        transform.translation * inverse_scale,
        node.size() * inverse_scale,
    );
    let bubble = motion.advance(target, time.elapsed_secs(), studio.reduced);
    for (mut surface, mut transform) in &mut bubbles {
        // Resize this decorative surface after layout rather than scaling it:
        // scaling a circle turns the original resting highlight into an oval.
        // Reuse the button's exact corner geometry at every device scale.
        surface.size = bubble.size() / inverse_scale;
        surface.unrounded_size = surface.size;
        surface.border_radius = node.border_radius;
        *transform = UiGlobalTransform::from_translation(bubble.center() / inverse_scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(x: f32, width: f32) -> Rect {
        Rect::from_center_size(Vec2::new(x, 900.), Vec2::new(width, 34.))
    }

    #[test]
    fn bubble_travels_stretches_and_settles_at_the_selected_tool() {
        let mut motion = BrushMotion::default();
        let start = slot(100., 75.);
        let end = slot(280., 110.);
        assert_eq!(motion.advance(start, 1., false), start);
        assert_eq!(motion.advance(end, 2., false), start);
        let middle = motion.advance(end, 2. + DURATION / 2., false);
        assert!(middle.center().x > start.center().x && middle.center().x < end.center().x);
        assert!(middle.width() > start.width() + (end.width() - start.width()) * 0.875);
        assert!(middle.height() < 34.);
        assert_eq!(motion.advance(end, 3., false), end);
        assert_eq!(motion.advance(end, 4., false), end);
    }

    #[test]
    fn rapid_switches_continue_from_the_visible_bubble() {
        let mut motion = BrushMotion::default();
        motion.advance(slot(100., 75.), 0., false);
        motion.advance(slot(280., 110.), 1., false);
        let visible = motion.sample(1.1);
        assert_eq!(motion.advance(slot(140., 90.), 1.1, false), visible);
        assert_eq!(motion.advance(slot(140., 90.), 2., false), slot(140., 90.));
    }

    #[test]
    fn reduced_motion_snaps_even_during_an_active_transition() {
        let mut motion = BrushMotion::default();
        motion.advance(slot(100., 75.), 0., false);
        motion.advance(slot(280., 110.), 1., false);
        let end = slot(280., 110.);
        assert_eq!(motion.advance(end, 1.1, true), end);
        assert_eq!(motion.advance(end, 1.2, false), end);
    }

    #[test]
    fn resting_surface_preserves_button_size_and_corners_at_every_scale() {
        for scale in [1., 2.] {
            let mut app = App::new();
            app.insert_resource(Studio {
                home: false,
                ..default()
            })
            .init_resource::<Time<Real>>()
            .init_resource::<BrushMotion>()
            .add_systems(Update, animate);
            let radius = bevy::ui::ResolvedBorderRadius {
                top_left: 10. * scale,
                top_right: 10. * scale,
                bottom_left: 10. * scale,
                bottom_right: 10. * scale,
            };
            let size = Vec2::new(110., 34.) * scale;
            let center = Vec2::new(600., 900.) * scale;
            app.world_mut().spawn((
                SelectedTool,
                ComputedNode {
                    size,
                    inverse_scale_factor: 1. / scale,
                    border_radius: radius,
                    ..default()
                },
                UiGlobalTransform::from_translation(center),
            ));
            let bubble = app
                .world_mut()
                .spawn((
                    BrushBubble,
                    ComputedNode::default(),
                    UiGlobalTransform::default(),
                ))
                .id();
            app.update();
            let surface = app.world().get::<ComputedNode>(bubble).unwrap();
            assert_eq!(surface.size(), size);
            assert_eq!(surface.border_radius, radius);
            assert_eq!(
                *app.world().get::<UiGlobalTransform>(bubble).unwrap(),
                UiGlobalTransform::from_translation(center)
            );
        }
    }
}
