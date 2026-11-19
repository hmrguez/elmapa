use super::{Editor, Layer};
use crate::{
    document::{RegionLayer, limits::MAX_LAYERS},
    scene::territories::{RegionView, Shoreline},
};
use std::sync::Arc;

impl Editor {
    /// An empty layer can be created explicitly. A first claim can also
    /// create its layer in the same undo step.
    pub fn set_regions(
        &mut self,
        target: Option<u64>,
        regions: RegionLayer,
        shore: &Shoreline,
    ) -> Result<(), String> {
        regions.validate()?;
        let parent = regions.parent;
        if RegionView::resolve(&regions, shore).is_none() {
            return Err("Keep the border crossing two edges of the selected land.".into());
        }
        if let Some(id) = target {
            let prior = self
                .document
                .layer(id)
                .filter(|layer| layer.visible && layer.opacity > 0. && layer.territories.is_some())
                .and_then(|layer| layer.territories.as_ref())
                .ok_or("Show the Regions layer before editing it.")?;
            if prior.parent != parent
                || self.document.layers.iter().any(|layer| {
                    layer
                        .territories
                        .as_ref()
                        .and_then(|data| data.parent)
                        .is_some_and(|parent| {
                            parent.layer == id && parent.owner >= regions.names.len()
                        })
                })
            {
                return Err("Keep the parent regions linked to their child layers.".into());
            }
            if **prior == regions {
                return Ok(());
            }
            self.checkpoint();
            self.document
                .layer_mut(id)
                .expect("checked layer")
                .territories = Some(Arc::new(regions));
        } else {
            let child_name = if let Some(parent) = parent {
                let name = self
                    .document
                    .region_name(parent)
                    .ok_or("The parent region is missing.")?;
                if self.document.child_regions(parent).is_some() {
                    return Err("This region already has a child layer.".into());
                }
                Some(format!(
                    "{} regions",
                    name.chars().take(48).collect::<String>()
                ))
            } else {
                None
            };
            if self.document.layers.len() >= MAX_LAYERS {
                return Err("This atlas has reached its layer limit.".into());
            }
            let id = self
                .checkpoint_with_id()
                .ok_or("No free layer identifiers.")?;
            let count = self
                .document
                .layers
                .iter()
                .filter(|layer| layer.territories.is_some())
                .count();
            self.document.layers.push(Layer {
                id,
                name: child_name.unwrap_or_else(|| {
                    if count == 0 {
                        "Regions".into()
                    } else {
                        format!("Regions {id}")
                    }
                }),
                visible: true,
                opacity: 1.,
                strokes: Vec::new().into(),
                labels: Vec::new(),
                landmarks: Vec::new(),
                terrain_replay: None,
                territories: Some(Arc::new(regions)),
            });
            self.active_layer = id;
        }
        Ok(())
    }

    pub fn select_artwork_layer(&mut self) {
        if self
            .document
            .layer(self.active_layer)
            .is_some_and(|layer| layer.territories.is_some())
            && let Some(layer) = self
                .document
                .layers
                .iter()
                .rev()
                .find(|layer| layer.territories.is_none() && layer.visible)
        {
            self.active_layer = layer.id;
        }
    }
}
