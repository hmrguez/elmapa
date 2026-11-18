use super::*;

#[test]
fn invalid_version_duplicate_ids_and_nonfinite_geometry_are_rejected() {
    let mut document = Document::demo();
    document.version = 999;
    assert!(document.validate().unwrap_err().contains("version"));
    document.version = DOCUMENT_VERSION;
    document.layers[1].id = document.layers[0].id;
    assert!(document.validate().is_err());
    document = Document::demo();
    document.layers[0].strokes[0].points[0].x = f32::NAN;
    assert!(document.validate().is_err());
    document = Document::demo();
    document.layers[0].opacity = f32::INFINITY;
    assert!(document.validate().is_err());
}

#[test]
fn content_frame_tracks_visible_paint_outside_the_legacy_page() {
    let mut document = Document::blank();
    let empty_frame = document.content_bounds();
    document.layers[0].strokes.push(Stroke {
        creation_id: None,
        material: None,
        brush: Brush::Land,
        points: vec![Point::new(-4000., 9000.)].into(),
        size: 100.,
        strength: 1.,
        seed: 1,
    });
    let frame = document.content_bounds();
    assert!(frame.min.x < -4000. && frame.max.x > -4000.);
    assert!(frame.min.y < 9000. && frame.max.y > 9000.);
    assert!(frame.width() < 500.);
    let mut eraser = document.layers[0].strokes[0].clone();
    eraser.brush = Brush::Erase;
    eraser.points = vec![Point::new(100000., -100000.)].into();
    document.layers[0].strokes.push(eraser);
    assert_eq!(frame, document.content_bounds());
    document.layers[0].visible = false;
    assert_eq!(empty_frame, document.content_bounds());
    document.layers[0].visible = true;
    document.layers[0].strokes[0].strength = 0.;
    assert_eq!(empty_frame, document.content_bounds());
}

#[test]
fn point_bounds_and_brush_summaries_follow_cow_mutations() {
    let mut document = Document::demo();
    let original = document.clone();
    assert!(document.layers[0].brushes().contains(&Brush::Land));
    let before = document.layers[0].strokes[0].bounds().unwrap();
    document.layers[0].strokes[0]
        .points
        .push_point(Point::new(-9000., 8000.));
    let bounds = document.layers[0].strokes[0].bounds().unwrap();
    assert!(bounds.min.x < before.min.x && bounds.max.y > before.max.y);
    assert_eq!(original.layers[0].strokes[0].bounds().unwrap(), before);
    for index in 0..document.layers[0].strokes.len() {
        document.layers[0].strokes[index].brush = Brush::Path;
    }
    assert_eq!(document.layers[0].brushes(), vec![Brush::Path]);
    assert_eq!(original.layers[0].brushes(), vec![Brush::Land]);
    document.layers[0].strokes.clear();
    assert!(document.layers[0].brushes().is_empty());
}

#[test]
fn shared_point_bounds_are_published_across_snapshots_and_reset_on_mutation() {
    let mut points: super::SharedVec<super::Point> =
        vec![super::Point::new(-20., 10.), super::Point::new(40., 50.)].into();
    let snapshot = points.clone();
    let before = snapshot.point_bounds().unwrap();
    assert_eq!(points.point_bounds(), Some(before));
    points.push_point(super::Point::new(90., -50.));
    assert_eq!(snapshot.point_bounds(), Some(before));
    assert_eq!(points.point_bounds().unwrap().max.x, 90.);
    points[0] = super::Point::new(-200., 0.);
    assert_eq!(points.point_bounds().unwrap().min.x, -200.);
    assert_eq!(snapshot.point_bounds(), Some(before));
}

#[test]
fn legacy_documents_and_label_roundtrips_preserve_safe_editable_typography() {
    let legacy = serde_json::to_string(&Document::demo()).unwrap();
    assert!(!legacy.contains("labels"));
    let mut document: Document = serde_json::from_str(&legacy).unwrap();
    let label = Label {
        id: document.next_id,
        text: "São Tomé <North> & Co.".into(),
        position: Point::new(-3000., -4000.),
        rotation: 35.,
        curvature: -65.,
        ..Label::default()
    };
    let mut legacy_label = serde_json::to_value(&label).unwrap();
    legacy_label.as_object_mut().unwrap().remove("curvature");
    let legacy_label: Label = serde_json::from_value(legacy_label).unwrap();
    assert_eq!(legacy_label.curvature, 0.);
    document.next_id += 1;
    document.layers[0].labels.push(label.clone());
    document.validate().unwrap();
    let reopened: Document =
        serde_json::from_str(&serde_json::to_string(&document).unwrap()).unwrap();
    assert_eq!(document, reopened);
    assert!(document.content_bounds().min.x < -3000.);
    document.layers[0].labels[0].id = document.layers[1].id;
    assert!(document.validate().is_err());
    document.layers[0].labels[0] = Label {
        spacing: f32::INFINITY,
        ..label.clone()
    };
    assert!(document.validate().is_err());
    for curvature in [f32::NAN, -121., 121.] {
        document.layers[0].labels[0] = Label {
            curvature,
            ..label.clone()
        };
        assert!(document.validate().is_err());
    }
    document.layers[0].labels[0] = Label {
        text: "a\nb".into(),
        ..label
    };
    assert!(document.validate().is_err());
}
