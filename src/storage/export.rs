use crate::{document::Document, scene};

/// Write a self-contained SVG atomically, with optional cancellation.
pub fn export_svg(
    path: &std::path::Path,
    document: &Document,
    cancellation: Option<(std::sync::Arc<std::sync::atomic::AtomicU64>, u64)>,
) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".elmapa-export-{}-{sequence}.tmp",
        std::process::id()
    ));
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = (|| -> Result<(), String> {
        let mut writer = std::io::BufWriter::with_capacity(128 * 1024, &file);
        scene::generation::bounded(scene::generation::Limits::EXPORT, cancellation, || {
            scene::write_svg(document, &mut writer)
        })
        .map_err(|e| e.to_string())??;
        drop(writer);
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}
