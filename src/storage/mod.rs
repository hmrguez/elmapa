//! Document persistence. Implementations must validate reads and preserve an
//! existing destination when validation or writing fails.
mod export;
mod local;
pub use export::export_svg;
pub use local::LocalDocumentStore;

use crate::document::Document;
use std::path::Path;

/// The editor's persistence seam; paths are chosen by the caller.
pub trait DocumentStore: Send + Sync {
    fn load(&self, path: &Path) -> Result<Document, String>;
    fn save(&self, document: &Document, path: &Path) -> Result<(), String>;
}

#[cfg(test)]
mod tests;
