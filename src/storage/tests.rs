use super::*;
use std::fs;

#[test]
fn map_roundtrip_preserves_original_seeded_geometry_and_replaces_file() {
    let store: &dyn DocumentStore = &LocalDocumentStore;
    let path = std::env::temp_dir().join(format!(
        "elmapa-document-test-{}.elmapa",
        std::process::id()
    ));
    let document = Document::demo();
    store.save(&document, &path).unwrap();
    assert_eq!(store.load(&path).unwrap(), document);
    let blank = Document::blank();
    store.save(&blank, &path).unwrap();
    assert_eq!(store.load(&path).unwrap(), blank);
    fs::remove_file(path).unwrap();
}
#[test]
fn invalid_save_preserves_existing_document() {
    let store: &dyn DocumentStore = &LocalDocumentStore;
    let path = std::env::temp_dir().join(format!(
        "elmapa-document-invalid-save-test-{}.elmapa",
        std::process::id()
    ));
    let document = Document::demo();
    store.save(&document, &path).unwrap();
    let mut invalid = Document::blank();
    invalid.layers.clear();
    assert!(store.save(&invalid, &path).is_err());
    assert_eq!(store.load(&path).unwrap(), document);
    fs::remove_file(path).unwrap();
}

#[test]
fn load_rejects_corrupt_unsupported_and_oversized_documents() {
    let store: &dyn DocumentStore = &LocalDocumentStore;
    let path = std::env::temp_dir().join(format!(
        "elmapa-invalid-input-{}.elmapa",
        std::process::id()
    ));
    fs::write(&path, b"not a document").unwrap();
    assert!(store.load(&path).unwrap_err().contains("not a valid"));
    let mut invalid = Document::blank();
    invalid.version = 999;
    fs::write(&path, serde_json::to_vec(&invalid).unwrap()).unwrap();
    assert!(store.load(&path).unwrap_err().contains("version"));
    let file = fs::File::create(&path).unwrap();
    file.set_len(32 * 1024 * 1024 + 1).unwrap();
    drop(file);
    assert!(store.load(&path).unwrap_err().contains("32 MB"));
    fs::remove_file(path).unwrap();
}

#[test]
fn png_frames_visible_artwork_at_a_bounded_resolution() {
    use crate::document::{Brush, Point, Stroke};

    let path = std::env::temp_dir().join(format!("elmapa-png-test-{}.png", std::process::id()));
    let mut document = Document::blank();
    let trail = Stroke {
        creation_id: None,
        material: None,
        brush: Brush::Path,
        points: vec![Point::new(-300., 40.), Point::new(100., 40.)].into(),
        size: 24.,
        strength: 1.,
        seed: 42,
    };
    document.layers[0].strokes.push(trail);
    let bounds = document.content_bounds();
    let mut hidden = document.layers[0].clone();
    hidden.visible = false;
    hidden.id = 2;
    hidden.strokes.push(Stroke {
        points: vec![Point::new(4150., -2000.)].into(),
        ..hidden.strokes[0].clone()
    });
    document.layers.push(hidden);
    document.next_id = 3;
    export_png(&path, &document, None).unwrap();
    let png = image::open(&path).unwrap().to_rgba8();
    fs::remove_file(path).unwrap();

    assert_eq!(png.width(), 4096);
    assert_eq!(
        png.height(),
        (4096. * bounds.height() / bounds.width()).ceil() as u32
    );
    assert!(png.pixels().all(|pixel| pixel[3] == 255));
    assert!(png.pixels().any(|pixel| pixel[0] < 160));
}
