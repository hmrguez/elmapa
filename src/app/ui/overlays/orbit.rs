//! Glass orbit rendering and reveal animation shared by contextual tools.
use crate::app::{
    radial::{self, Item},
    state::{Action, Studio},
    ui::{
        feedback::Hover,
        widgets::{Builder, color, row, shadow},
    },
};
use bevy::prelude::*;
use elmapa::document::LabelFont;
use std::{collections::HashMap, time::Duration};

/// Geometry supplied by a target adapter; the renderer has no text dependency.
#[derive(Component, Default)]
pub struct Orbit {
    keys: Vec<String>,
    single_ring_radius: f32,
    pub(super) center: Vec2,
    pub(super) radius: Vec2,
    pub(super) hidden: bool,
    pub(super) card_height: f32,
}

impl Orbit {
    fn outer_offset(&self) -> f32 {
        self.keys.len().saturating_sub(1) as f32 * radial::RING_GAP
    }

    fn radius_limit(&self, viewport: Vec2) -> Vec2 {
        ((viewport - Vec2::new(100., 270.)) / 2. - self.outer_offset()).max(Vec2::splat(40.))
    }

    pub(super) fn fit_single_ring(&mut self, viewport: Vec2) {
        if self.single_ring_radius <= self.radius_limit(viewport).min_element() {
            self.radius = self.radius.max(Vec2::splat(self.single_ring_radius));
        }
    }
}

const RESIZE_SETTLE: Duration = Duration::from_millis(750);

/// Retain the displayed size through UI rebuilds. The target's anchor and the
/// viewport can still move while a series of size edits is settling.
#[derive(Default)]
pub struct OrbitResize {
    key: String,
    target_size: f32,
    displayed_size: f32,
    settle_at: Duration,
}

impl OrbitResize {
    pub(super) fn displayed_size(&mut self, orbit: &Orbit, size: f32, now: Duration) -> f32 {
        let key = &orbit.keys[0];
        if self.key != *key || orbit.hidden {
            self.key.clone_from(key);
            self.target_size = size;
            self.settle_at = now;
        } else if self.target_size != size {
            self.target_size = size;
            self.settle_at = now + RESIZE_SETTLE;
        }
        if now >= self.settle_at {
            self.displayed_size = size;
        }
        self.displayed_size
    }
}

#[derive(Clone, Copy)]
enum Element {
    Root,
    Bubble { index: usize, depth: usize },
    Card,
}
#[derive(Component)]
pub struct RadialElement {
    orbit: Entity,
    kind: Element,
}

impl RadialElement {
    pub(super) fn card(orbit: Entity) -> Self {
        Self {
            orbit,
            kind: Element::Card,
        }
    }
}

