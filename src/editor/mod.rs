//! Undoable document operations, independent of persistence and presentation.
mod annotations;
mod history;
mod labels;
mod layers;
mod strokes;
mod territories;

use crate::document::{AtlasStyle, Document, Layer, Stroke};
use history::Snapshot;

pub struct Editor {
    document: Document,
    active_layer: u64,
    revision: u64,
    point_count: usize,
    stroke_count: usize,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl Default for Editor {
    fn default() -> Self {
        Self::demo()
    }
}

impl Editor {
    pub fn new(document: Document) -> Result<Self, String> {
        document.validate()?;
        let active_layer = document.layers.first().map(|layer| layer.id).unwrap_or(1);
        let point_count = document
            .layers
            .iter()
            .flat_map(|l| l.strokes.iter())
            .map(|s| s.points.len())
            .sum();
        let stroke_count = document.layers.iter().map(|l| l.strokes.len()).sum();
        Ok(Self {
            document,
            active_layer,
            revision: 0,
            point_count,
            stroke_count,
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }
    pub fn demo() -> Self {
        Self::new(Document::demo()).expect("valid example document")
    }
    pub fn blank() -> Self {
        Self::new(Document::blank()).expect("valid blank document")
    }
    /// Replace the current world as one undoable operation, retaining earlier edits.
    /// Rejected or identical documents leave both the document and history intact.
    pub fn replace_document(&mut self, document: Document) -> bool {
        if document.validate().is_err() || document == self.document {
            return false;
        }
        self.checkpoint();
        self.active_layer = document.layers[0].id;
        self.document = document;
        self.recount();
        true
    }
    pub fn set_atlas_style(&mut self, style: AtlasStyle) {
        if self.document.style != style {
            self.checkpoint();
            self.document.style = style;
        }
    }

    /// Read-only access keeps all mutations inside undoable editor operations.
    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn point_count(&self) -> usize {
        self.point_count
    }
    pub fn stroke_count(&self) -> usize {
        self.stroke_count
    }
    fn recount(&mut self) {
        self.point_count = self
            .document
            .layers
            .iter()
            .flat_map(|l| l.strokes.iter())
            .map(|s| s.points.len())
            .sum();
        self.stroke_count = self.document.layers.iter().map(|l| l.strokes.len()).sum();
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn active_layer(&self) -> u64 {
        self.active_layer
    }

    /// Selection is session state and does not create an undo checkpoint.
    pub fn select_layer(&mut self, id: u64) -> bool {
        if self.document.layer(id).is_none() {
            return false;
        }
        self.active_layer = id;
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests;

mod landmarks;
