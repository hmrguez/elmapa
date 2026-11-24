//! Shelf settings dialog and its scrollable landmark pack controls.
use super::{Action, Builder, Palette, ScrollArea, Studio, color, column, row};
use bevy::prelude::*;

pub(super) fn build(b: &mut Builder, root: Entity, s: &Studio, p: Palette, pack_scroll: f32) {
    let ink = p.text;
    let overlay = b.node(
        root,
        Node {
            position_type: PositionType::Absolute,
            width: percent(100.),
            height: percent(100.),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
    );
    b.commands
        .entity(overlay)
        .insert(BackgroundColor(color(0x263c36, 0.18)));
    b.hit(overlay, None);
    let settings = b.panel(
        overlay,
        Node {
            width: px(460.),
            max_width: percent(94.),
            max_height: percent(90.),
            ..column(22.)
        },
        24.,
        28.,
        0.98,
        true,
    );
    b.commands
        .entity(settings)
        .insert(BackgroundColor(color(0xf8f8ef, 0.98)));
    b.commands
        .entity(settings)
        .insert(BorderColor::all(color(p.line, 0.65)));
    let header = b.node(
        settings,
        Node {
            width: percent(100.),
            justify_content: JustifyContent::SpaceBetween,
            ..row(0.)
        },
    );
    b.text(header, "Settings", 30., ink, true);
    b.shelf_button(header, "close", "", Action::CloseSettings);
    let appearance = b.node(
        settings,
        Node {
            width: percent(100.),
            justify_content: JustifyContent::SpaceBetween,
            ..row(0.)
        },
    );
    b.text(appearance, "Appearance", 12., ink, false);
    b.appearance_picker(appearance);
    let motion = b.node(
        settings,
        Node {
            width: percent(100.),
            justify_content: JustifyContent::SpaceBetween,
            ..row(0.)
        },
    );
    b.text(motion, "Reduced motion", 12., ink, false);
    b.shelf_button(
        motion,
        if s.reduced { "check" } else { "minus" },
        if s.reduced { "On" } else { "Off" },
        Action::Motion,
    );
    let shader_effects = b.node(
        settings,
        Node {
            width: percent(100.),
            justify_content: JustifyContent::SpaceBetween,
            ..row(0.)
        },
    );
    let enabled = !s.shader_effects_disabled;
    b.text(shader_effects, "Shader effects", 12., ink, false);
    b.shelf_button(
        shader_effects,
        if enabled { "check" } else { "minus" },
        if enabled { "On" } else { "Off" },
        Action::ShaderEffects,
    );
    let sound_effects = b.node(
        settings,
        Node {
            width: percent(100.),
            justify_content: JustifyContent::SpaceBetween,
            ..row(0.)
        },
    );
    let enabled = !s.sound_effects_disabled;
    b.text(sound_effects, "Sound effects", 12., ink, false);
    b.shelf_button(
        sound_effects,
        if enabled { "check" } else { "minus" },
        if enabled { "On" } else { "Off" },
        Action::SoundEffects,
    );
    let catalog = elmapa::scene::landmark_catalog();
    let title = b.node(settings, column(3.));
    b.text(title, "Landmark packs", 19., ink, true);
    b.text(
        title,
        "Choose the packs shown in the landmark catalog.",
        11.,
        p.muted,
        false,
    );
    let packs = b.node(
        settings,
        Node {
            width: percent(100.),
            min_height: px(0.),
            overflow: Overflow::scroll_y(),
            ..column(10.)
        },
    );
    b.commands.entity(packs).insert((
        ScrollArea::Packs,
        ScrollPosition(Vec2::new(0., pack_scroll)),
    ));
    b.hit(packs, None);
    for (i, pack) in catalog.packs.iter().enumerate() {
        let enabled = !s.disabled_landmark_packs.contains(&pack.id);
        let pack_row = b.node(
            packs,
            Node {
                width: percent(100.),
                flex_shrink: 0.,
                justify_content: JustifyContent::SpaceBetween,
                ..row(10.)
            },
        );
        let label = b.node(pack_row, row(10.));
        b.icon(label, &pack.icon_key, ink, 26.);
        let names = b.node(label, column(1.));
        let name = b.text(names, &pack.name, 12., ink, false);
        b.commands.entity(name).insert((
            TextLayout::justify(Justify::Left),
            Node {
                max_width: px(240.),
                ..default()
            },
        ));
        let count = catalog.items.iter().filter(|d| d.pack == i).count();
        b.text(
            names,
            format!("{count} landmark{}", if count == 1 { "" } else { "s" }),
            10.,
            p.muted,
            false,
        );
        b.shelf_button(
            pack_row,
            if enabled { "check" } else { "minus" },
            if enabled { "On" } else { "Off" },
            Action::LandmarkPack(i),
        );
    }
    for error in &catalog.errors {
        let message = b.text(
            packs,
            format!("Pack unavailable: {error}"),
            11.,
            p.muted,
            false,
        );
        b.commands
            .entity(message)
            .insert(TextLayout::justify(Justify::Left));
    }
    b.text(
        settings,
        "Placed landmarks stay on your maps when a pack is off.",
        10.,
        p.muted,
        false,
    );
}
