//! Compose retained material and annotations into one ordered scene.
use super::{
    junctions::Junctions,
    material::MaterialLayer,
    snapshot::{Batch, RegionSource, Scene, TerritoryView},
    territories::Territories,
};
use crate::{
    document::{AtlasStyle, Document, Label, Landmark, Layer},
    scene::{
        ForestTree, LandmarkBatch, LandmarkPart, MountainPart, PathPart, RiverReach,
        TerrainPalette, generation, label_shapes, landmark_batches_in, territories::Shoreline,
    },
};
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub(super) struct AnnotationCache {
    labels: Vec<(Label, Batch)>,
    landmarks: Vec<(Landmark, Vec<LandmarkBatch>)>,
}

pub(super) enum AnnotationSource<'a> {
    Committed,
    Current {
        document: &'a Document,
        updated: &'a mut AnnotationCache,
    },
}

impl<'a> AnnotationSource<'a> {
    fn layer(&self, source: &'a Layer) -> Option<&'a Layer> {
        match self {
            Self::Committed => Some(source),
            Self::Current { document, .. } => document.layer(source.id),
        }
    }

    fn admits(&self, batch: &Batch) -> bool {
        matches!(self, Self::Committed) || generation::vertices(batch.len().saturating_mul(3))
    }
}

#[derive(Default)]
struct Composition {
    batches: Vec<Batch>,
    trees: HashMap<usize, ForestTree>,
    mountains: HashMap<usize, MountainPart>,
    rivers: HashMap<usize, Arc<RiverReach>>,
    paths: HashMap<usize, PathPart>,
    opacity: HashMap<usize, f32>,
    labels: HashMap<usize, Label>,
    landmarks: HashMap<usize, LandmarkPart>,
    regions: HashMap<usize, RegionSource>,
    terrain_palettes: HashMap<usize, Arc<TerrainPalette>>,
    territory_views: HashMap<u64, TerritoryView>,
}

impl Composition {
    fn push(&mut self, batch: Batch, opacity: f32) {
        self.opacity.insert(Arc::as_ptr(&batch) as usize, opacity);
        self.batches.push(batch);
    }

    fn terrain(&mut self, material: &MaterialLayer) {
        let opacity = material.source.opacity;
        material.mountain_parts(&mut self.mountains);
        self.rivers.extend(
            material
                .rivers
                .iter()
                .map(|(key, reach)| (*key, reach.clone())),
        );
        material.path_parts(&mut self.paths);
        self.regions.extend(material.region_sources());
        self.terrain_palettes.extend(material.terrain_palettes());

        for batch in material.forest.iter() {
            let mut tree = batch.tree.clone();
            tree.opacity *= opacity;
            self.trees.insert(Arc::as_ptr(&batch.shapes) as usize, tree);
        }
        for batch in material.batches() {
            self.push(batch, opacity);
        }
    }

    fn annotations(
        &mut self,
        layer: &Layer,
        cache: &AnnotationCache,
        source: &mut AnnotationSource<'_>,
        style: AtlasStyle,
    ) {
        for landmark in &layer.landmarks {
            let parts = cache
                .landmarks
                .iter()
                .find(|(old, _)| {
                    old.id == landmark.id
                        && old.kind == landmark.kind
                        && old.position == landmark.position
                        && old.size == landmark.size
                })
                .map(|(_, parts)| parts.clone())
                .unwrap_or_else(|| landmark_batches_in(landmark, style));
            for batch in &parts {
                if !source.admits(&batch.shapes) {
                    break;
                }
                self.landmarks
                    .insert(Arc::as_ptr(&batch.shapes) as usize, batch.part.clone());
                self.push(batch.shapes.clone(), layer.opacity);
            }
            if let AnnotationSource::Current { updated, .. } = source {
                updated.landmarks.push((landmark.clone(), parts));
            }
        }

        for label in layer
            .landmarks
            .iter()
            .filter_map(Landmark::name_label)
            .chain(layer.labels.iter().cloned())
        {
            let batch = cache
                .labels
                .iter()
                .find(|(old, _)| old == &label)
                .map(|(_, batch)| batch.clone())
                .unwrap_or_else(|| {
                    let mut glyphs = label_shapes(&label);
                    if !style.is_classic() {
                        super::super::palette::recolor(
                            Arc::make_mut(&mut glyphs).as_mut_slice(),
                            |_| label.ink.color_in(style),
                        );
                    }
                    glyphs
                });
            if !source.admits(&batch) {
                break;
            }
            self.labels
                .insert(Arc::as_ptr(&batch) as usize, label.clone());
            if let AnnotationSource::Current { updated, .. } = source {
                updated.labels.push((label, batch.clone()));
            }
            self.push(batch, layer.opacity);
        }
    }

    fn finish(self, style: AtlasStyle, shoreline: Arc<Shoreline>) -> Scene {
        Scene {
            style,
            batches: Arc::new(self.batches),
            trees: Arc::new(self.trees),
            mountains: Arc::new(self.mountains),
            rivers: Arc::new(self.rivers),
            paths: Arc::new(self.paths),
            opacity: Arc::new(self.opacity),
            expanded: Arc::default(),
            labels: Arc::new(self.labels),
            landmarks: Arc::new(self.landmarks),
            regions: Arc::new(self.regions),
            terrain_palettes: Arc::new(self.terrain_palettes),
            shoreline,
            territory_views: Arc::new(self.territory_views),
        }
    }
}

pub(super) fn compose<'a>(
    style: AtlasStyle,
    materials: impl Iterator<Item = &'a MaterialLayer> + Clone,
    cache: &AnnotationCache,
    mut source: AnnotationSource<'_>,
    junctions: &mut Junctions,
    territories: &mut Territories,
) -> Scene {
    let mut composition = Composition::default();
    territories.update_shore(materials.clone());
    for material in materials
        .clone()
        .filter(|base| base.source.visible && base.source.opacity > 0.)
    {
        composition.terrain(material);
        if let Some(layer) = source.layer(&material.source) {
            if let Some((batch, view)) = territories.render(layer, style, |id| {
                let material = materials
                    .clone()
                    .find(|material| material.source.id == id)?;
                source.layer(&material.source)?.territories.as_ref()
            }) {
                composition.push(batch, layer.opacity);
                composition.territory_views.insert(
                    layer.id,
                    (
                        layer.territories.clone().expect("rendered region layer"),
                        view,
                    ),
                );
            }
            composition.annotations(layer, cache, &mut source, style);
        }
    }
    let mut scene = composition.finish(style, territories.shore.clone());
    territories.retain(&scene);
    junctions.apply(&mut scene, materials);
    scene.order_paths_between_rivers_and_relief();
    scene
}
