//! Place one name per owner on its largest visible piece of land.
use super::{RegionView, Shoreline, interior, split::area, visible_face};
use crate::{
    document::{Label, RegionLayer, RegionPolygon},
    scene::generation,
};
use std::collections::BTreeMap;

pub(super) fn place(view: &RegionView, shore: &Shoreline, data: &RegionLayer) -> Vec<Label> {
    if !data.styles.values().any(|style| style.label == Some(true)) {
        return Vec::new();
    }
    let mut largest: BTreeMap<usize, (f64, RegionPolygon)> = BTreeMap::new();
    for face in &view.faces {
        if generation::stopped() {
            return Vec::new();
        }
        if data.styles.get(&face.owner).and_then(|style| style.label) != Some(true) {
            continue;
        }
        for piece in visible_face(&face.polygon, shore) {
            let size = area(&piece);
            let candidate = largest.entry(face.owner).or_default();
            if size > candidate.0 {
                *candidate = (size, piece);
            }
        }
    }
    largest
        .into_iter()
        .filter_map(|(owner, (_, polygon))| fit(&data.names[owner], &polygon))
        .collect()
}

fn fit(name: &str, polygon: &RegionPolygon) -> Option<Label> {
    let mut label = Label {
        text: name.into(),
        size: 42.,
        ..Label::default()
    };
    let ink = label.bounds();
    let aspect = f64::from((ink.max.x - ink.min.x).max(1.) / (ink.max.y - ink.min.y).max(1.));
    let (clearance, position) = interior(polygon, aspect)?;
    label.position = position;
    let usable_height = 2. * clearance;
    label.size *= ((0.82 * usable_height as f32) / (ink.max.y - ink.min.y).max(1.)).min(1.);
    // Small islands still get their name when zoomed in.
    label.size = label.size.max(crate::document::limits::MIN_LABEL_SIZE);
    Some(label)
}
