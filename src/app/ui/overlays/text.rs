//! Label selection handles and contextual text controls.
use super::orbit::{Orbit, OrbitResize, RadialElement, action_orbit};
use crate::app::{
    lettering::{Edit, Field, LabelStyle, selection_size},
    state::{Action, Studio, Tool},
    ui::widgets::{Builder, color, column, row, shadow},
};
use bevy::prelude::*;
use elmapa::{
    document::{Label, LabelFont, LabelInk},
    scene::label_caret,
};

#[derive(Component)]
pub struct TextSelection;
#[derive(Component)]
pub struct TextOrbit;
#[derive(Component)]
pub struct TextCard;

#[derive(Component)]
pub struct InlineNameEdit {
    anchor: Vec2,
    caret: Vec2,
    tilt: f32,
    height: f32,
    selected: bool,
}

pub(super) fn build(b: &mut Builder, root: Entity, s: &Studio) {
    if let Some(draft) = s.text.draft {
        let identity = format!("text draft/{}/{:?}", draft.layer, draft.point);
        let orbit = text_orbit(b, root, s, &identity);
        build_draft(b, root, s, orbit);
        if let Some(label) = s.draft_preview_label() {
            inline_name_edit(b, root, s, &label);
        }
        return;
    }
    if !s.has_text_selection() {
        return;
    }
    let label = s.current_label();
    if !label.text.trim().is_empty() {
        let node = b.node(
            root,
            Node {
                position_type: PositionType::Absolute,
                border: UiRect::all(px(1.)),
                ..default()
            },
        );
        b.commands
            .entity(node)
            .insert((TextSelection, BorderColor::all(color(b.p.accent, 0.72))));
        for (x, y) in [(0., 0.), (100., 0.), (100., 100.), (0., 100.)] {
            let handle = b.node(
                node,
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(x),
                    top: percent(y),
                    margin: UiRect {
                        left: px(-4.5),
                        top: px(-4.5),
                        ..default()
                    },
                    width: px(9.),
                    height: px(9.),
                    border: UiRect::all(px(1.)),
                    border_radius: BorderRadius::all(px(2.5)),
                    ..default()
                },
            );
            b.commands.entity(handle).insert((
                BackgroundColor(color(b.p.surface, 1.)),
                BorderColor::all(color(b.p.accent, 0.9)),
                shadow(false),
            ));
        }
        let above = s.name_above_landmark(&label);
        let edge = if above { 0. } else { 100. };
        let stem = b.node(
            node,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50.),
                top: percent(edge),
                margin: UiRect::top(px(if above { -22. } else { 0. })),
                width: px(1.),
                height: px(22.),
                ..default()
            },
        );
        b.commands
            .entity(stem)
            .insert(BackgroundColor(color(b.p.accent, 0.55)));
        let handle = b.node(
            node,
            Node {
                position_type: PositionType::Absolute,
                left: percent(50.),
                top: percent(edge),
                margin: UiRect {
                    left: px(-7.),
                    top: px(if above { -35. } else { 21. }),
                    ..default()
                },
                width: px(14.),
                height: px(14.),
                border: UiRect::all(px(1.)),
                border_radius: BorderRadius::MAX,
                justify_content: JustifyContent::Center,
                ..row(0.)
            },
        );
        b.commands.entity(handle).insert((
            BackgroundColor(color(b.p.surface, 0.98)),
            BorderColor::all(color(b.p.accent, 0.85)),
            shadow(false),
        ));
        b.icon(handle, "text-rotate", b.p.accent, 10.);
    }
    let identity = if s.text.landmark_name {
        "landmark-name"
    } else {
        "text"
    };
    let orbit = text_orbit(b, root, s, &format!("{identity}/{}", label.id));
    if let Some(field) = s.text.control {
        let panel = edit_card(b, root, orbit);
        let (title, value, step) = style_setting(field, LabelStyle::from(&s.current_label()));
        b.text(panel, title, 17., b.p.text, true);
        let controls = b.node(panel, row(5.));
        text_stepper(b, controls, s, field, value, step);
        if let Some(error) = &s.text.error {
            b.text(panel, error, 10., b.p.muted, false);
        }
    } else if let Some(error) = &s.text.error {
        let panel = edit_card(b, root, orbit);
        b.text(panel, error, 11., b.p.text, false);
    }
    inline_name_edit(b, root, s, &label);
}

