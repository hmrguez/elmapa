use super::*;
use crate::document::{Brush, Point};

fn stroke() -> Stroke {
    Stroke {
        creation_id: None,
        material: None,
        brush: Brush::Forest,
        points: vec![Point::new(100.0, 100.0)].into(),
        size: 80.0,
        strength: 0.7,
        seed: 42,
    }
}
#[test]
fn a_stroke_is_one_undo_transaction_and_new_edits_drop_redo() {
    let mut editor = Editor::blank();
    assert!(editor.commit_stroke(stroke()));
    assert_eq!(editor.document.layers[0].strokes.len(), 1);
    assert!(editor.undo());
    assert!(editor.document.layers[0].strokes.is_empty());
    assert!(editor.redo());
    assert_eq!(editor.document.layers[0].strokes[0].seed, 42);
    assert_eq!(editor.document.layers[0].strokes[0].creation_id, Some(2));
    assert!(editor.undo());
    editor.add_layer();
    assert!(!editor.redo());
}
#[test]
fn replacing_world_restores_previous_world_selection_and_earlier_history() {
    let mut editor = Editor::blank();
    let active = editor.add_layer();
    editor.commit_stroke(stroke());
    let previous = editor.document.clone();
    let replacement = Document::demo();
    assert!(editor.replace_document(replacement.clone()));
    assert_eq!(editor.active_layer, replacement.layers[0].id);
    assert!(editor.undo());
    assert_eq!(editor.document, previous);
    assert_eq!(editor.active_layer, active);
    assert!(editor.undo());
    assert!(editor.document.layer(active).unwrap().strokes.is_empty());
    assert!(editor.redo());
    assert!(editor.redo());
    assert_eq!(editor.document, replacement);
}
#[test]
fn invalid_replacement_preserves_document_and_redo() {
    let mut editor = Editor::blank();
    editor.commit_stroke(stroke());
    editor.undo();
    let before = editor.document.clone();
    let mut invalid = Document::demo();
    invalid.version = 999;
    assert!(!editor.replace_document(invalid));
    assert!(!editor.replace_document(before.clone()));
    assert_eq!(editor.document, before);
    assert!(editor.redo());
}
#[test]
fn final_layer_is_protected_and_deleting_active_layer_is_undoable() {
    let mut editor = Editor::blank();
    assert!(!editor.remove_layer(1));
    let added = editor.add_layer();
    assert!(editor.remove_layer(added));
    assert_eq!(editor.active_layer, 1);
    assert!(editor.undo());
    assert_eq!(editor.active_layer, added);
    assert_eq!(editor.document.layers.len(), 2);
}
#[test]
fn hidden_layers_reject_paint_and_visibility_is_undoable() {
    let mut editor = Editor::blank();
    assert!(editor.toggle_layer(1));
    assert!(!editor.commit_stroke(stroke()));
    assert!(editor.undo());
    assert!(editor.commit_stroke(stroke()));
}

#[test]
fn construction_and_selection_reject_invalid_state() {
    let mut invalid = Document::blank();
    invalid.layers.clear();
    assert!(Editor::new(invalid).is_err());

    let mut editor = Editor::blank();
    let added = editor.add_layer();
    assert!(editor.select_layer(1));
    assert!(!editor.select_layer(999));
    assert_eq!(editor.active_layer(), 1);
    assert!(editor.select_layer(added));
    assert!(editor.undo());
    assert_eq!(editor.active_layer(), 1);
    assert_eq!(editor.document().layers.len(), 1);
}

#[test]
fn invalid_strokes_leave_document_and_redo_intact() {
    let mut editor = Editor::blank();
    editor.commit_stroke(stroke());
    editor.undo();
    let before = editor.document().clone();
    let mut invalid = stroke();
    invalid.points[0].x = f32::NAN;
    assert!(!editor.commit_stroke(invalid));
    assert_eq!(editor.document(), &before);
    assert!(editor.redo());
}

