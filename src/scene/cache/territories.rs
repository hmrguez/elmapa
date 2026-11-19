//! Retain region overlays until their authored geometry or the shoreline changes.
use super::{
    REGIONS,
    material::{MaterialLayer, TerrainIdentity},
    snapshot::Batch,
};
use crate::{
    document::{Layer, RegionLayer},
    scene::territories::{RegionView, Shoreline},
};
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub(super) struct Territories {
    terrain: Vec<TerrainIdentity>,
    pub(super) shore: Arc<Shoreline>,
    layers: HashMap<u64, Rendered>,
}

struct Rendered {
    source: Arc<RegionLayer>,
    ancestors: Vec<Arc<RegionLayer>>,
    view: Arc<RegionView>,
    batch: Batch,
}

impl Territories {
    pub(super) fn update_shore<'a>(&mut self, layers: impl Iterator<Item = &'a MaterialLayer>) {
        let layers = layers
            .filter(|layer| {
                layer.source.visible
                    && layer.source.opacity > 0.
                    && layer.source.territories.is_none()
            })
            .map(|layer| &layer.terrain)
            .collect::<Vec<_>>();
        if self.terrain.len() == layers.len()
            && self
                .terrain
                .iter()
                .zip(&layers)
                .all(|(old, new)| old.matches(new))
        {
            return;
        }
        let mut shore = Shoreline::default();
        shore.surfaces = layers
            .iter()
            .map(|tiles| {
                REGIONS
                    .iter()
                    .enumerate()
                    .flat_map(|(index, &brush)| {
                        tiles
                            .regions(index)
                            .filter(|(_, region)| !region.surface.is_empty())
                            .map(move |(_, region)| (brush, region.surface.clone()))
                    })
                    .collect()
            })
            .collect();
        self.shore = Arc::new(shore);
        self.terrain = layers.into_iter().map(|tiles| tiles.identity()).collect();
        self.layers.clear();
    }

    pub(super) fn render<'a>(
        &mut self,
        layer: &Layer,
        lookup: impl Fn(u64) -> Option<&'a Arc<RegionLayer>>,
    ) -> Option<(Batch, Arc<RegionView>)> {
        let source = layer.territories.as_ref()?;
        let ancestors = source.ancestors(lookup)?;
        if let Some(previous) = self.layers.get(&layer.id)
            && Arc::ptr_eq(&previous.source, source)
            && previous.ancestors.len() == ancestors.len()
            && previous
                .ancestors
                .iter()
                .zip(&ancestors)
                .all(|(prior, (_, current))| Arc::ptr_eq(prior, current))
        {
            return Some((previous.batch.clone(), previous.view.clone()));
        }
        let shore = Shoreline::scoped(&self.shore, &ancestors)?;
        let view = Arc::new(RegionView::resolve(source, &shore)?);
        let batch = Arc::new(view.shapes(&shore, source));
        if crate::scene::generation::stopped() {
            return None;
        }
        self.layers.insert(
            layer.id,
            Rendered {
                source: source.clone(),
                ancestors: ancestors
                    .into_iter()
                    .map(|(_, data)| data.clone())
                    .collect(),
                view: view.clone(),
                batch: batch.clone(),
            },
        );
        Some((batch, view))
    }

    pub(super) fn retain(&mut self, scene: &super::Scene) {
        self.layers
            .retain(|id, _| scene.territory_views.contains_key(id));
    }
}
