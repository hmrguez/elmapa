//! Resolve authored region cuts against the visible shoreline.
mod labels;
mod split;
pub use split::{MAX_SPLIT_COUNT, split_land};

use crate::document::{
    ContentBounds, Point, RegionAddress, RegionArea, RegionLayer, RegionNode, RegionPolygon,
};
use i_overlay::{
    core::{fill_rule::FillRule, overlay_rule::OverlayRule},
    float::{clip::FloatClip, single::SingleFloatOverlay, slice::FloatSlice},
    string::clip::ClipRule,
};
use std::sync::{Arc, OnceLock};

use super::{
    Shape, generation,
    shape::line,
    terrain::materials::{coordinates, granulated_surface, points},
};

pub const BORDER_INK: u32 = 0x594731;
pub const WASHES: [u32; 9] = [
    0xb66948, 0x6e8244, 0x806684, 0x46798a, 0xb19043, 0x876151, 0x598374, 0x9b576b, 0x797d9c,
];

pub fn washes_for(style: crate::document::AtlasStyle) -> [u32; 9] {
    if style.is_classic() {
        WASHES
    } else {
        [
            0xaa785d, 0x818963, 0x918092, 0x718f8a, 0xad935d, 0x927660, 0x7d967f, 0x9f7980,
            0x7e8894,
        ]
    }
}

pub fn pigment(data: &RegionLayer, owner: usize, style: crate::document::AtlasStyle) -> u32 {
    let palette_offset = if data.parent.is_some() { 3 } else { 0 };
    data.styles
        .get(&owner)
        .and_then(|style| style.color)
        .unwrap_or(washes_for(style)[(owner + palette_offset) % WASHES.len()])
}

#[derive(Default)]
pub struct Shoreline {
    pub(super) surfaces: Vec<Vec<(crate::document::Brush, Arc<RegionPolygon>)>>,
    land: OnceLock<Vec<(ContentBounds, RegionPolygon)>>,
}

impl Shoreline {
    fn land(&self) -> &[(ContentBounds, RegionPolygon)] {
        self.land.get_or_init(|| {
            use crate::document::Brush;
            let mut land: Vec<RegionPolygon> = Vec::new();
            for layer in &self.surfaces {
                for brush in [Brush::Land, Brush::Water] {
                    // Tiles have disjoint interiors. NonZero stitches their shared edges
                    // and preserves holes when merging adjacent tiles.
                    let rings: RegionPolygon = layer
                        .iter()
                        .filter(|(kind, _)| *kind == brush)
                        .flat_map(|(_, rings)| rings.iter().cloned())
                        .collect();
                    if rings.is_empty() {
                        continue;
                    }
                    let rule = if brush == Brush::Land {
                        OverlayRule::Union
                    } else {
                        OverlayRule::Difference
                    };
                    land = land.overlay_as::<i64>(&rings, rule, FillRule::NonZero);
                }
            }
            land.into_iter()
                .filter_map(|polygon| Some((bounds(polygon.iter().flatten())?, polygon)))
                .collect()
        })
    }

    /// Restrict terrain to every enclosing region, preserving disconnected land.
    pub fn scoped(
        world: &Arc<Self>,
        ancestors: &[(usize, &Arc<RegionLayer>)],
    ) -> Option<Arc<Self>> {
        let mut shore = world.clone();
        for &(owner, layer) in ancestors {
            let view = RegionView::resolve(layer, &shore)?;
            let mut pieces: Vec<_> = view
                .faces
                .iter()
                .filter(|face| face.owner == owner)
                .flat_map(|face| visible_face(&face.polygon, &shore))
                .collect();
            if pieces.len() > 1 {
                let rings: RegionPolygon = pieces.into_iter().flatten().collect();
                pieces = Vec::<RegionPolygon>::new().overlay_as::<i64>(
                    &rings,
                    OverlayRule::Union,
                    FillRule::NonZero,
                );
            }
            let land: Vec<_> = pieces
                .into_iter()
                .filter_map(|piece| Some((bounds(piece.iter().flatten())?, piece)))
                .collect();
            shore = Arc::new(Self {
                land: OnceLock::from(land),
                ..Self::default()
            });
        }
        Some(shore)
    }

