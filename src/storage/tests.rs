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
