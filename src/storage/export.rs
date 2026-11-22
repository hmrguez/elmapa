use crate::{document::Document, scene};
use std::{
    io::{BufWriter, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

type Cancellation = Option<(Arc<AtomicU64>, u64)>;

/// Write a self-contained SVG atomically, with optional cancellation.
pub fn export_svg(
    path: &Path,
    document: &Document,
    cancellation: Cancellation,
) -> Result<(), String> {
    write_export(path, cancellation, |writer| {
        scene::write_svg(document, writer)
    })
}

/// Write a PNG of the visible map artwork atomically, with optional cancellation.
pub fn export_png(
    path: &Path,
    document: &Document,
    cancellation: Cancellation,
) -> Result<(), String> {
    write_export(path, cancellation, |writer| {
        scene::write_png(document, writer)
    })
}

fn write_export(
    path: &Path,
    cancellation: Cancellation,
    write: impl FnOnce(&mut BufWriter<&std::fs::File>) -> Result<(), String>,
) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
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
        scene::generation::bounded(scene::generation::Limits::EXPORT, cancellation, || {
            if !scene::generation::is_stopped() {
                let mut writer = BufWriter::with_capacity(128 * 1024, &file);
                write(&mut writer)?;
                writer.flush().map_err(|e| e.to_string())?;
                file.sync_all().map_err(|e| e.to_string())?;
                scene::generation::is_stopped();
            }
            Ok::<(), String>(())
        })
        .map_err(|e| e.to_string())??;
        drop(file);
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}