    pub fn visible_path(&self, path: &[Point]) -> Vec<Vec<Point>> {
        let path = coordinates(path);
        clip(&path, &self.nearby(path.iter()))
            .iter()
            .map(|span| points(span))
            .collect()
    }

    pub fn island_at(&self, point: Point) -> Option<&RegionPolygon> {
        let point_bounds = ContentBounds {
            min: point,
            max: point,
        };
        self.land().iter().find_map(|(bounds, shape)| {
            (bounds.intersects(point_bounds) && contains(shape, point)).then_some(shape)
        })
    }

    fn nearby<'a>(&self, points: impl Iterator<Item = &'a [f64; 2]>) -> Vec<&[[f64; 2]]> {
        let Some(target) = bounds(points) else {
            return Vec::new();
        };
        self.land()
            .iter()
            .filter(|(bounds, _)| bounds.intersects(target))
            .flat_map(|(_, shape)| shape.iter().map(Vec::as_slice))
            .collect()
    }
}

fn bounds<'a>(points: impl Iterator<Item = &'a [f64; 2]>) -> Option<ContentBounds> {
    points
        .map(|&[x, y]| {
            let point = Point::new(x as f32, y as f32);
            ContentBounds {
                min: point,
                max: point,
            }
        })
        .reduce(ContentBounds::union)
}

pub fn contains(polygon: &RegionPolygon, point: Point) -> bool {
    let x = f64::from(point.x);
    let y = f64::from(point.y);
    let mut inside = false;
    for ring in polygon {
        for (&[ax, ay], &[bx, by]) in ring.iter().zip(ring.iter().cycle().skip(1)) {
            if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
                inside = !inside;
            }
        }
    }
    inside
}

pub fn unclaimed_at(
    island: &RegionPolygon,
    layer: &RegionLayer,
    point: Point,
) -> Option<RegionPolygon> {
    if layer.areas.is_empty() {
        return Some(island.clone());
    }
    let target = bounds(island.iter().flatten())?;
    let claimed: RegionPolygon = layer
        .areas
        .iter()
        .filter(|area| bounds(area.land.iter().flatten()).is_some_and(|b| b.intersects(target)))
        .flat_map(|area| area.land.iter().cloned())
        .collect();
    island
        .overlay_as::<i64>(&claimed, OverlayRule::Difference, FillRule::EvenOdd)
        .into_iter()
        .find(|shape| contains(shape, point))
}

pub struct RegionFace {
    pub address: RegionAddress,
    pub owner: usize,
    pub polygon: RegionPolygon,
}

pub struct RegionBorder {
    pub address: RegionAddress,
    pub visible: Vec<Vec<Point>>,
}

#[derive(Default)]
pub struct RegionView {
    pub faces: Vec<RegionFace>,
    pub borders: Vec<RegionBorder>,
}

impl RegionView {
    pub fn resolve(layer: &RegionLayer, shore: &Shoreline) -> Option<Self> {
        let mut view = Self::default();
        for (area, scope) in layer.areas.iter().enumerate() {
            if generation::stopped() {
                return None;
            }
            view.add_area(area, scope, shore)?;
        }
        Some(view)
    }

