//! Generate editable cuts that keep each resulting piece connected.
use super::cut;
use crate::document::{
    Point, RegionAddress, RegionLayer, RegionPolygon, limits::MAX_WORLD_COORDINATE,
};

pub const MAX_SPLIT_COUNT: usize = 16;

pub fn split_land(
    polygon: &RegionPolygon,
    names: Vec<String>,
    owner: Option<usize>,
    count: usize,
) -> Result<RegionLayer, String> {
    if !(2..=MAX_SPLIT_COUNT).contains(&count) {
        return Err("Choose between 2 and 16 regions.".into());
    }
    let mut layer = RegionLayer {
        names,
        ..Default::default()
    };
    let owner = owner.unwrap_or_else(|| layer.new_region());
    let root = layer.claim(polygon.clone(), owner);
    let points = polygon.first().ok_or("Select land to split.")?;
    let frame = [projection(points, [1., 0.]), projection(points, [0., 1.])];
    let mut pending = vec![(root, polygon.clone(), count)];
    while let Some((address, polygon, count)) = pending.pop() {
        if count == 1 {
            continue;
        }
        let choice = choose_cut(&polygon, frame, count)
            .ok_or("This land could not be split into that many connected regions.")?;
        let first = layer.areas[0].nodes.len();
        let new_owner = layer.new_region();
        assert!(layer.split(address, choice.path, new_owner));
        for (index, (polygon, count)) in choice
            .sides
            .into_iter()
            .zip(choice.counts)
            .enumerate()
            .rev()
        {
            pending.push((
                RegionAddress {
                    area: 0,
                    node: first + index,
                },
                polygon,
                count,
            ));
        }
    }
    layer.validate()?;
    Ok(layer)
}

struct Candidate {
    path: Vec<Point>,
    sides: [RegionPolygon; 2],
    counts: [usize; 2],
    imbalance: f64,
}

fn choose_cut(polygon: &RegionPolygon, frame: [[f64; 2]; 2], count: usize) -> Option<Candidate> {
    let exterior = polygon.first()?;
    let [x0, x1] = projection(exterior, [1., 0.]);
    let [y0, y1] = projection(exterior, [0., 1.]);
    let primary = if x1 - x0 >= y1 - y0 {
        [[1., 0.], [0., 1.]]
    } else {
        [[0., 1.], [1., 0.]]
    };
    let diagonal = std::f64::consts::FRAC_1_SQRT_2;
    let short = count / 2;
    let target = short as f64 / count as f64;
    let mut best: Option<Candidate> = None;

    for axis in primary
        .into_iter()
        .chain([[diagonal, diagonal], [diagonal, -diagonal]])
    {
        let normal = [-axis[1], axis[0]];
        let [min, max] = projection(exterior, axis);
        let [t0, t1] = projection(
            &[
                [frame[0][0], frame[1][0]],
                [frame[0][0], frame[1][1]],
                [frame[0][1], frame[1][0]],
                [frame[0][1], frame[1][1]],
            ],
            normal,
        );
        let margin = ((t1 - t0) * 0.02).clamp(2., 32.);
        for fraction in [target, 0.5, 1. - target, 0.4, 0.6, 0.25, 0.75, 0.125, 0.875] {
            let cross = min + (max - min) * fraction;
            let clamp = |q: f64| (q as f32).clamp(-MAX_WORLD_COORDINATE, MAX_WORLD_COORDINATE);
            let path: Vec<_> = [t0 - margin, t1 + margin]
                .map(|t| {
                    Point::new(
                        clamp(axis[0] * cross + normal[0] * t),
                        clamp(axis[1] * cross + normal[1] * t),
                    )
                })
                .into();
            let Some((sides, _)) = cut(polygon, &path) else {
                continue;
            };
            let [a, b] = sides.each_ref().map(area);
            let fraction = a.min(b) / (a + b);
            if !fraction.is_finite() || fraction < target * 0.15 {
                continue;
            }
            let counts = if a <= b {
                [short, count - short]
            } else {
                [count - short, short]
            };
            let imbalance = (fraction - target).abs();
            let candidate = Candidate {
                path,
                sides,
                counts,
                imbalance,
            };
            if imbalance <= 0.055 {
                return Some(candidate);
            }
            if best.as_ref().is_none_or(|prev| imbalance < prev.imbalance) {
                best = Some(candidate);
            }
        }
    }
    best
}

fn projection(points: &[[f64; 2]], [x, y]: [f64; 2]) -> [f64; 2] {
    points
        .iter()
        .fold([f64::INFINITY, f64::NEG_INFINITY], |[low, high], point| {
            let dot = point[0] * x + point[1] * y;
            [low.min(dot), high.max(dot)]
        })
}

pub(super) fn area(polygon: &RegionPolygon) -> f64 {
    polygon
        .iter()
        .flat_map(|ring| ring.iter().zip(ring.iter().cycle().skip(1)))
        .map(|(&[ax, ay], &[bx, by])| ax * by - ay * bx)
        .sum::<f64>()
        .abs()
        * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::RegionNode,
        scene::territories::{RegionView, Shoreline},
    };

    #[test]
    fn generates_the_requested_count_on_a_concave_shore_with_a_lake() {
        let shore = vec![
            vec![
                [0., 0.],
                [960., 0.],
                [960., 570.],
                [620., 570.],
                [620., 500.],
                [390., 500.],
                [390., 570.],
                [0., 570.],
            ],
            vec![[400., 200.], [400., 320.], [550., 320.], [550., 200.]],
        ];
        for count in [2, 3, 8, MAX_SPLIT_COUNT] {
            let draft =
                split_land(&shore, Vec::new(), None, count).expect("valid draft with a lake");
            let view = RegionView::resolve(&draft, &Shoreline::default()).expect("valid cut tree");
            assert_eq!(view.faces.len(), count);
            assert_eq!(view.borders.len(), count - 1);
            assert!(view.faces.iter().all(|face| area(&face.polygon) > 0.));
            let total: f64 = view.faces.iter().map(|face| area(&face.polygon)).sum();
            assert!((total - area(&shore)).abs() < 1e-4);
            assert!(matches!(draft.areas[0].nodes[0], RegionNode::Cut { .. }));
        }
    }
}
