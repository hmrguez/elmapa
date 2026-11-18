use super::{Editor, Layer};
use crate::document::limits::{MAX_LAYERS, valid_name};

impl Editor {
    pub fn add_layer(&mut self) -> u64 {
        if self.document.layers.len() >= MAX_LAYERS {
            return self.active_layer;
        }
        let Some(id) = self.checkpoint_with_id() else {
            return self.active_layer;
        };
        self.document.layers.push(Layer {
            id,
            name: format!("Layer {id}"),
            visible: true,
            opacity: 1.0,
            strokes: Vec::new().into(),
            labels: Vec::new(),
            landmarks: Vec::new(),
            terrain_replay: None,
            territories: None,
        });
        self.active_layer = id;
        id
    }
    pub fn remove_layer(&mut self, id: u64) -> bool {
        let Some(index) = self.document.layers.iter().position(|layer| layer.id == id) else {
            return false;
        };
        if self.document.layers.len() == 1 {
            return false;
        }
        self.checkpoint();
        let removed = self.document.layers.remove(index);
        self.stroke_count -= removed.strokes.len();
        self.point_count -= removed
            .strokes
            .iter()
            .map(|s| s.points.len())
            .sum::<usize>();
        if self.active_layer == id {
            self.active_layer = self.document.layers[index.min(self.document.layers.len() - 1)].id;
        }
        true
    }
    pub fn toggle_layer(&mut self, id: u64) -> bool {
        if self.document.layer(id).is_none() {
            return false;
        }
        self.checkpoint();
        let layer = self.document.layer_mut(id).expect("checked layer");
        layer.visible = !layer.visible;
        true
    }
    pub fn set_layer_opacity(&mut self, id: u64, opacity: f32) -> bool {
        if !opacity.is_finite() {
            return false;
        }
        let opacity = opacity.clamp(0.0, 1.0);
        if self
            .document
            .layer(id)
            .is_none_or(|layer| layer.opacity == opacity)
        {
            return false;
        }
        self.checkpoint();
        self.document.layer_mut(id).expect("checked layer").opacity = opacity;
        true
    }
    /// Move by an offset in bottom-to-top order; out-of-range requests are no-ops.
    pub fn move_layer(&mut self, id: u64, delta: isize) -> bool {
        let Some(index) = self.document.layers.iter().position(|layer| layer.id == id) else {
            return false;
        };
        let Some(target) = index.checked_add_signed(delta) else {
            return false;
        };
        if target >= self.document.layers.len() || target == index {
            return false;
        }
        self.checkpoint();
        let layer = self.document.layers.remove(index);
        self.document.layers.insert(target, layer);
        true
    }
    pub fn rename_layer(&mut self, id: u64, name: String) -> bool {
        if !valid_name(&name)
            || self
                .document
                .layer(id)
                .is_none_or(|layer| layer.name == name)
        {
            return false;
        }
        self.checkpoint();
        self.document.layer_mut(id).expect("checked layer").name = name;
        true
    }
}
