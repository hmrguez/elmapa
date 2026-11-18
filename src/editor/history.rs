use super::{Document, Editor};
const MAX_HISTORY: usize = 64;
const MAX_HISTORY_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone)]
pub(super) struct Snapshot {
    pub(super) document: Document,
    pub(super) active_layer: u64,
}

impl Editor {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            document: self.document.clone(),
            active_layer: self.active_layer,
        }
    }
    pub(super) fn checkpoint(&mut self) {
        self.record_checkpoint(self.snapshot());
    }

    pub(super) fn checkpoint_with_id(&mut self) -> Option<u64> {
        let snapshot = self.snapshot();
        let id = self.document.allocate_id().ok()?;
        self.record_checkpoint(snapshot);
        Some(id)
    }

    fn record_checkpoint(&mut self, snapshot: Snapshot) {
        if self.undo.len() == MAX_HISTORY {
            self.undo.remove(0);
        }
        self.undo.push(snapshot);
        self.redo.clear();
        self.revision += 1;
        while self.undo.len() > 1 && self.history_bytes() > MAX_HISTORY_BYTES {
            self.undo.remove(0);
        }
    }
    /// Unique retained COW payloads, including the current document and redo.
    pub fn history_bytes(&self) -> usize {
        let mut seen = std::collections::HashSet::new();
        let mut bytes = 0;
        for document in std::iter::once(&self.document)
            .chain(self.undo.iter().chain(&self.redo).map(|s| &s.document))
        {
            bytes += document.name.capacity();
            if !seen.insert(document.layers.storage_id()) {
                continue;
            }
            bytes += document.layers.capacity() * std::mem::size_of::<crate::document::Layer>();
            for layer in &document.layers {
                bytes += layer.name.capacity();
                bytes += layer.labels.capacity() * std::mem::size_of::<crate::document::Label>();
                bytes += layer
                    .labels
                    .iter()
                    .map(|l| l.text.capacity())
                    .sum::<usize>();
                if let Some(regions) = &layer.territories
                    && seen.insert(std::sync::Arc::as_ptr(regions) as usize)
                {
                    bytes += regions.bytes();
                }
                if !seen.insert(layer.strokes.storage_id()) {
                    continue;
                }
                bytes += layer.strokes.head_bytes();
                for (id, payload, strokes) in layer.strokes.chunks() {
                    if !seen.insert(id) {
                        continue;
                    }
                    bytes += payload;
                    for stroke in strokes {
                        if seen.insert(stroke.points.storage_id()) {
                            bytes += stroke.points.capacity()
                                * std::mem::size_of::<crate::document::Point>();
                        }
                    }
                }
            }
        }
        bytes
    }
    fn restore(&mut self, snapshot: Snapshot) {
        self.revision += 1;
        self.document = snapshot.document;
        self.recount();
        self.active_layer = snapshot.active_layer;
    }
    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.restore(snapshot);
        true
    }
    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snapshot());
        self.restore(snapshot);
        true
    }
}
