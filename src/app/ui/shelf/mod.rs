//! Atlas shelf, preview cards and shelf controls.
mod settings;
pub(in crate::app) use settings::update_levels;

use super::{
    ScrollArea,
    components::{Builder, Palette, color, column, row},
    feedback::{Hover, HoverFollow, HoverFrame},
};
use crate::app::{
    branding,
    files::Jobs,
    state::{Action, Studio},
};
use bevy::{
    prelude::*,
    text::{LetterSpacing, LineHeight},
};

#[derive(Component)]
pub struct PreviewSlot(pub u64);

pub(super) fn build(
    b: &mut Builder<'_, '_, '_>,
    root: Entity,
    s: &Studio,
    jobs: &Jobs,
    scroll_offset: f32,
    pack_scroll: f32,
) {
    let p = Palette::shelf();
    let compact = s.bounds.size.x < 1100. || s.bounds.size.y < 760.;
    let ink = p.text;
    let surface = p.surface;
    let background = b.node(
        root,
        Node {
            width: percent(100.),
            height: percent(100.),
            position_type: PositionType::Absolute,
            ..default()
        },
    );
    b.commands
        .entity(background)
        .insert(BackgroundGradient::from(LinearGradient {
            angle: 130_f32.to_radians(),
            stops: vec![color(0xe7eee6, 1.).into(), color(0xf5f4e9, 1.).into()],
            ..default()
        }));
    let ellipse = b.node(
        background,
        Node {
            position_type: PositionType::Absolute,
            top: percent(35.),
            right: percent(-15.),
            width: percent(70.),
            height: percent(90.),
            border_radius: BorderRadius::MAX,
            ..default()
        },
    );
    b.commands
        .entity(ellipse)
        .insert(BackgroundGradient::from(LinearGradient {
            angle: 40_f32.to_radians(),
            stops: vec![color(0xd8e5da, 0.1).into(), color(0xd8e5da, 1.).into()],
            ..default()
        }));
    let shelf = b.node(
        root,
        Node {
            width: percent(100.),
            height: percent(100.),
            padding: UiRect::axes(
                px(if compact { 32. } else { 64. }),
                px(if compact { 24. } else { 40. }),
            ),
            ..column(if compact { 28. } else { 40. })
        },
    );
    let header = b.node(
        shelf,
        Node {
            width: percent(100.),
            justify_content: JustifyContent::SpaceBetween,
            flex_shrink: 0.,
            ..row(24.)
        },
    );
    let identity = b.node(header, row(14.));
    b.icon(
        identity,
        branding::SYMBOL,
        ink,
        if compact { 52. } else { 64. },
    );
    let masthead = b.node(identity, column(2.));
    let wordmark = b.text(
        masthead,
        "ELMAPA",
        if compact { 36. } else { 44. },
        ink,
        true,
    );
    b.commands.entity(wordmark).insert((
        LetterSpacing::Px(if compact { 3.5 } else { 4.5 }),
        LineHeight::RelativeToFont(0.95),
    ));
    let caption = b.node(masthead, row(10.));
    b.text(caption, "Your atlases", 16., p.muted, false);
    let rule = b.node(
        caption,
        Node {
            height: px(1.),
            flex_grow: 1.,
            ..default()
        },
    );
    b.commands
        .entity(rule)
        .insert(BackgroundColor(color(p.muted, 0.35)));
    let actions = b.node(header, row(10.));
    b.shelf_button(actions, "folder-open", "Open atlas…", Action::Open);
    b.shelf_button(actions, "settings", "Settings", Action::Settings);
    let scroll = b.node(
        shelf,
        Node {
            width: percent(100.),
            flex_grow: 1.,
            min_height: px(0.),
            overflow: Overflow::scroll_y(),
            ..default()
        },
    );
    b.commands.entity(scroll).insert((
        ScrollArea::Shelf,
        ScrollPosition(Vec2::new(0., scroll_offset)),
    ));
    let grid = b.node(
        scroll,
        Node {
            width: percent(100.),
            flex_wrap: FlexWrap::Wrap,
            column_gap: px(26.),
            row_gap: px(26.),
            align_content: AlignContent::Start,
            ..default()
        },
    );
    let available = (s.bounds.size.x - if compact { 64. } else { 128. }).max(270.);
    let preferred = (available * 0.31).clamp(270., 520.);
    let columns = ((available + 26.) / (preferred + 26.)).floor().max(1.);
    let width = ((available - (columns - 1.) * 26.) / columns).clamp(270., 520.);
    if let Some(library) = &s.library {
        for record in &library.shelf.atlases {
            let card = b.node(
                grid,
                Node {
                    width: px(width),
                    height: px(if compact { 268. } else { 310. }),
                    min_width: px(270.),
                    max_width: px(520.),
                    padding: UiRect::all(px(9.)),
                    border: UiRect::all(px(1.)),
                    border_radius: BorderRadius::all(px(22.)),
                    flex_shrink: 0.,
                    ..column(13.)
                },
            );
            let base = color(surface, 0.65);
            b.commands.entity(card).insert((
                BackgroundColor(base),
                BorderColor::all(color(p.line, 0.85)),
                BoxShadow(vec![ShadowStyle {
                    color: color(p.muted, 0.1),
                    x_offset: px(0.),
                    y_offset: px(4.),
                    blur_radius: px(18.),
                    spread_radius: px(-4.),
                }]),
                Hover {
                    base,
                    target: color(surface, 0.92),
                    progress: 0.,
                    from: 0.,
                    started: 0.,
                    hovered: false,
                    tip: record.name.clone(),
                },
            ));
            b.commands.entity(card).insert(HoverFrame {
                from: color(p.line, 0.85),
                to: color(p.accent, 0.85),
                card: true,
            });
            b.hit(card, Some(Action::Enter(record.id)));
            let preview = b.node(
                card,
                Node {
                    height: px(if compact { 190. } else { 232. }),
                    width: percent(100.),
                    border_radius: BorderRadius::all(px(15.)),
                    overflow: Overflow::clip(),
                    flex_shrink: 0.,
                    ..default()
                },
            );
            b.commands
                .entity(preview)
                .insert((PreviewSlot(record.id), BackgroundColor(color(0xe6e5d3, 1.))));
            if let Some(image) = jobs.preview(record.id) {
                let image_node = b.node(
                    preview,
                    Node {
                        width: percent(100.),
                        height: percent(100.),
                        border_radius: BorderRadius::all(px(15.)),
                        ..default()
                    },
                );
                b.commands.entity(image_node).insert(ImageNode {
                    image: image.clone(),
                    rect: Some(cover_rect(Vec2::new(
                        width - 20.,
                        if compact { 190. } else { 232. },
                    ))),
                    image_mode: bevy::ui::widget::NodeImageMode::Stretch,
                    ..default()
                });
            }
            let caption = b.node(
                card,
                Node {
                    height: px(44.),
                    width: percent(100.),
                    padding: UiRect::horizontal(px(10.)),
                    justify_content: JustifyContent::SpaceBetween,
                    ..row(8.)
                },
            );
            let name = b.node(
                caption,
                Node {
                    flex_grow: 1.,
                    min_width: px(0.),
                    overflow: Overflow::clip(),
                    ..default()
                },
            );
            b.text(name, &record.name, 23., ink, true);
            let arrow = b.node(
                caption,
                Node {
                    width: px(35.),
                    height: px(35.),
                    border_radius: BorderRadius::MAX,
                    justify_content: JustifyContent::Center,
                    flex_shrink: 0.,
                    ..row(0.)
                },
            );
            b.commands.entity(arrow).insert((
                BackgroundColor(color(0x8fb4a5, 0.19)),
                HoverFollow {
                    parent: card,
                    from: color(0x8fb4a5, 0.19),
                    to: color(0x8fb4a5, 0.31),
                },
            ));
            b.icon(arrow, "chevron-right", ink, 17.);
        }
    }
    let create = b.node(
        grid,
        Node {
            width: px(width),
            height: px(if compact { 268. } else { 310. }),
            min_width: px(270.),
            max_width: px(520.),
            border: UiRect::all(px(1.)),
            border_radius: BorderRadius::all(px(22.)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_shrink: 0.,
            ..column(16.)
        },
    );
    let base = color(surface, 0.22);
    b.commands.entity(create).insert((
        BackgroundColor(base),
        BorderColor::all(color(ink, 0.15)),
        Hover {
            base,
            target: color(surface, 0.58),
            progress: 0.,
            from: 0.,
            started: 0.,
            hovered: false,
            tip: "New atlas".into(),
        },
    ));
    b.commands.entity(create).insert(HoverFrame {
        from: color(ink, 0.15),
        to: color(p.accent, 0.8),
        card: false,
    });
    b.hit(create, Some(Action::Blank));
    let plus = b.node(
        create,
        Node {
            width: px(52.),
            height: px(52.),
            border_radius: BorderRadius::MAX,
            justify_content: JustifyContent::Center,
            ..row(0.)
        },
    );
    b.commands
        .entity(plus)
        .insert(BackgroundColor(color(surface, 0.65)));
    b.icon(plus, "world-new", ink, 23.);
    b.text(create, "New atlas", 18., ink, false);
    if s.status.starts_with("Could not") || jobs.switching() {
        b.text(shelf, &s.status, 12., ink, false);
    }
    if s.settings {
        settings::build(b, root, s, p, pack_scroll);
    }
}

pub fn cover_rect(frame: Vec2) -> Rect {
    let source = Vec2::new(512., 320.);
    let scale = (frame.x / source.x).max(frame.y / source.y);
    Rect::from_center_size(source / 2., frame / scale.max(0.0001))
}