fn glass_bubble(
    b: &mut Builder,
    parent: Entity,
    item: &Item<Action>,
    action: Action,
    selected_branch: bool,
) -> Entity {
    let active = item.active || selected_branch;
    let p = b.p;
    let id = b.button(parent, "", "", action, active);
    let base = color(if active { p.wash } else { p.surface }, 0.94);
    b.commands.entity(id).insert((
        Node {
            position_type: PositionType::Absolute,
            width: px(36.),
            height: px(36.),
            border: UiRect::all(px(1.)),
            border_radius: BorderRadius::MAX,
            justify_content: JustifyContent::Center,
            ..row(0.)
        },
        BorderColor::all(color(if active { p.accent } else { p.line }, 0.85)),
        BackgroundColor(base),
        shadow(false),
        Hover {
            base,
            target: color(p.wash, 1.),
            tip: item.title.clone(),
            ..default()
        },
    ));
    let hex = if active { p.accent } else { p.text };
    if let Action::BrushMaterial(index) = action {
        b.material_swatch(id, index, 23.);
    } else if let Some(swatch) = item.swatch {
        b.swatch(id, swatch, 16., None);
    } else if let Action::TextFont(font) = action {
        let glyph = b.text(
            id,
            match font {
                LabelFont::Baskerville => "B",
                LabelFont::Didot => "D",
                LabelFont::Avenir => "A",
            },
            19.,
            hex,
            true,
        );
        let handle = match font {
            LabelFont::Baskerville => b.assets.baskerville.clone(),
            LabelFont::Didot => b.assets.didot.clone(),
            LabelFont::Avenir => b.assets.avenir.clone(),
        };
        b.commands
            .entity(glyph)
            .insert(TextFont::from_font_size(19.).with_font(handle));
    } else if item.icon == "ink" {
        for ink in [0x554430, 0x355e72, 0xf6eed9] {
            let dot = b.node(
                id,
                Node {
                    width: px(7.),
                    height: px(14.),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
            );
            b.commands
                .entity(dot)
                .insert(BackgroundColor(color(ink, 1.)));
        }
    } else if item.icon == "typeface" {
        b.text(id, "Aa", 17., hex, true);
    } else {
        b.icon(
            id,
            &item.icon,
            hex,
            if matches!(action, Action::LandmarkKind(_)) {
                30.
            } else {
                16.
            },
        );
    }
    if !item.children.is_empty() {
        let dot = b.node(
            id,
            Node {
                position_type: PositionType::Absolute,
                right: px(4.),
                bottom: px(4.),
                width: px(4.),
                height: px(4.),
                border_radius: BorderRadius::MAX,
                ..default()
            },
        );
        b.commands
            .entity(dot)
            .insert(BackgroundColor(color(p.accent, 0.65)));
    }
    // A fine highlight gives each bubble the same layered glass as the chrome.
    let glint = b.node(
        id,
        Node {
            position_type: PositionType::Absolute,
            top: px(2.),
            left: px(10.),
            width: px(14.),
            height: px(1.),
            ..default()
        },
    );
    b.commands
        .entity(glint)
        .insert(BackgroundColor(color(0xffffff, 0.46)));
    id
}

/// Render every ancestor arc, adding one increasingly outer ring per level.
/// Target adapters only supply the tree, navigation actions and live geometry.
pub(super) fn action_orbit(
    b: &mut Builder,
    root: Entity,
    tree: &[Item<Action>],
    navigation: &radial::Navigation,
    identity: &str,
    enter: fn(usize, usize) -> Action,
    back: Action,
) -> Entity {
    let orbit = b.node(
        root,
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
    );
    let mut keys = Vec::new();
    let mut single_ring_radius: f32 = 0.;
    let mut items = tree;
    for depth in 0..=navigation.path.len() {
        keys.push(format!("{identity}/{:?}", &navigation.path[..depth]));
        let show_back = depth > 0 && depth == navigation.path.len();
        let bubbles = items.len() + usize::from(show_back);
        single_ring_radius = single_ring_radius
            .max(radial::single_ring_radius(bubbles) - depth as f32 * radial::RING_GAP);
        for (index, item) in items.iter().enumerate() {
            let action = item.action.unwrap_or_else(|| enter(depth, index));
            let bubble = glass_bubble(
                b,
                orbit,
                item,
                action,
                navigation.path.get(depth) == Some(&index),
            );
            b.commands.entity(bubble).insert(RadialElement {
                orbit,
                kind: Element::Bubble { index, depth },
            });
        }
        if show_back {
            let title = if back == Action::RadialBack {
                "Close outer ring"
            } else {
                "Close outer ring · Esc"
            };
            let item = Item::action(title, "close", back);
            let bubble = glass_bubble(b, orbit, &item, back, false);
            b.commands.entity(bubble).insert(RadialElement {
                orbit,
                kind: Element::Bubble {
                    index: items.len(),
                    depth,
                },
            });
        }
        let Some(&index) = navigation.path.get(depth) else {
            break;
        };
        let Some(item) = items.get(index) else {
            break;
        };
        items = &item.children;
    }
    b.commands.entity(orbit).insert((
        Orbit {
            keys,
            single_ring_radius,
            ..default()
        },
        RadialElement {
            orbit,
            kind: Element::Root,
        },
    ));
    orbit
}

/// Rebuilds during typing preserve reveal clocks. Each newly opened ring has
/// its own clock, so ancestors stay still as children fill clockwise from top.
pub fn radial_follow(
    studio: Res<Studio>,
    time: Res<Time<Real>>,
    mut clocks: Local<HashMap<String, f32>>,
    targets: Query<&Orbit>,
    mut nodes: Query<(&RadialElement, &mut Node, &mut UiTransform)>,
) {
    clocks.retain(|key, _| targets.iter().any(|target| target.keys.contains(key)));
    for target in &targets {
        for key in &target.keys {
            clocks.entry(key.clone()).or_insert(time.elapsed_secs());
        }
    }
    for (element, mut node, mut transform) in &mut nodes {
        let Ok(target) = targets.get(element.orbit) else {
            continue;
        };
        let outer = target.outer_offset();
        let radius = target.radius.min(target.radius_limit(studio.bounds.size));
        let outer_radius = radius + Vec2::splat(outer);
        let min = outer_radius + Vec2::new(28., 80.);
        let max = studio.bounds.size - outer_radius - Vec2::new(28., 128.);
        let center = target.center.clamp(min.min(max), max.max(min));
        node.display = if target.hidden {
            Display::None
        } else {
            Display::Flex
        };
        match element.kind {
            Element::Root => {
                node.left = px(center.x);
                node.top = px(center.y);
            }
            Element::Bubble { index, depth } => {
                let started = target
                    .keys
                    .get(depth)
                    .and_then(|key| clocks.get(key))
                    .copied()
                    .unwrap_or(time.elapsed_secs());
                let progress = radial::reveal(time.elapsed_secs() - started, index, studio.reduced);
                let offset =
                    radial::offset(index, radius + depth as f32 * radial::RING_GAP, progress);
                node.left = px(offset.x - 18.);
                node.top = px(offset.y - 18.);
                if progress <= 0.02 {
                    node.display = Display::None;
                }
                transform.scale = Vec2::splat(0.65 + 0.35 * progress);
            }
            Element::Card => {
                node.left =
                    px((center.x - 150.).clamp(12., (studio.bounds.size.x - 330.).max(12.)));
                let y = center.y - outer_radius.y - target.card_height;
                node.top = px(if y >= 70. {
                    y
                } else {
                    (center.y + outer_radius.y + 30.).min(studio.bounds.size.y - 290.)
                });
            }
        }
    }
}

#[cfg(test)]
mod tests;
