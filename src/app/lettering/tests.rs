use super::{
    Field, Handle, selection_size,
    transform::{map_point, vector},
};
use crate::app::{
    files::Jobs,
    state::{Action, Studio, Tool},
};
use bevy::prelude::{Rot2, Vec2, default};
use elmapa::document::{Label, LabelFont, LabelInk, Point};

fn studio() -> Studio {
    Studio {
        home: false,
        ..default()
    }
}
fn place_and_select(s: &mut Studio, jobs: &mut Jobs, position: Vec2) -> u64 {
    s.act(Action::TextTool, jobs);
    s.down(position);
    s.up(position, false);
    s.text_insert("Place name");
    s.act(Action::PlaceLabel, jobs);
    assert_eq!(s.tool, Tool::Text);
    s.act(Action::SelectTool, jobs);
    s.down(position);
    s.up(position, false);
    s.text.selected.unwrap()
}

#[test]
fn ten_label_drafts_reuse_style_and_commit_once_each() {
    let mut s = studio();
    let mut jobs = Jobs::default();
    s.act(Action::TextTool, &mut jobs);
    s.act(Action::TextFont(LabelFont::Didot), &mut jobs);
    s.act(Action::TextInk(LabelInk::Blue), &mut jobs);
    s.act(Action::TextStep(Field::Size, 4.), &mut jobs);
    let revision = s.editor.revision();
    for index in 0..10 {
        s.annotation_press(Point::new(15. + 100. * index as f32, 200.), false);
        assert!(s.text.edit.as_ref().unwrap().value.is_empty());
        assert_eq!(s.editor.document().layers[0].labels.len(), index);
        s.text_insert(&format!("Island {index}"));
        assert!(s.text.selected.is_none());
        assert_eq!(s.tool, Tool::Text);
    }
    s.act(Action::PlaceLabel, &mut jobs);
    let labels = &s.editor.document().layers[0].labels;
    assert_eq!(labels.len(), 10);
    assert_eq!(s.editor.revision(), revision + 10);
    assert!(labels.iter().all(|label| label.font == LabelFont::Didot
        && label.ink == LabelInk::Blue
        && label.size == 36.));
    assert_eq!(labels[9].text, "Island 9");

    s.annotation_press(Point::new(20., 30.), false);
    s.text_insert("Cancelled");
    s.cancel_label_draft();
    assert_eq!(s.editor.revision(), revision + 10);
    s.act(Action::Undo, &mut jobs);
    assert_eq!(s.editor.document().layers[0].labels.len(), 9);
    s.act(Action::Redo, &mut jobs);
    assert_eq!(s.editor.document().layers[0].labels.len(), 10);
}

#[test]
fn editing_an_existing_label_changes_placement_style_only_when_reused() {
    let mut s = studio();
    let mut jobs = Jobs::default();
    let position = Vec2::new(400., 400.);
    let id = place_and_select(&mut s, &mut jobs, position);
    s.act(Action::TextInk(LabelInk::Blue), &mut jobs);
    s.text_insert("Renamed");
    s.commit_text_field();
    assert_eq!(s.editor.label(id).unwrap().text, "Renamed");
    assert_eq!(s.text.style.ink, LabelInk::Sepia);
    s.act(Action::TextReuseStyle, &mut jobs);
    s.act(Action::TextTool, &mut jobs);
    assert_eq!(s.text.style.ink, LabelInk::Blue);
    s.down(position);
    s.up(position, false);
    assert!(s.text.edit.as_ref().unwrap().value.is_empty());
    s.text_insert("New name");
    s.act(Action::PlaceLabel, &mut jobs);
    let labels = &s.editor.document().layers[0].labels;
    assert_eq!(labels.len(), 2);
    assert_eq!(labels[1].position, labels[0].position);
    assert_eq!(labels[1].text, "New name");
    assert_eq!(labels[1].ink, LabelInk::Blue);
}

#[test]
fn rejected_draft_preserves_its_name_and_original_layer() {
    let mut s = studio();
    let mut jobs = Jobs::default();
    let layer = s.editor.active_layer();
    let another = s.editor.add_layer();
    s.editor.select_layer(layer);
    s.act(Action::TextTool, &mut jobs);
    let position = Point::new(40., 80.);
    s.annotation_press(position, false);
    s.text_insert("Salt Marsh");
    s.act(Action::Visibility(layer), &mut jobs);
    s.annotation_press(Point::new(70., 20.), false);
    assert_eq!(s.draft_label().unwrap().position, position);
    assert_eq!(s.draft_label().unwrap().text, "Salt Marsh");
    assert!(s.text.error.is_some());
    s.act(Action::Visibility(layer), &mut jobs);
    s.editor.select_layer(another);
    s.act(Action::PlaceLabel, &mut jobs);
    assert_eq!(s.editor.document().layer(layer).unwrap().labels.len(), 1);
    assert!(
        s.editor
            .document()
            .layer(another)
            .unwrap()
            .labels
            .is_empty()
    );
    assert_eq!(s.editor.active_layer(), another);
}

