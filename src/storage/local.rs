use super::DocumentStore;
use crate::document::Document;
use std::{fs, io::Write, path::Path};

const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;

/// Versioned JSON files, bounded reads, and atomic replacement on save.
pub struct LocalDocumentStore;

impl DocumentStore for LocalDocumentStore {
    fn save(&self, document: &Document, path: &Path) -> Result<(), String> {
        let _span = crate::performance::Span::new("document_save");
        document.validate()?;
        let bytes = serde_json::to_vec(document)
            .map_err(|error| format!("Could not encode map: {error}"))?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err("Map exceeds the 32 MB file limit.".into());
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let temporary = parent.join(format!(".elmapa-{}-{unique}.tmp", std::process::id()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("Could not create temporary map file: {error}"))?;
        let result = (|| -> std::io::Result<()> {
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, path)?;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(format!("Could not save map: {error}"));
        }
        Ok(())
    }
    fn load(&self, path: &Path) -> Result<Document, String> {
        let _span = crate::performance::Span::new("document_load");
        let file = fs::File::open(path).map_err(|error| format!("Could not open map: {error}"))?;
        // Cap the actual read as well as metadata, including files that grow during loading.
        use std::io::Read;
        let mut bytes = Vec::new();
        file.take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("Could not read map: {error}"))?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err("Map exceeds the 32 MB file limit.".into());
        }
        let document: Document = serde_json::from_slice(&bytes)
            .map_err(|error| format!("This is not a valid Elmapa document: {error}"))?;
        document.validate()?;
        Ok(document)
    }
}
