use super::{
    DOCUMENT_VERSION, Document, Stroke,
    limits::{
        MAX_LABELS, MAX_LANDMARKS, MAX_LAYERS, MAX_NAME_BYTES, MAX_STROKE_POINTS, MAX_STROKES,
        valid_name,
    },
};
use std::collections::HashSet;

impl Document {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != DOCUMENT_VERSION {
            return Err(format!(
                "Unsupported map version {} (expected {}).",
                self.version, DOCUMENT_VERSION
            ));
        }
        if !(64.0..=16384.0).contains(&self.width) || !(64.0..=16384.0).contains(&self.height) {
            return Err("Map dimensions must be between 64 and 16384 pixels.".into());
        }
        if !valid_name(&self.name) {
            return Err(format!(
                "A map needs a name of at most {MAX_NAME_BYTES} bytes."
            ));
        }
        if self.layers.is_empty() || self.layers.len() > MAX_LAYERS {
            return Err(format!(
                "A map must contain between 1 and {MAX_LAYERS} layers."
            ));
        }
        let mut ids = HashSet::new();
        let mut point_count = 0usize;
        let mut stroke_count = 0usize;
        let mut label_count = 0usize;
        let mut landmark_count = 0usize;
        let mut region_parents = HashSet::new();
        for layer in &self.layers {
            if layer.id == 0 || !ids.insert(layer.id) || layer.id >= self.next_id {
                return Err("Map layer identifiers are invalid or duplicated.".into());
            }
            if !valid_name(&layer.name) || !(0.0..=1.0).contains(&layer.opacity) {
                return Err("A layer has an invalid name or opacity.".into());
            }
            if let Some(super::TerrainReplay::BrushPaths { radius, paths }) = &layer.terrain_replay
                && (!(1. ..=1024.).contains(radius)
                    || paths.is_empty()
                    || paths.len() > super::limits::MAX_REPLAY_PATHS
                    || paths.iter().any(|path| {
                        path.is_empty()
                            || path.len() > super::limits::MAX_REPLAY_PATH_POINTS
                            || path.iter().any(|point| !point.is_in_world())
                    }))
            {
                return Err("A layer has invalid terrain replay brush paths.".into());
            }
            if let Some(regions) = &layer.territories {
                regions.validate()?;
                if let Some(parent) = regions.parent
                    && (!region_parents.insert(parent)
                        || regions.ancestors(|id| self.region_data(id)).is_none())
                {
                    return Err("A child Regions layer has an invalid or duplicate parent.".into());
                }
                if !layer.strokes.is_empty()
                    || !layer.labels.is_empty()
                    || !layer.landmarks.is_empty()
                {
                    return Err("A Regions layer can only contain regions.".into());
                }
            }
            landmark_count += layer.landmarks.len();
            for landmark in &layer.landmarks {
                landmark.validate()?;
                if landmark.id == 0 || landmark.id >= self.next_id || !ids.insert(landmark.id) {
                    return Err("Map landmark identifiers are invalid or duplicated.".into());
                }
            }
            label_count += layer.labels.len();
            for label in &layer.labels {
                label.validate()?;
                if label.id == 0 || label.id >= self.next_id || !ids.insert(label.id) {
                    return Err("Map label identifiers are invalid or duplicated.".into());
                }
            }
            if layer.relief_moves.len() > MAX_STROKES
                || layer
                    .relief_moves
                    .windows(2)
                    .any(|pair| pair[0].key >= pair[1].key)
                || layer.relief_moves.iter().any(|item| {
                    let brush = match item.key.specimen {
                        super::Specimen::Tree { .. } => super::Brush::Forest,
                        super::Specimen::Peak(_) => super::Brush::Mountain,
                    };
                    !item.position.is_in_world()
                        || item.after_strokes <= item.key.stroke
                        || item.after_strokes > layer.strokes.len()
                        || layer
                            .strokes
                            .get(item.key.stroke)
                            .is_none_or(|stroke| stroke.brush != brush)
                })
            {
                return Err("A layer has invalid relief placements.".into());
            }
            stroke_count += layer.strokes.len();
            for stroke in &layer.strokes {
                stroke.validate()?;
                if stroke
                    .creation_id
                    .is_some_and(|id| id == 0 || id >= self.next_id || !ids.insert(id))
                {
                    return Err("Map stroke identifiers are invalid or duplicated.".into());
                }
                point_count += stroke.points.len();
            }
        }
        if point_count > MAX_STROKE_POINTS
            || stroke_count > MAX_STROKES
            || label_count > MAX_LABELS
            || landmark_count > MAX_LANDMARKS
        {
            return Err("This map exceeds the current editor's geometry limit.".into());
        }
        Ok(())
    }
}

impl Stroke {
    pub fn validate(&self) -> Result<(), String> {
        if self.points.is_empty()
            || !(1.0..=2048.0).contains(&self.size)
            || !(0.0..=1.0).contains(&self.strength)
        {
            return Err("A brush stroke has invalid settings or no points.".into());
        }
        if self
            .material
            .as_deref()
            .is_some_and(|id| !crate::brush_materials::valid_id(id))
        {
            return Err("A brush stroke has an invalid material identifier.".into());
        }
        if self.points.iter().any(|p| !p.is_in_world()) {
            return Err("A brush stroke has invalid coordinates.".into());
        }
        Ok(())
    }
}