fn inline_name_edit(b: &mut Builder, root: Entity, s: &Studio, label: &Label) {
    if let Some(edit) = s.text.edit.as_ref().filter(|edit| edit.kind == Field::Name) {
        let (start, end) = if s.text.composition.is_empty() {
            (edit.anchor, edit.cursor)
        } else {
            let end = edit.anchor.min(edit.cursor) + s.text.composition.len();
            (end, end)
        };
        let (caret, tilt) = label_caret(label, end);
        let selected = start != end;
        let anchor = if selected {
            label_caret(label, start).0
        } else {
            caret
        };
        b.commands.spawn((
            TextSelection,
            InlineNameEdit {
                anchor: Vec2::new(anchor.x, anchor.y),
                caret: Vec2::new(caret.x, caret.y),
                tilt,
                height: label.size * 0.88,
                selected,
            },
            Node {
                position_type: PositionType::Absolute,
                border_radius: BorderRadius::all(px(1.)),
                ..default()
            },
            BackgroundColor(color(b.p.accent, if selected { 0.23 } else { 0.96 })),
            ChildOf(root),
        ));
    }
}

fn edit_card(b: &mut Builder, root: Entity, orbit: Entity) -> Entity {
    let panel = b.panel(
        root,
        Node {
            position_type: PositionType::Absolute,
            ..column(7.)
        },
        17.,
        10.,
        0.96,
        true,
    );
    b.commands
        .entity(panel)
        .insert((RadialElement::card(orbit), TextCard));
    panel
}

fn text_orbit(b: &mut Builder, root: Entity, s: &Studio, identity: &str) -> Entity {
    let orbit = action_orbit(
        b,
        root,
        &s.text_actions(),
        &s.text.radial,
        identity,
        Action::RadialEnter,
        Action::RadialBack,
    );
    b.commands.entity(orbit).insert(TextOrbit);
    orbit
}

fn build_draft(b: &mut Builder, root: Entity, s: &Studio, orbit: Entity) {
    if s.text.control.is_none() && s.text.error.is_none() {
        return;
    }
    let panel = edit_card(b, root, orbit);
    if let Some(field) = s.text.control {
        let (title, value, step) = style_setting(field, s.text.style);
        style_stepper(b, panel, field, title, value, step);
    }
    if let Some(error) = &s.text.error {
        b.text(panel, error, 11., b.p.text, false);
    }
}

pub(super) fn placement_style(b: &mut Builder, root: Entity, s: &Studio) {
    if s.tool != Tool::Text || !s.text.style_open {
        return;
    }
    let guard = b.node(
        root,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100.),
            height: percent(100.),
            ..default()
        },
    );
    b.hit(guard, Some(Action::CloseTextStyle));

    let width = (s.bounds.size.x - 24.).clamp(290., 368.);
    let panel = b.panel(
        root,
        Node {
            position_type: PositionType::Absolute,
            left: px((s.bounds.size.x - width) / 2.),
            bottom: px(crate::app::ui::editor::TEXT_STYLE_BOTTOM),
            width: px(width),
            ..column(7.)
        },
        17.,
        10.,
        0.98,
        true,
    );
    b.commands.entity(panel).insert(GlobalZIndex(30));
    let p = b.p;
    let header = b.node(
        panel,
        Node {
            justify_content: JustifyContent::SpaceBetween,
            ..row(4.)
        },
    );
    b.text(header, "New label style", 20., p.text, true);
    b.button(header, "close", "", Action::CloseTextStyle, false);
    let style = s.text.style;

    let fonts = b.node(
        panel,
        Node {
            flex_wrap: FlexWrap::Wrap,
            ..row(3.)
        },
    );
    for font in LabelFont::ALL {
        b.button(
            fonts,
            "",
            font.family(),
            Action::TextFont(font),
            style.font == font,
        );
    }
    let inks = b.node(
        panel,
        Node {
            flex_wrap: FlexWrap::Wrap,
            ..row(3.)
        },
    );
    for ink in LabelInk::ALL {
        let button = b.button(inks, "", ink.name(), Action::TextInk(ink), style.ink == ink);
        let swatch = b.swatch(button, ink.color_in(b.atlas_style), 10., None);
        b.commands.entity(button).insert_children(0, &[swatch]);
    }
    let (name, value, step) = style_setting(Field::Size, style);
    style_stepper(b, panel, Field::Size, name, value, step);
    b.button(
        panel,
        "chevron-down",
        "Advanced typography",
        Action::TextStyleDetails,
        s.text.style_details,
    );
    if s.text.style_details {
        for field in [Field::Spacing, Field::Opacity, Field::Curvature] {
            let (name, value, step) = style_setting(field, style);
            style_stepper(b, panel, field, name, value, step);
        }
        b.button(panel, "text-reset", "Reset style", Action::TextReset, false);
    }
}