    fn add_area(&mut self, area: usize, scope: &RegionArea, shore: &Shoreline) -> Option<()> {
        let mut stack = vec![(0, scope.land.clone())];
        while let Some((node, polygon)) = stack.pop() {
            if generation::stopped() {
                return None;
            }
            let address = RegionAddress { area, node };
            match scope.nodes.get(node)? {
                &RegionNode::Owned(owner) => {
                    self.faces.push(RegionFace {
                        address,
                        owner,
                        polygon,
                    });
                }
                RegionNode::Cut { path, children } => {
                    let (sides, border) = cut(&polygon, path)?;
                    let visible = clip(&border, &shore.nearby(border.iter()))
                        .iter()
                        .map(|span| points(span))
                        .collect();
                    self.borders.push(RegionBorder { address, visible });
                    stack.extend(children.iter().copied().zip(sides).rev());
                }
            }
        }
        Some(())
    }

    /// Focus the largest visible piece while retaining its editable face.
    pub fn focus(&self, owner: usize, shore: &Shoreline) -> Option<(&RegionFace, Point)> {
        let (face, piece, _) = self
            .faces
            .iter()
            .filter(|face| face.owner == owner)
            .flat_map(|face| {
                visible_face(&face.polygon, shore)
                    .into_iter()
                    .map(move |piece| {
                        let area = split::area(&piece);
                        (face, piece, area)
                    })
            })
            .max_by(|a, b| a.2.total_cmp(&b.2))?;
        Some((face, interior(&piece, 1.)?.1))
    }

    pub fn washes(
        &self,
        shore: &Shoreline,
        data: &RegionLayer,
        style: crate::document::AtlasStyle,
    ) -> Vec<Shape> {
        let mut shapes = Vec::new();
        for face in &self.faces {
            if generation::stopped() {
                break;
            }
            let land = visible_face(&face.polygon, shore);
            granulated_surface(
                &mut shapes,
                land.into_iter().flatten().collect(),
                pigment(data, face.owner, style),
                if style.is_classic() { 0.30 } else { 0.24 },
            );
        }
        shapes
    }

    pub fn shapes(
        &self,
        shore: &Shoreline,
        data: &RegionLayer,
        style: crate::document::AtlasStyle,
    ) -> Vec<Shape> {
        let mut shapes = self.washes(shore, data, style);
        let border_ink = if style.is_classic() {
            BORDER_INK
        } else {
            super::palette::UMBER
        };
        for border in &self.borders {
            for span in &border.visible {
                line(&mut shapes, span.clone(), border_ink, 1.6, 0.89);
            }
        }
        for label in labels::place(self, shore, data) {
            let start = shapes.len();
            shapes.extend(super::label_shapes(&label).iter().cloned());
            super::palette::recolor(&mut shapes[start..], |_| label.ink.color_in(style));
        }
        shapes
    }
}

// Find an interior point with clearance in the requested aspect ratio.
fn interior(polygon: &RegionPolygon, aspect: f64) -> Option<(f64, Point)> {
    let extent = bounds(polygon.iter().flatten())?;
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
        // Measure clearance in the target's proportions so wide objects
        // have room at their ends as well as at their center.
        let clearance = edges()
            .map(|(&[ax, ay], &[bx, by])| {
                project([x / aspect, y], [ax / aspect, ay], [bx / aspect, by]).1
            })
            .fold(f64::INFINITY, f64::min)
            .sqrt();
        if best.is_none_or(|(prior, _)| clearance > prior) {
            best = Some((clearance, Point::new(x as f32, y as f32)));
        }
    }
    best
}

pub fn visible_face(polygon: &RegionPolygon, shore: &Shoreline) -> Vec<RegionPolygon> {
    polygon.overlay_as::<i64>(
        &shore.nearby(polygon.iter().flatten()),
        OverlayRule::Intersect,
        FillRule::EvenOdd,
    )
}

fn clip<T>(path: &Vec<[f64; 2]>, polygon: &T) -> Vec<Vec<[f64; 2]>>
where
    Vec<[f64; 2]>: FloatClip<T, [f64; 2]>,
    T: i_overlay::i_shape::source::float::resource::ShapeResource<[f64; 2]>,
{
    path.clip_by_as::<i64>(
        polygon,
        FillRule::EvenOdd,
        ClipRule {
            invert: false,
            boundary_included: false,
        },
    )
}

