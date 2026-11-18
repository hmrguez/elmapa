use super::Editor;
use crate::document::{
    Label, Landmark, Layer, Point,
    limits::{MAX_LABELS, MAX_LANDMARKS},
};

pub(super) trait Annotation: PartialEq + Sized {
    const LIMIT: usize;
    const EDITABLE_AT_ZERO_OPACITY: bool;

    fn items(layer: &Layer) -> &[Self];
    fn items_mut(layer: &mut Layer) -> &mut Vec<Self>;
    fn id(&self) -> u64;
    fn set_id(&mut self, id: u64);
    fn validate(&self) -> Result<(), String>;
    fn hit(&self, point: Point, tolerance: f32) -> bool;

    fn editable_on(layer: &Layer) -> bool {
        layer.territories.is_none()
            && layer.visible
            && (Self::EDITABLE_AT_ZERO_OPACITY || layer.opacity > 0.)
    }
}

impl Annotation for Label {
    const LIMIT: usize = MAX_LABELS;
    const EDITABLE_AT_ZERO_OPACITY: bool = true;

    fn items(layer: &Layer) -> &[Self] {
        &layer.labels
    }
    fn items_mut(layer: &mut Layer) -> &mut Vec<Self> {
        &mut layer.labels
    }
    fn id(&self) -> u64 {
        self.id
    }
    fn set_id(&mut self, id: u64) {
        self.id = id;
    }
    fn validate(&self) -> Result<(), String> {
        Label::validate(self)
    }
    fn hit(&self, point: Point, tolerance: f32) -> bool {
        self.opacity > 0. && self.contains(point, tolerance)
    }
}

impl Annotation for Landmark {
    const LIMIT: usize = MAX_LANDMARKS;
    const EDITABLE_AT_ZERO_OPACITY: bool = false;

    fn items(layer: &Layer) -> &[Self] {
        &layer.landmarks
    }
    fn items_mut(layer: &mut Layer) -> &mut Vec<Self> {
        &mut layer.landmarks
    }
    fn id(&self) -> u64 {
        self.id
    }
    fn set_id(&mut self, id: u64) {
        self.id = id;
    }
    fn validate(&self) -> Result<(), String> {
        Landmark::validate(self)
    }
    fn hit(&self, point: Point, tolerance: f32) -> bool {
        self.contains(point, tolerance)
    }
}

impl Editor {
    pub(super) fn annotation<A: Annotation>(&self, id: u64) -> Option<&A> {
        let (layer, index) = self.annotation_location::<A>(id)?;
        Some(&A::items(&self.document.layers[layer])[index])
    }

    pub(super) fn annotation_at<A: Annotation>(
        &self,
        point: Point,
        tolerance: f32,
    ) -> Option<(u64, u64)> {
        self.document
            .layers
            .iter()
            .rev()
            .filter(|layer| layer.visible && layer.opacity > 0.)
            .find_map(|layer| {
                A::items(layer)
                    .iter()
                    .rev()
                    .find(|item| item.hit(point, tolerance))
                    .map(|item| (layer.id, item.id()))
            })
    }

    pub(super) fn add_annotation<A: Annotation>(&mut self, item: A) -> Option<u64> {
        self.add_annotation_on_layer(item, self.active_layer)
    }

    pub(super) fn add_annotation_on_layer<A: Annotation>(
        &mut self,
        mut item: A,
        layer_id: u64,
    ) -> Option<u64> {
        if item.validate().is_err()
            || self
                .document
                .layers
                .iter()
                .map(|layer| A::items(layer).len())
                .sum::<usize>()
                >= A::LIMIT
        {
            return None;
        }
        let layer = self
            .document
            .layers
            .iter()
            .position(|layer| layer.id == layer_id && A::editable_on(layer))?;
        let id = self.checkpoint_with_id()?;
        item.set_id(id);
        A::items_mut(&mut self.document.layers[layer]).push(item);
        Some(id)
    }

    pub(super) fn update_annotation<A: Annotation>(&mut self, item: A) -> bool {
        let Some((layer, index)) = self.annotation_location::<A>(item.id()) else {
            return false;
        };
        let target = &self.document.layers[layer];
        if !A::editable_on(target) || item.validate().is_err() || A::items(target)[index] == item {
            return false;
        }
        self.checkpoint();
        A::items_mut(&mut self.document.layers[layer])[index] = item;
        true
    }

    pub(super) fn delete_annotation<A: Annotation>(&mut self, id: u64) -> bool {
        let Some((layer, index)) = self.annotation_location::<A>(id) else {
            return false;
        };
        if !A::editable_on(&self.document.layers[layer]) {
            return false;
        }
        self.checkpoint();
        A::items_mut(&mut self.document.layers[layer]).remove(index);
        true
    }

    fn annotation_location<A: Annotation>(&self, id: u64) -> Option<(usize, usize)> {
        self.document
            .layers
            .iter()
            .enumerate()
            .find_map(|(layer, data)| {
                A::items(data)
                    .iter()
                    .position(|item| item.id() == id)
                    .map(|index| (layer, index))
            })
    }
}