#[test]
fn select_only_picks_visible_labels_and_empty_canvas_deselects_without_editing() {
    let mut s = studio();
    let mut jobs = Jobs::default();
    let point = Point::new(100., 200.);
    let lower_layer = s.editor.active_layer();
    let lower = s
        .editor
        .add_label(Label {
            position: point,
            ..default()
        })
        .unwrap();
    let upper_layer = s.editor.add_layer();
    let upper = s
        .editor
        .add_label(Label {
            position: point,
            ..default()
        })
        .unwrap();
    let revision = s.editor.revision();
    s.act(Action::SelectTool, &mut jobs);
    assert!(s.selected_label().is_none());
    s.annotation_press(point, false);
    s.finish_text_move();
    assert_eq!(s.text.selected, Some(upper));
    assert!(s.selected_label().is_some());
    assert_eq!(s.editor.revision(), revision);
    s.act(Action::Visibility(upper_layer), &mut jobs);
    assert!(s.selected_label().is_none());
    s.annotation_press(point, false);
    s.finish_text_move();
    assert_eq!(s.text.selected, Some(lower));
    assert_eq!(s.editor.active_layer(), lower_layer);
    let revision = s.editor.revision();
    s.down(Vec2::new(900., 400.));
    s.moved(Vec2::new(950., 450.), false);
    s.up(Vec2::new(950., 450.), false);
    assert!(s.text.selected.is_none());
    assert!(s.selected_label().is_none());
    assert!(s.stroke.is_none());
    assert_eq!(s.editor.revision(), revision);
}
#[test]
fn selected_text_edits_and_moves_with_history_and_text_tool_rearms_placement() {
    let mut s = studio();
    let mut jobs = Jobs::default();
    let id = place_and_select(&mut s, &mut jobs, Vec2::new(400., 400.));
    let original = s.editor.label(id).unwrap().clone();
    s.act(Action::Brush(elmapa::document::Brush::Forest), &mut jobs);
    s.act(Action::SelectTool, &mut jobs);
    s.down(Vec2::new(400., 400.));
    s.up(Vec2::new(400., 400.), false);
    assert_eq!(s.text.selected, Some(id));
    s.text_insert("The North");
    s.commit_text_field();
    assert_eq!(s.editor.label(id).unwrap().text, "The North");
    let revision = s.editor.revision();
    s.down(Vec2::new(400., 400.));
    s.moved(Vec2::new(440., 420.), false);
    s.up(Vec2::new(480., 450.), false);
    assert_eq!(s.editor.revision(), revision + 1);
    assert_ne!(s.editor.label(id).unwrap().position, original.position);
    s.act(Action::Undo, &mut jobs);
    assert_eq!(s.editor.label(id).unwrap().position, original.position);
    s.act(Action::TextTool, &mut jobs);
    assert_eq!(s.tool, Tool::Text);
    assert!(s.text.selected.is_none());
    assert!(s.text.edit.is_none());
}
#[test]
fn moving_and_live_name_drafts_commit_once_and_escape_reverts() {
    let mut s = studio();
    let mut jobs = Jobs::default();
    let id = place_and_select(&mut s, &mut jobs, Vec2::new(400., 400.));
    let original = s.editor.label(id).unwrap().clone();
    s.text_insert("P1 [Île]");
    assert_eq!(s.editor.label(id), Some(&original));
    assert_eq!(s.current_label().text, "P1 [Île]");
    s.commit_text_field();
    assert_eq!(s.editor.label(id).unwrap().text, "P1 [Île]");
    let revision = s.editor.revision();
    s.down(Vec2::new(400., 400.));
    s.moved(Vec2::new(440., 420.), false);
    s.moved(Vec2::new(480., 450.), false);
    assert_eq!(s.editor.revision(), revision);
    s.up(Vec2::new(480., 450.), false);
    assert_eq!(s.editor.revision(), revision + 1);
    assert_ne!(s.editor.label(id).unwrap().position, original.position);
    s.act(Action::Undo, &mut jobs);
    assert_eq!(s.editor.label(id).unwrap().position, original.position);
    s.down(Vec2::new(400., 400.));
    s.moved(Vec2::new(600., 500.), false);
    s.cancel_text();
    assert_eq!(s.editor.label(id).unwrap().position, original.position);
    assert!(s.text.preview.is_none());
    s.act(Action::Undo, &mut jobs);
    assert_eq!(s.editor.label(id).unwrap().text, "Place name");
    s.act(Action::Undo, &mut jobs);
    assert!(s.editor.label(id).is_none());
}
#[test]
fn corner_drag_preserves_opposite_anchor_at_rotation_and_zoom_and_commits_once() {
    for zoom in [0.5, 1., 3.] {
        let mut s = studio();
        s.viewport.resolve(s.bounds);
        s.viewport.zoom = zoom;
        s.tool = Tool::Select;
        let label = Label {
            position: Point::new(100., 200.),
            rotation: 37.,
            spacing: 2.,
            ..default()
        };
        let id = s.editor.add_label(label.clone()).unwrap();
        s.text.selected = Some(id);
        let half = selection_size(&label, zoom) / (2. * zoom);
        let rotation = Rot2::degrees(label.rotation);
        let handle = vector(label.position) + rotation * half;
        assert_eq!(
            s.text_handle(map_point(handle)),
            Some(Handle::Corner(1., 1.))
        );
        let extent = (selection_size(&label, zoom) - Vec2::splat(12.)) / zoom;
        let diagonal = rotation * extent;
        let anchor = vector(label.position) - diagonal / 2.;
        let revision = s.editor.revision();
        s.annotation_press(map_point(handle), false);
        s.text_move(map_point(handle + diagonal * 0.5));
        let preview = s.current_label();
        assert!((preview.size - label.size * 1.5).abs() < 0.001);
        assert!(vector(preview.position).distance(anchor + diagonal * 0.75) < 0.001);
        assert_eq!(s.editor.revision(), revision);
        s.finish_text_move();
        assert_eq!(s.editor.revision(), revision + 1);
        assert!(s.editor.undo());
        assert_eq!(s.editor.label(id).unwrap().size, label.size);
    }
}
#[test]
fn rotation_drag_snaps_wraps_and_escape_reverts_without_history() {
    let mut s = studio();
    s.viewport.resolve(s.bounds);
    s.tool = Tool::Select;
    let label = Label {
        position: Point::new(100., 200.),
        rotation: 170.,
        ..default()
    };
    let id = s.editor.add_label(label.clone()).unwrap();
    s.text.selected = Some(id);
    let zoom = s.viewport.zoom;
    let local = Vec2::new(0., selection_size(&label, zoom).y / 2. + 28.) / zoom;
    let handle = vector(label.position) + Rot2::degrees(label.rotation) * local;
    assert_eq!(s.text_handle(map_point(handle)), Some(Handle::Rotate));
    let revision = s.editor.revision();
    s.annotation_press(map_point(handle), false);
    s.text.snap = true;
    let end = vector(label.position) + Rot2::degrees(30.) * (handle - vector(label.position));
    s.text_move(map_point(end));
    assert_eq!(s.current_label().rotation, -165.);
    assert_eq!(s.editor.revision(), revision);
    s.cancel_text();
    assert_eq!(s.editor.label(id).unwrap(), &Label { id, ..label });
    assert!(s.text.transforming.is_none());
    assert_eq!(s.editor.revision(), revision);
}
#[test]
fn captured_resize_can_finish_over_chrome_without_painting() {
    let mut s = studio();
    s.viewport.resolve(s.bounds);
    s.tool = Tool::Select;
    let label = Label {
        position: s.map_point(Vec2::new(400., 400.)),
        ..default()
    };
    let id = s.editor.add_label(label.clone()).unwrap();
    s.text.selected = Some(id);
    let half = selection_size(&label, s.viewport.zoom) / 2.;
    let handle = Vec2::new(400., 400.) + half;
    s.down(handle);
    assert!(s.text.transforming.is_some());
    s.moved(handle + Vec2::new(50., 20.), true);
    s.up(handle + Vec2::new(80., 30.), true);
    assert!(s.editor.label(id).unwrap().size > label.size);
    assert!(s.text.transforming.is_none());
    assert!(s.stroke.is_none());
    assert!(s.editor.undo());
    assert_eq!(s.editor.label(id).unwrap().size, label.size);
}
#[test]
fn exact_numeric_values_duplicate_and_delete_remain_undoable() {
    let mut s = studio();
    let mut jobs = Jobs::default();
    let id = place_and_select(&mut s, &mut jobs, Vec2::new(400., 400.));
    for (field, value) in [
        (Field::Spacing, "3.5"),
        (Field::Opacity, "87.5"),
        (Field::Curvature, "-45.5"),
    ] {
        s.focus_text(field);
        s.text_insert(value);
        s.commit_text_field();
    }
    let l = s.editor.label(id).unwrap();
    assert_eq!((l.spacing, l.opacity, l.curvature), (3.5, 0.875, -45.5));
    let revision = s.editor.revision();
    s.focus_text(Field::Opacity);
    s.dismiss_text_control();
    assert_eq!(s.editor.revision(), revision);
    s.act(Action::TextDuplicate, &mut jobs);
    let duplicate = s.text.selected.unwrap();
    assert_ne!(id, duplicate);
    assert_eq!(s.editor.label(duplicate).unwrap().curvature, -45.5);
    s.act(Action::TextDelete, &mut jobs);
    assert!(s.editor.label(duplicate).is_none());
    s.act(Action::Undo, &mut jobs);
    assert!(s.editor.label(duplicate).is_some());
    s.text.selected = Some(id);
    s.focus_text(Field::Spacing);
    s.text_insert("NaN");
    s.commit_text_field();
    assert_eq!(s.editor.label(id).unwrap().spacing, 3.5);
    s.act(Action::TextReset, &mut jobs);
    assert_eq!(s.editor.label(id).unwrap().curvature, 0.);
    s.act(Action::Undo, &mut jobs);
    assert_eq!(s.editor.label(id).unwrap().curvature, -45.5);
}