fn continuous_border(mut spans: Vec<Vec<[f64; 2]>>) -> Option<Vec<[f64; 2]>> {
    let mut border = spans.pop()?;
    // The clipper can return adjoining segments separately and out of order.
    while !spans.is_empty() {
        let index = spans
            .iter()
            .position(|span| span.first() == border.last() || span.last() == border.first())?;
        let mut span = spans.swap_remove(index);
        if span.first() == border.last() {
            border.extend(span.into_iter().skip(1));
        } else {
            span.pop();
            span.append(&mut border);
            border = span;
        }
    }
    Some(border)
}

/// Extract one directed border and its two neighbors. A cut through more than
/// one span (for example, across an inlet) can be made as separate gestures.
pub fn cut(polygon: &RegionPolygon, path: &[Point]) -> Option<([RegionPolygon; 2], Vec<[f64; 2]>)> {
    let path = coordinates(path);
    let border = continuous_border(clip(&path, polygon))?;
    let [first, second]: [_; 2] = polygon
        .slice_by_as::<i64>(&path, FillRule::EvenOdd)
        .try_into()
        .ok()?;
    let &[a, b] = border
        .windows(2)
        .max_by(|a, b| length_squared(a[0], a[1]).total_cmp(&length_squared(b[0], b[1])))?
    else {
        return None;
    };
    let direction = [b[0] - a[0], b[1] - a[1]];
    let midpoint = [f64::midpoint(a[0], b[0]), f64::midpoint(a[1], b[1])];
    let orientation = |poly: &RegionPolygon| {
        poly.iter().find_map(|ring| {
            ring.iter()
                .zip(ring.iter().cycle().skip(1))
                .find_map(|(&p, &q)| {
                    let (_, distance) = project(midpoint, p, q);
                    (distance < 1e-12)
                        .then_some((q[0] - p[0]) * direction[0] + (q[1] - p[1]) * direction[1])
                })
        })
    };
    match (orientation(&first)?, orientation(&second)?) {
        (a, b) if a > 0. && b < 0. => Some(([first, second], border)),
        (a, b) if a < 0. && b > 0. => Some(([second, first], border)),
        _ => None,
    }
}

fn length_squared(a: [f64; 2], b: [f64; 2]) -> f64 {
    (b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)
}

fn project(q: [f64; 2], a: [f64; 2], b: [f64; 2]) -> (f64, f64) {
    let length = length_squared(a, b);
    if length == 0. {
        return (0., length_squared(a, q));
    }
    let d = [b[0] - a[0], b[1] - a[1]];
    let t = (((q[0] - a[0]) * d[0] + (q[1] - a[1]) * d[1]) / length).clamp(0., 1.);
    (t, length_squared(q, [a[0] + t * d[0], a[1] + t * d[1]]))
}