fn style_setting(field: Field, style: LabelStyle) -> (&'static str, String, f32) {
    match field {
        Field::Size => ("Size", format!("{:.0}", style.size), 4.),
        Field::Spacing => ("Letter spacing", format!("{:.1}", style.spacing), 0.5),
        Field::Opacity => ("Ink opacity", format!("{:.0}%", style.opacity * 100.), 0.1),
        Field::Curvature => ("Curvature", format!("{}°", style.curvature), 10.),
        Field::Name => unreachable!("A label name is not a style setting"),
    }
}

fn style_stepper(
    b: &mut Builder,
    parent: Entity,
    field: Field,
    title: &str,
    value: String,
    delta: f32,
) {
    let line = b.node(
        parent,
        Node {
            justify_content: JustifyContent::SpaceBetween,
            ..row(3.)
        },
    );
    b.text(line, title, 12., b.p.text, false);
    let steps = b.node(line, row(3.));
    b.button(steps, "minus", "", Action::TextStep(field, -delta), false);
    let number = b.node(
        steps,
        Node {
            width: px(43.),
            justify_content: JustifyContent::Center,
            ..row(0.)
        },
    );
    b.text(number, value, 11., b.p.text, false);
    b.button(steps, "plus", "", Action::TextStep(field, delta), false);
}

type SelectionNodes<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Node,
        &'static mut UiTransform,
        Option<&'static InlineNameEdit>,
    ),
    With<TextSelection>,
>;

pub fn text_selection_follow(
    studio: Res<Studio>,
    time: Res<Time<Real>>,
    mut resize: Local<OrbitResize>,
    mut nodes: SelectionNodes,
    mut orbits: Query<&mut Orbit, With<TextOrbit>>,
) {
    if orbits.is_empty() {
        *resize = default();
    }
    if nodes.is_empty() && orbits.is_empty() {
        return;
    }
    let view = studio.viewport.transform(studio.bounds);
    let mut label = studio
        .draft_preview_label()
        .unwrap_or_else(|| studio.current_label());
    let center = view.origin + Vec2::new(label.position.x, label.position.y) * view.scale;
    let size = selection_size(&label, view.scale);
    for (mut node, mut transform, inline) in &mut nodes {
        if let Some(edit) = inline {
            let delta = edit.caret - edit.anchor;
            let center = view.origin + (edit.caret + edit.anchor) * (view.scale / 2.);
            let width = if edit.selected {
                (delta.length() * view.scale).max(2.)
            } else {
                1.5
            };
            let height = (edit.height * view.scale).max(11.);
            node.left = px(center.x - width / 2.);
            node.top = px(center.y - height / 2.);
            node.width = px(width);
            node.height = px(height);
            transform.rotation = if edit.selected {
                Rot2::radians(delta.to_angle())
            } else {
                Rot2::degrees(edit.tilt)
            };
            continue;
        }
        node.left = px(center.x - size.x / 2.);
        node.top = px(center.y - size.y / 2.);
        node.width = px(size.x);
        node.height = px(size.y);
        transform.rotation = Rot2::degrees(label.rotation);
    }
    let landmark_bounds = studio
        .name_landmark()
        .filter(|_| studio.text.landmark_name)
        .map(elmapa::scene::landmark_bounds);
    let vertical_gap = if studio.text.landmark_name { 64. } else { 58. };
    let target_size = label.size;
    for mut orbit in &mut orbits {
        orbit.hidden = studio.text.moving.is_some() || studio.text.transforming.is_some();
        label.size = resize.displayed_size(&orbit, target_size, time.elapsed());
        let label_bounds = label.bounds();
        let (orbit_center, bounds) = if let Some(landmark_bounds) = landmark_bounds {
            let bounds = label_bounds.union(landmark_bounds);
            let middle = Vec2::new(bounds.min.x + bounds.max.x, bounds.min.y + bounds.max.y) * 0.5;
            (view.origin + middle * view.scale, bounds)
        } else {
            (center, label_bounds)
        };
        orbit.center = orbit_center;
        orbit.radius = (Vec2::new(bounds.width(), bounds.height()) * view.scale / 2.
            + Vec2::new(48., vertical_gap))
        .max(Vec2::new(84., 76.));
        orbit.card_height = if studio.text.control.is_some() && studio.text.draft.is_none() {
            105.
        } else {
            74.
        };
    }
}

