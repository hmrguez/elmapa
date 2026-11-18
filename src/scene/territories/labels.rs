//! Place one name per owner on its largest visible piece of land.
use super::{RegionView, Shoreline, bounds, split::area, visible_face};
use crate::{
    document::{Label, Point, RegionLayer, RegionPolygon},
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
    let extent = bounds(polygon.iter().flatten())?;
    let mut label = Label {
        text: name.into(),
        size: 42.,
        ..Label::default()
    };
    let ink = label.bounds();
    let aspect = f64::from((ink.max.x - ink.min.x).max(1.) / (ink.max.y - ink.min.y).max(1.));
    let edges = || {
        polygon
            .iter()
            .flat_map(|ring| ring.iter().zip(ring.iter().cycle().skip(1)))
    };

    // Try the middle first. Even-odd spans avoid both water and enclosed lakes.
    let height = f64::from(extent.max.y - extent.min.y);
    let mut best = None;
    for step in [0, -1, 1, -2, 2, -3, 3, -4, 4, -5, 5, -6, 6, -7, 7] {
        let y =
            f64::from(f32::midpoint(extent.min.y, extent.max.y)) + height * f64::from(step) / 16.;
        let mut crossings: Vec<_> = edges()
            .filter(|&(&[_, ay], &[_, by])| (ay > y) != (by > y))
            .map(|(&[ax, ay], &[bx, by])| ax + (y - ay) * (bx - ax) / (by - ay))
            .collect();
        crossings.sort_unstable_by(f64::total_cmp);
        let Some(span) = crossings
            .chunks_exact(2)
            .max_by(|a, b| (a[1] - a[0]).total_cmp(&(b[1] - b[0])))
        else {
            continue;
        };
        let x = f64::midpoint(span[0], span[1]);
        // Scale the distance test to the lettering's aspect ratio so a wide
        // name leaves room for its ends as well as its center.
        let clearance = edges()
            .map(|(&[ax, ay], &[bx, by])| {
                super::project([x / aspect, y], [ax / aspect, ay], [bx / aspect, by]).1
            })
            .fold(f64::INFINITY, f64::min)
            .sqrt();
        if best.is_none_or(|(prior, _)| clearance > prior) {
            best = Some((clearance, Point::new(x as f32, y as f32)));
        }
    }
    let (clearance, position) = best?;
    label.position = position;
    let usable_height = 2. * clearance;
    label.size *= ((0.82 * usable_height as f32) / (ink.max.y - ink.min.y).max(1.)).min(1.);
    // Small islands still get their name when zoomed in.
    label.size = label.size.max(crate::document::limits::MIN_LABEL_SIZE);
    Some(label)
}