/// Closest point, squared distance, and source segment index.
pub fn nearest_path(q: Point, path: &[Point]) -> Option<(Point, f64, usize)> {
    path.windows(2)
        .enumerate()
        .map(|(index, pair)| {
            let [a, b] = [pair[0], pair[1]];
            let coords = |p: Point| [f64::from(p.x), f64::from(p.y)];
            let (t, distance) = project(coords(q), coords(a), coords(b));
            (
                Point::new(a.x + (b.x - a.x) * t as f32, a.y + (b.y - a.y) * t as f32),
                distance,
                index,
            )
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{Brush, Document, Stroke},
        editor::Editor,
        scene::{SceneCache, to_svg},
    };

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> RegionPolygon {
        vec![vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]]
    }

    fn point(x: f32, y: f32) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn moving_a_shared_cut_keeps_the_side_owners_and_does_not_overlap() {
        let land = rect(0., 0., 200., 160.);
        let straight = [point(100., -10.), point(100., 170.)];
        let (before, _) = cut(&land, &straight).expect("two sides");
        let bent = [straight[0], point(130., 80.), straight[1]];
        let (after, border) = cut(&land, &bent).expect("reshaped sides");
        for point in [point(10., 80.), point(190., 80.)] {
            assert_eq!(contains(&before[0], point), contains(&after[0], point));
        }
        let changed = point(115., 80.);
        assert_ne!(contains(&before[0], changed), contains(&after[0], changed));
        assert_ne!(contains(&after[0], changed), contains(&after[1], changed));
        assert_eq!(border.len(), 3);
        assert!(
            after[0]
                .overlay_as::<i64>(&after[1], OverlayRule::Intersect, FillRule::EvenOdd)
                .is_empty()
        );
        assert!(cut(&land, &[point(70., 20.), point(70., 125.)]).is_none());
    }

    #[test]
    fn selecting_land_stitches_tiles_and_keeps_islands_and_water_separate() {
        let mut editor = Editor::blank();
        let paint = |brush, size, points: Vec<_>| Stroke {
            creation_id: None,
            material: None,
            brush,
            points: points.into(),
            size,
            strength: 1.,
            seed: 40,
        };
        assert!(editor.commit_stroke(paint(
            Brush::Land,
            140.,
            vec![point(0., 0.), point(600., 0.)]
        )));
        assert!(editor.commit_stroke(paint(Brush::Land, 100., vec![point(500., 200.)])));
        let water = editor.add_layer();
        assert!(editor.commit_stroke(paint(Brush::Water, 28., vec![point(300., 0.)])));
        let mut cache = SceneCache::default();
        let scene = cache.render(editor.document(), None);
        let shore = scene.shoreline();
        let mainland = shore.island_at(point(0., 0.)).expect("mainland");
        assert!(contains(mainland, point(600., 0.)));
        assert!(!contains(mainland, point(500., 200.)));
        assert!(shore.island_at(point(300., 0.)).is_none());

        let mut layer = RegionLayer::default();
        let west = layer.new_region();
        let east = layer.new_region();
        let mainland = layer.claim(mainland.clone(), west);
        assert!(layer.split(mainland, vec![point(420., -110.), point(420., 110.)], east));
        let island = layer.claim(shore.island_at(point(500., 200.)).unwrap().clone(), east);
        editor
            .set_regions(None, layer, shore)
            .expect("valid split and island");
        let id = editor.active_layer();

        let scene = cache.render(editor.document(), None);
        let view = scene.region_view(id).unwrap();
        assert_eq!(view.borders.len(), 1);
        assert_eq!(
            view.faces.iter().filter(|face| face.owner == east).count(),
            2
        );
        assert!(
            view.faces
                .iter()
                .any(|face| face.address == island && face.owner == east)
        );
        let svg = to_svg(editor.document());
        assert!(
            svg.contains("#5d4b3b"),
            "committed border appears in export"
        );
        let json = serde_json::to_string(editor.document()).unwrap();
        let loaded = serde_json::from_str::<Document>(&json).unwrap();
        assert_eq!(loaded, *editor.document());
        loaded.validate().unwrap();

        assert!(editor.select_layer(water));
        assert!(editor.commit_stroke(paint(
            Brush::Water,
            22.,
            vec![point(420., -100.), point(420., 100.)]
        )));
        let scene = cache.render(editor.document(), None);
        assert!(scene.shoreline().island_at(point(420., 0.)).is_none());
        assert!(scene.region_view(id).unwrap().borders[0].visible.is_empty());
        assert!(editor.undo());
        assert!(
            !cache
                .render(editor.document(), None)
                .region_view(id)
                .unwrap()
                .borders[0]
                .visible
                .is_empty()
        );
        assert!(editor.toggle_layer(id));
        assert!(
            cache
                .render(editor.document(), None)
                .region_view(id)
                .is_none()
        );
    }
}
