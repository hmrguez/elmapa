//! Expandable region rows inside the Layers panel.
use super::layer_row;
use crate::app::{
    regions::RegionAction,
    state::{Action, LayerBranch, Studio},
    ui::{
        components::{Builder, color, row},
        feedback::{Fade, Hover},
    },
};
use bevy::prelude::*;
use elmapa::{
    document::{Layer, RegionParent},
    scene::territories::pigment,
};
use std::collections::HashMap;

pub(super) struct Tree<'a> {
    studio: &'a Studio,
    children: HashMap<RegionParent, &'a Layer>,
}

impl<'a> Tree<'a> {
    pub(super) fn new(studio: &'a Studio) -> Self {
        let children = studio
            .editor
            .document()
            .layers
            .iter()
            .filter_map(|layer| Some((layer.territories.as_ref()?.parent?, layer)))
            .collect();
        Self { studio, children }
    }

    pub(super) fn root_disclosure(&self, layer: &Layer) -> Option<(LayerBranch, bool)> {
        let data = layer.territories.as_ref()?;
        if data.names.is_empty() {
            return None;
        }
        let branch = LayerBranch::Layer(layer.id);
        let open = self
            .studio
            .layer_branches
            .get(&branch)
            .copied()
            .unwrap_or_else(|| self.children.keys().any(|parent| parent.layer == layer.id));
        Some((branch, open))
    }

    pub(super) fn rows(&self, b: &mut Builder, list: Entity, layer: &Layer, depth: usize) {
        let Some(data) = &layer.territories else {
            return;
        };
        let visible = layer.visible && layer.opacity > 0.;
        let selected_owner = (self.studio.regions.active
            && layer.id == self.studio.editor.active_layer())
        .then(|| {
            self.studio
                .regions
                .selection
                .as_ref()
                .and_then(|selection| selection.owner)
        })
        .flatten();
        for (owner, name) in data.names.iter().enumerate() {
            let region = RegionParent {
                layer: layer.id,
                owner,
            };
            let branch = LayerBranch::Region(region);
            let child = self.children.get(&region);
            let expanded = child.is_some()
                && self
                    .studio
                    .layer_branches
                    .get(&branch)
                    .copied()
                    .unwrap_or(false);
            let selected = selected_owner == Some(owner);
            let item = b.node(
                list,
                Node {
                    margin: UiRect::left(px(indent(depth))),
                    height: px(32.),
                    min_width: px(0.),
                    padding: UiRect::horizontal(px(5.)),
                    border: UiRect::all(px(1.)),
                    border_radius: BorderRadius::all(px(8.)),
                    flex_shrink: 0.,
                    ..row(5.)
                },
            );
            let p = b.p;
            let base = color(p.wash, if selected { 1. } else { 0. });
            b.commands.entity(item).insert((
                BackgroundColor(base),
                BorderColor::all(color(p.accent, if selected { 0.58 } else { 0. })),
                Hover {
                    base,
                    target: color(p.wash, 0.74),
                    tip: name.clone(),
                    ..default()
                },
            ));
            b.hit(
                item,
                Some(Action::Regions(RegionAction::SelectRegion(region))),
            );
            if child.is_some() {
                disclosure(b, item, branch, expanded, name);
            } else {
                b.node(
                    item,
                    Node {
                        width: px(22.),
                        flex_shrink: 0.,
                        ..default()
                    },
                );
            }
            let dot = b.node(
                item,
                Node {
                    width: px(8.),
                    height: px(8.),
                    flex_shrink: 0.,
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
            );
            b.commands.entity(dot).insert(BackgroundColor(color(
                pigment(data, owner, b.atlas_style),
                1.,
            )));
            let title = b.text(item, name, 12., p.text, false);
            b.commands.entity(title).insert(Node {
                min_width: px(0.),
                overflow: Overflow::clip(),
                ..default()
            });
            if !visible {
                b.commands.entity(dot).insert(Fade(0.35));
                b.commands.entity(title).insert(Fade(0.35));
            }

            if let Some(child) = child.filter(|_| expanded) {
                layer_row(b, list, self.studio, child, depth + 1, None);
                self.rows(b, list, child, depth + 2);
            }
        }
    }
}

pub(super) fn indent(depth: usize) -> f32 {
    depth.min(5) as f32 * 11.
}

pub(super) fn disclosure(
    b: &mut Builder,
    parent: Entity,
    branch: LayerBranch,
    expanded: bool,
    label: &str,
) {
    let p = b.p;
    let button = b.node(
        parent,
        Node {
            width: px(22.),
            height: px(28.),
            flex_shrink: 0.,
            border_radius: BorderRadius::all(px(6.)),
            justify_content: JustifyContent::Center,
            ..row(0.)
        },
    );
    let verb = if expanded { "Collapse" } else { "Expand" };
    b.commands.entity(button).insert((
        BackgroundColor(Color::NONE),
        Hover {
            base: Color::NONE,
            target: color(p.line, 0.55),
            tip: format!("{verb} {label}"),
            ..default()
        },
    ));
    b.icon(
        button,
        if expanded {
            "chevron-down"
        } else {
            "chevron-right"
        },
        p.accent,
        13.,
    );
    b.hit(button, Some(Action::SetLayerBranch(branch, !expanded)));
}