fn text_field(b: &mut Builder, parent: Entity, s: &Studio, kind: Field, value: String, width: f32) {
    let p = b.p;
    let edit = s.text.edit.as_ref().filter(|e| e.kind == kind);
    let field = b.node(
        parent,
        Node {
            width: px(width),
            height: px(34.),
            padding: UiRect::horizontal(px(10.)),
            border: UiRect::all(px(1.)),
            border_radius: BorderRadius::all(px(9.)),
            overflow: Overflow::clip(),
            ..row(0.)
        },
    );
    b.commands.entity(field).insert((
        BackgroundColor(color(p.wash, if edit.is_some() { 0.85 } else { 0.35 })),
        BorderColor::all(color(if edit.is_some() { p.accent } else { p.line }, 0.7)),
    ));
    b.hit(field, Some(Action::TextField(kind)));
    if let Some(edit) = edit {
        edit_text(b, field, edit, &s.text.composition, 12.);
    } else {
        b.text(field, value, 12., p.text, false);
    }
}
pub(super) fn edit_text(b: &mut Builder, field: Entity, edit: &Edit, composition: &str, size: f32) {
    let p = b.p;
    let range = edit.range();
    b.text(field, &edit.value[..range.start], size, p.text, false);
    if !composition.is_empty() {
        b.text(field, composition, size, p.accent, false);
    } else if !range.is_empty() {
        let selection = b.node(field, row(0.));
        b.commands
            .entity(selection)
            .insert(BackgroundColor(color(p.accent, 0.22)));
        b.text(selection, &edit.value[range.clone()], size, p.text, false);
    } else {
        let caret = b.node(
            field,
            Node {
                width: px(1.),
                height: px(size + 2.),
                flex_shrink: 0.,
                ..default()
            },
        );
        b.commands
            .entity(caret)
            .insert(BackgroundColor(color(p.accent, 1.)));
    }
    b.text(field, &edit.value[range.end..], size, p.text, false);
}

fn text_stepper(
    b: &mut Builder,
    parent: Entity,
    s: &Studio,
    kind: Field,
    value: String,
    delta: f32,
) {
    let group = b.node(parent, row(3.));
    b.button(group, "minus", "", Action::TextStep(kind, -delta), false);
    text_field(b, group, s, kind, value, 65.);
    b.button(group, "plus", "", Action::TextStep(kind, delta), false);
}

/// Enable composition before the first keystroke on a selected label.
pub fn text_ime(studio: Res<Studio>, mut window: Single<&mut Window>) {
    let draft = studio.text.draft;
    let region = studio
        .regions
        .style
        .edit
        .as_ref()
        .and(studio.regions.style.anchor);
    let enabled = studio.replay_view.is_none()
        && (region.is_some()
            || draft.is_some()
            || studio.selected_label().is_some()
            || studio.name_landmark().is_some());
    if window.ime_enabled != enabled {
        window.ime_enabled = enabled;
    }
    if enabled {
        let view = studio.viewport.transform(studio.bounds);
        let position = region.unwrap_or_else(|| {
            draft.map_or_else(
                || {
                    studio
                        .name_landmark()
                        .filter(|_| !studio.text.landmark_name)
                        .map_or_else(|| studio.current_label(), Studio::landmark_name_label)
                        .position
                },
                |draft| draft.point,
            )
        });
        window.ime_position = (view.origin + Vec2::new(position.x, position.y) * view.scale).clamp(
            Vec2::splat(30.),
            (studio.bounds.size - Vec2::splat(50.)).max(Vec2::splat(30.)),
        );
    }
}
