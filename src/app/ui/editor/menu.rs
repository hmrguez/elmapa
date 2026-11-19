//! Map actions menu and its retained animation state.
use crate::app::{
    state::{Action, Studio},
    ui::{
        feedback::Hover,
        widgets::{Builder, column},
    },
};
use bevy::{ecs::query::QueryData, prelude::*};

pub(super) fn build(b: &mut Builder, root: Entity, s: &Studio, menu_progress: f32) {
    if s.menu || menu_progress > 0. {
        let menu = b.panel(
            root,
            Node {
                position_type: PositionType::Absolute,
                top: px(78.),
                right: px(20.),
                width: px(214.),
                ..column(0.)
            },
            16.,
            5.,
            0.93,
            true,
        );
        b.commands.entity(menu).insert(MenuSurface);
        for (symbol, label, action) in [
            ("save", "Save", Action::Save),
            ("folder-open", "Open", Action::Open),
            ("export-svg", "Export SVG", Action::Export),
            (
                "atlas",
                if s.editor.document().style.is_classic() {
                    "Use woodland palette"
                } else {
                    "Use classic palette"
                },
                Action::AtlasStyle,
            ),
            ("redo", "Replay this view", Action::ReplayView),
            ("world-new", "New world", Action::Blank),
            ("world-example", "Example world", Action::Demo),
            (
                if s.dark { "sun" } else { "moon" },
                if s.dark {
                    "Light appearance"
                } else {
                    "Dark appearance"
                },
                Action::Theme,
            ),
        ] {
            let button = b.button(menu, symbol, label, action, false);
            b.commands
                .entity(button)
                .entry::<Node>()
                .and_modify(|mut n| {
                    n.justify_content = JustifyContent::Start;
                    n.width = percent(100.);
                });
        }
        b.commands
            .queue(move |world: &mut World| mark_menu(world, menu));
    }
}

#[derive(Resource, Default)]
pub struct ChromeMotion {
    pub(in crate::app::ui) progress: f32,
    from: f32,
    started: f32,
    expanded: bool,
}
#[derive(Component)]
pub struct MenuSurface;
#[derive(Component)]
pub struct MenuInk {
    background: Color,
    text: Option<Color>,
    image: Option<Color>,
    border: Option<BorderColor>,
    shadow: Option<BoxShadow>,
}
pub fn advance_motion(
    mut motion: ResMut<ChromeMotion>,
    mut studio: ResMut<Studio>,
    time: Res<Time<Real>>,
) {
    if motion.expanded != studio.menu {
        motion.expanded = studio.menu;
        motion.from = motion.progress;
        motion.started = time.elapsed_secs();
    }
    let target = if studio.menu { 1. } else { 0. };
    let previous = motion.progress;
    let t = ((time.elapsed_secs() - motion.started) / 0.18).clamp(0., 1.);
    motion.progress = if studio.reduced {
        target
    } else {
        motion.from + (target - motion.from) * (1. - (1. - t).powi(3))
    };
    if previous > 0. && motion.progress == 0. {
        studio.ui_dirty = true;
    }
}
fn mark_menu(world: &mut World, entity: Entity) {
    let ink = MenuInk {
        background: world
            .get::<BackgroundColor>(entity)
            .map_or(Color::NONE, |c| c.0),
        text: world.get::<TextColor>(entity).map(|c| c.0),
        image: world.get::<ImageNode>(entity).map(|i| i.color),
        border: world.get::<BorderColor>(entity).copied(),
        shadow: world.get::<BoxShadow>(entity).cloned(),
    };
    world.entity_mut(entity).insert(ink);
    if let Some(children) = world.get::<Children>(entity) {
        let children: Vec<_> = children.iter().collect();
        for child in children {
            mark_menu(world, child);
        }
    }
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct MenuColors {
    ink: &'static MenuInk,
    hover: Option<&'static Hover>,
    background: Option<&'static mut BackgroundColor>,
    text: Option<&'static mut TextColor>,
    image: Option<&'static mut ImageNode>,
    border: Option<&'static mut BorderColor>,
    shadow: Option<&'static mut BoxShadow>,
}
pub fn menu_animation(
    motion: Res<ChromeMotion>,
    mut nodes: Query<&mut Node, With<MenuSurface>>,
    mut ink: Query<MenuColors>,
) {
    for mut node in &mut nodes {
        node.top = px(78. - 8. * (1. - motion.progress));
    }
    for item in &mut ink {
        let MenuColorsItem {
            ink,
            hover,
            background,
            text,
            image,
            border,
            shadow,
        } = item;
        let fade = |mut c: Color| {
            c.set_alpha(c.alpha() * motion.progress);
            c
        };
        if let Some(mut c) = background {
            c.0 = fade(hover.map_or(ink.background, |h| h.base.mix(&h.target, h.progress)));
        }
        if let (Some(mut c), Some(base)) = (text, ink.text) {
            c.0 = fade(base);
        }
        if let (Some(mut c), Some(base)) = (image, ink.image) {
            c.color = fade(base);
        }
        if let (Some(mut c), Some(base)) = (border, ink.border) {
            *c = BorderColor {
                top: fade(base.top),
                bottom: fade(base.bottom),
                left: fade(base.left),
                right: fade(base.right),
            };
        }
        if let (Some(mut c), Some(base)) = (shadow, &ink.shadow) {
            c.0 = base
                .0
                .iter()
                .cloned()
                .map(|mut s| {
                    s.color = fade(s.color);
                    s
                })
                .collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ui::color;

    #[test]
    fn menu_fade_keeps_text_icons_and_backgrounds_independent() {
        let mut app = App::new();
        app.insert_resource(ChromeMotion {
            progress: 1.,
            ..default()
        })
        .add_systems(Update, menu_animation);
        let text = app
            .world_mut()
            .spawn((
                Node::default(),
                Text::new("Save"),
                TextColor(color(0x303e33, 1.)),
                BackgroundColor(Color::NONE),
            ))
            .id();
        let icon = app
            .world_mut()
            .spawn((
                Node::default(),
                ImageNode {
                    color: color(0x7a8577, 1.),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .id();
        mark_menu(app.world_mut(), text);
        mark_menu(app.world_mut(), icon);
        app.update();
        assert_eq!(
            app.world().get::<TextColor>(text).unwrap().0,
            color(0x303e33, 1.)
        );
        assert_eq!(
            app.world().get::<ImageNode>(icon).unwrap().color,
            color(0x7a8577, 1.)
        );
        app.world_mut().resource_mut::<ChromeMotion>().progress = 0.5;
        app.update();
        assert_eq!(app.world().get::<TextColor>(text).unwrap().0.alpha(), 0.5);
        assert_eq!(
            app.world().get::<BackgroundColor>(text).unwrap().0.alpha(),
            0.
        );
    }
}