#[test]
fn history_retains_the_most_recent_sixty_four_transactions() {
    let mut editor = Editor::blank();
    for _ in 0..70 {
        assert!(editor.commit_stroke(stroke()));
    }
    for _ in 0..64 {
        assert!(editor.undo());
    }
    assert!(!editor.undo());
    assert_eq!(editor.document().layers[0].strokes.len(), 6);
    for _ in 0..64 {
        assert!(editor.redo());
    }
    assert!(!editor.redo());
    assert_eq!(editor.document().layers[0].strokes.len(), 70);
}

#[test]
fn history_and_snapshots_share_unchanged_point_payloads_without_changing_json() {
    let mut document = Document::blank();
    let mut paint = stroke();
    paint.points = (0..100_000)
        .map(|i| crate::document::Point::new(i as f32, 0.))
        .collect();
    document.layers[0].strokes.push(paint);
    let snapshot = document.clone();
    assert!(snapshot.layers.ptr_eq(&document.layers));
    let mut editor = Editor::new(document).unwrap();
    for i in 0..64 {
        assert!(editor.set_layer_opacity(1, if i % 2 == 0 { 0.4 } else { 0.5 }));
    }
    assert!(
        editor.document().layers[0].strokes[0]
            .points
            .ptr_eq(&snapshot.layers[0].strokes[0].points)
    );
    assert!(
        editor.history_bytes() < 2_000_000,
        "metadata edits duplicated point arrays"
    );
    assert!(editor.undo());
    assert!(editor.redo());
    let encoded = serde_json::to_value(editor.document()).unwrap();
    assert!(encoded["layers"].is_array());
    assert!(encoded["layers"][0]["strokes"][0]["points"].is_array());
    let decoded: Document = serde_json::from_value(encoded).unwrap();
    assert_eq!(&decoded, editor.document());
}

#[test]
fn transparent_layers_accept_label_edits_but_reject_landmark_edits() {
    use crate::document::{Label, Landmark};

    let mut editor = Editor::blank();
    let landmark_id = editor.add_landmark(Landmark::default()).unwrap();
    assert!(editor.set_layer_opacity(1, 0.));

    let label_id = editor.add_label(Label::default()).unwrap();
    let mut label = editor.label(label_id).unwrap().clone();
    label.text = "Stillwater".into();
    assert!(editor.update_label(label));
    assert_eq!(editor.label_at(Point::new(0., 0.), 1.), None);
    assert_eq!(editor.landmark_at(Point::new(0., 0.), 1.), None);
    assert!(editor.delete_label(label_id));

    let mut landmark = editor.landmark(landmark_id).unwrap().clone();
    landmark.size += 1.;
    let revision = editor.revision();
    assert!(!editor.update_landmark(landmark));
    assert!(!editor.delete_landmark(landmark_id));
    assert_eq!(editor.add_landmark(Landmark::default()), None);
    assert_eq!(editor.revision(), revision);
}

#[test]
fn document_id_exhaustion_preserves_undo_and_redo() {
    use crate::document::{Label, Landmark};

    let mut document = Document::blank();
    document.next_id = u64::MAX - 1;
    let mut editor = Editor::new(document).unwrap();

    assert_eq!(editor.add_label(Label::default()), Some(u64::MAX - 1));
    let exhausted = editor.document().clone();
    assert_eq!(exhausted.next_id, u64::MAX);
    assert!(editor.rename_layer(1, "Renamed".into()));
    assert!(editor.undo());

    let revision = editor.revision();
    assert_eq!(editor.add_label(Label::default()), None);
    assert_eq!(editor.add_landmark(Landmark::default()), None);
    assert_eq!(editor.add_layer(), 1);
    assert!(!editor.commit_stroke(stroke()));
    assert_eq!(editor.revision(), revision);
    assert_eq!(editor.document(), &exhausted);
    assert!(editor.redo());
    assert!(editor.undo());
    assert!(editor.undo());
    assert_eq!(editor.document().next_id, u64::MAX - 1);
    assert!(editor.redo());
    assert_eq!(editor.document(), &exhausted);
}
