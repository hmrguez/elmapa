use crate::document::{Point, Stroke};

pub(super) fn p(x: f32, y: f32) -> Point {
    Point { x, y }
}
pub(super) fn distance(a: Point, b: Point) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}
pub(super) fn segment_distance(q: Point, a: Point, b: Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let t =
        (((q.x - a.x) * dx + (q.y - a.y) * dy) / (dx * dx + dy * dy).max(0.0001)).clamp(0.0, 1.0);
    distance(q, p(a.x + t * dx, a.y + t * dy))
}
pub(super) fn stroke_distance(q: Point, stroke: &Stroke) -> f32 {
    if stroke.points.len() == 1 {
        return distance(q, stroke.points[0]);
    }
    stroke
        .points
        .windows(2)
        .map(|a| segment_distance(q, a[0], a[1]))
        .fold(f32::INFINITY, f32::min)
}
pub(super) fn area(points: &[Point]) -> f32 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum::<f32>()
        * 0.5
}
pub(super) fn contains(points: &[Point], q: Point) -> bool {
    let mut inside = false;
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        if (a.y > q.y) != (b.y > q.y) && q.x < (b.x - a.x) * (q.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}
pub(super) fn softened(points: &[Point]) -> Vec<Point> {
    let mut result = Vec::with_capacity(points.len() * 2);
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        result.push(p(a.x * 0.75 + b.x * 0.25, a.y * 0.75 + b.y * 0.25));
        result.push(p(a.x * 0.25 + b.x * 0.75, a.y * 0.25 + b.y * 0.75));
    }
    result
}
pub(super) fn sampled(stroke: &Stroke, spacing: f32) -> Vec<Point> {
    let mut result = Vec::new();
    if let Some(first) = stroke.points.first() {
        result.push(*first);
    }
    let mut carry = 0.0;
    for pair in stroke.points.windows(2) {
        let len = distance(pair[0], pair[1]);
        if len < 0.001 {
            continue;
        }
        let mut d = spacing - carry;
        while d <= len {
            if !super::generation::samples(1) {
                return result;
            }
            let t = d / len;
            result.push(p(
                pair[0].x + (pair[1].x - pair[0].x) * t,
                pair[0].y + (pair[1].y - pair[0].y) * t,
            ));
            d += spacing;
        }
        carry = (carry + len) % spacing;
    }
    if let Some(last) = stroke.points.last()
        && result.last().is_none_or(|q| distance(*q, *last) > 1.0)
    {
        result.push(*last);
    }
    result
}
