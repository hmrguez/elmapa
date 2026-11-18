//! Hover and opacity feedback shared by shelf and editor widgets.
use super::{Hit, color, contains};
use crate::app::state::Studio;
use bevy::{ecs::query::QueryData, prelude::*};

#[derive(Component, Default)]
pub struct Hover {
    pub(super) from: f32,
    pub(super) started: f32,
    pub(super) hovered: bool,
    pub(super) base: Color,
    pub(super) target: Color,
    pub(super) progress: f32,
    pub(super) tip: String,
}

#[derive(Component)]
pub(super) struct HoverFrame {
    pub(super) from: Color,
    pub(super) to: Color,
    pub(super) card: bool,
}
#[derive(Component)]
pub struct HoverFollow {
    pub(super) parent: Entity,
    pub(super) from: Color,
    pub(super) to: Color,
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct HoverNode {
    entity: Entity,
    node: &'static ComputedNode,
    transform: &'static UiGlobalTransform,
    clip: Option<&'static CalculatedClip>,
    hover: &'static mut Hover,
    background: &'static mut BackgroundColor,
    border: Option<&'static mut BorderColor>,
    shadow: Option<&'static mut BoxShadow>,
    frame: Option<&'static HoverFrame>,
}
pub fn hover(
    studio: Res<Studio>,
    window: Single<&Window>,
    time: Res<Time<Real>>,
    mut query: Query<HoverNode>,
    hits: Query<(
        Entity,
        &Hit,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&CalculatedClip>,
    )>,
    parents: Query<&ChildOf>,
) {
    let point = window.cursor_position().filter(|_| window.focused);
    let top = point.and_then(|point| {
        hits.iter()
            .filter(|(_, _, n, t, c)| contains(point, n, t, *c))
            .max_by_key(|(_, h, _, _, _)| h.order)
            .map(|(e, _, _, _, _)| e)
    });
    for item in &mut query {
        let HoverNodeItem {
            entity,
            node,
            transform,
            clip,
            mut hover,
            mut background,
            border,
            shadow,
            frame,
        } = item;
        let mut covered = true;
        if let Some(mut ancestor) = top {
            loop {
                if ancestor == entity {
                    covered = false;
                    break;
                }
                let Ok(parent) = parents.get(ancestor) else {
                    break;
                };
                ancestor = parent.parent();
            }
        } else {
            covered = false;
        }
        let hovered = !covered && point.is_some_and(|p| contains(p, node, transform, clip));
        let target = if hovered { 1. } else { 0. };
        if hover.hovered != hovered {
            hover.hovered = hovered;
            hover.from = hover.progress;
            hover.started = time.elapsed_secs();
        }
        if hover.progress == target {
            continue;
        }
        let t = ((time.elapsed_secs() - hover.started) / 0.18).clamp(0., 1.);
        hover.progress = if studio.reduced {
            target
        } else {
            hover.from + (target - hover.from) * (1. - (1. - t).powi(3))
        };
        background.0 = hover.base.mix(&hover.target, hover.progress);
        if let Some(frame) = frame {
            if let Some(mut border) = border {
                *border = BorderColor::all(frame.from.mix(&frame.to, hover.progress));
            }
            if frame.card
                && let Some(mut shadow) = shadow
            {
                shadow.0 = vec![ShadowStyle {
                    color: color(0x3d5e70, 0.10 + 0.06 * hover.progress),
                    x_offset: px(0.),
                    y_offset: px(4. + 4. * hover.progress),
                    blur_radius: px(18. + 14. * hover.progress),
                    spread_radius: px(-4.),
                }];
            }
        }
    }
}
pub fn hover_follow(
    parents: Query<&Hover>,
    mut query: Query<(&HoverFollow, &mut BackgroundColor)>,
) {
    for (follow, mut background) in &mut query {
        if let Ok(hover) = parents.get(follow.parent) {
            let desired = follow.from.mix(&follow.to, hover.progress);
            if background.0 != desired {
                background.0 = desired;
            }
        }
    }
}

#[derive(Component)]
pub struct Fade(pub(super) f32);
pub fn alpha(mut commands: Commands, query: Query<(Entity, &Fade), Added<Fade>>) {
    for (entity, alpha) in &query {
        let a = alpha.0;
        commands.queue(move |world: &mut World| fade(world, entity, a));
    }
}
fn fade(world: &mut World, entity: Entity, alpha: f32) {
    if let Some(mut color) = world.get_mut::<ImageNode>(entity) {
        let a = color.color.alpha() * alpha;
        color.color.set_alpha(a);
    }
    if let Some(mut color) = world.get_mut::<TextColor>(entity) {
        let a = color.0.alpha() * alpha;
        color.0.set_alpha(a);
    }
    if let Some(children) = world.get::<Children>(entity) {
        let children: Vec<_> = children.iter().collect();
        for child in children {
            fade(world, child, alpha);
        }
    }
}
