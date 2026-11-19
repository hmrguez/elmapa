//! Direct manipulation of saved river and trail courses.
use super::Studio;
use bevy::prelude::Vec2;
use elmapa::document::{Document, LinearStrokeId, Point, SharedVec, limits::MAX_STROKE_POINTS};

const DEFAULT_SPACING: f32 = 68.;
const HANDLE_RADIUS: f32 = 10.;

pub(in crate::app) struct PathSelection {
    pub id: LinearStrokeId,
    pub anchor: Point,
    pub density: i32,
    scale: f32,
    nodes: Vec<usize>,
    points: SharedVec<Point>,
    drag: Option<NodeDrag>,
}

struct NodeDrag {
    node: usize,
    start: Point,
    pointer: Point,
    span: std::ops::RangeInclusive<usize>,
    weights: Vec<f32>,
    preview: Option<SharedVec<Point>>,
}

fn vector(point: Point) -> Vec2 {
    Vec2::new(point.x, point.y)
}

impl PathSelection {
    fn new(id: LinearStrokeId, anchor: Point, scale: f32, source: &[Point], budget: usize) -> Self {
        let mut selection = Self {
            id,
            anchor,
            density: 0,
            scale,
            nodes: Vec::new(),
            points: SharedVec::default(),
            drag: None,
        };
        selection.rebuild(source, budget, scale);
        selection
    }

    fn rebuild(&mut self, source: &[Point], budget: usize, scale: f32) {
        self.scale = scale;
        let mut length = 0.;
        let lengths = source.windows(2).map(|pair| {
            length += f64::from(vector(pair[0]).distance(vector(pair[1])));
            length
        });
        let lengths: Vec<_> = std::iter::once(0.).chain(lengths).collect();
        self.nodes = vec![0];
        if length == 0. {
            self.points = source.to_vec().into();
            return;
        }

        let screen_length = length * f64::from(scale);
        let spacing = DEFAULT_SPACING * 0.65_f32.powi(self.density);
        let min_spans = if screen_length >= 30. { 2 } else { 1 };
        let spans = ((screen_length / f64::from(spacing)).round() as usize).clamp(min_spans, 256);
        const STEPS_PER_SPAN: usize = 8;
        let steps = spans * STEPS_PER_SPAN;
        let interval = length / steps as f64;
        // Merge edit samples into the freehand course. Reuse nearby original
        // samples and keep the stroke's geometry intact until a node moves.
        let snap = if budget >= steps - 1 {
            interval / 4.
        } else {
            f64::INFINITY
        };
        let mut points = vec![source[0]];
        let mut step = 1;
        for (pair, distances) in source.windows(2).zip(lengths.windows(2)) {
            let [a, b] = [vector(pair[0]), vector(pair[1])];
            let [start, end] = [distances[0], distances[1]];
            let start_index = points.len() - 1;
            while step < steps && step as f64 * interval <= end {
                let distance = step as f64 * interval;
                let from_start = distance - start;
                let to_end = end - distance;
                let index = if from_start <= snap && from_start <= to_end {
                    start_index
                } else if to_end <= snap {
                    points.len()
                } else {
                    let p = a + (b - a) * (from_start / (end - start)) as f32;
                    points.push(Point::new(p.x, p.y));
                    points.len() - 1
                };
                if step.is_multiple_of(STEPS_PER_SPAN) && index > *self.nodes.last().unwrap() {
                    self.nodes.push(index);
                }
                step += 1;
            }
            points.push(pair[1]);
        }
        let last = points.len() - 1;
        if self.nodes.last() != Some(&last) {
            self.nodes.push(last);
        }
        self.points = points.into();
    }

    pub fn points(&self) -> &SharedVec<Point> {
        self.preview().unwrap_or(&self.points)
    }

    pub fn nodes(&self) -> impl ExactSizeIterator<Item = Point> + '_ {
        self.nodes.iter().map(|&index| self.points()[index])
    }

    pub fn node(&self, index: usize) -> Option<Point> {
        Some(self.points()[*self.nodes.get(index)?])
    }

    pub fn active_node(&self) -> Option<usize> {
        self.drag.as_ref().map(|drag| drag.node)
    }

    pub fn node_at(&self, point: Point, scale: f32) -> Option<usize> {
        let limit = (HANDLE_RADIUS / scale).powi(2);
        self.nodes()
            .enumerate()
            .map(|(index, node)| (index, vector(node).distance_squared(vector(point))))
            .filter(|(_, distance)| *distance <= limit)
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(node, _)| node)
    }

    fn preview(&self) -> Option<&SharedVec<Point>> {
        self.drag.as_ref()?.preview.as_ref()
    }

    fn begin_drag(&mut self, node: usize, start: Point) {
        let center = self.nodes[node];
        let first = self.nodes[node.saturating_sub(1)];
        let last = self
            .nodes
            .get(node + 1)
            .copied()
            .unwrap_or(self.points.len() - 1);
        let mut length = 0.;
        let mut weights = vec![1.; last - first + 1];
        for (index, pair) in self.points[first..=last].windows(2).enumerate() {
            length += vector(pair[0]).distance(vector(pair[1]));
            weights[index + 1] = length;
        }
        weights[0] = 0.;
        let middle = weights[center - first];
        for weight in &mut weights {
            let span = if *weight < middle {
                middle
            } else {
                length - middle
            };
            let t = if span > 0. {
                1. - (*weight - middle).abs() / span
            } else {
                1.
            };
            // Smooth falloff joins the original course at the neighboring nodes.
            *weight = t * t * (3. - 2. * t);
        }
        self.drag = Some(NodeDrag {
            node,
            start,
            pointer: start,
            span: first..=last,
            weights,
            preview: None,
        });
    }

    fn move_node(&mut self, point: Point, scale: f32) -> bool {
        let Some(drag) = &mut self.drag else {
            return false;
        };
        if point == drag.pointer {
            return false;
        }
        let delta = vector(point) - vector(drag.start);
        if delta.length_squared() * scale * scale < 1. {
            drag.pointer = point;
            return drag.preview.take().is_some();
        }
        let moved: Vec<_> = self.points[drag.span.clone()]
            .iter()
            .zip(&drag.weights)
            .map(|(&original, &weight)| {
                let p = vector(original) + delta * weight;
                Point::new(p.x, p.y)
            })
            .collect();
        if moved.iter().any(|point| !point.is_in_world()) {
            return false;
        }
        let mut points = self.points.clone();
        points[drag.span.clone()].copy_from_slice(&moved);
        drag.pointer = point;
        drag.preview = Some(points);
        true
    }
}

impl Studio {
    pub(in crate::app) fn select_path(&mut self, point: Point) {
        let scale = self.viewport.transform(self.bounds).scale;
        let id = self.scene.snapshot().linear_stroke_at(point, 6. / scale);
        self.path_selection = id.and_then(|id| {
            let stroke = self.editor.linear_stroke(id)?;
            if let Some(mut selection) = self.path_selection.take().filter(|s| s.id == id) {
                selection.anchor = point;
                return Some(selection);
            }
            let budget = MAX_STROKE_POINTS - self.editor.point_count();
            Some(PathSelection::new(id, point, scale, &stroke.points, budget))
        });
    }

    pub(in crate::app) fn clear_path_selection(&mut self) {
        if let Some(selection) = self.path_selection.take() {
            if selection.preview().is_some() {
                self.invalidate_preview();
            }
            self.ui_dirty = true;
        }
    }

    pub(in crate::app) fn cancel_path_drag(&mut self) -> bool {
        let Some(drag) = self
            .path_selection
            .as_mut()
            .and_then(|path| path.drag.take())
        else {
            return false;
        };
        if drag.preview.is_some() {
            self.invalidate_preview();
        }
        true
    }

    pub(in crate::app) fn path_node_at(&self, point: Point) -> Option<usize> {
        let selection = self.path_selection.as_ref()?;
        selection.node_at(point, self.viewport.transform(self.bounds).scale)
    }

    pub(in crate::app) fn begin_path_drag(&mut self, point: Point) -> bool {
        let scale = self.viewport.transform(self.bounds).scale;
        let Some(selection) = &mut self.path_selection else {
            return false;
        };
        let Some(node) = selection.node_at(point, scale) else {
            return false;
        };
        selection.begin_drag(node, point);
        self.animate_new = false;
        true
    }

    pub(in crate::app) fn path_dragging(&self) -> bool {
        self.path_selection
            .as_ref()
            .and_then(PathSelection::active_node)
            .is_some()
    }

    pub(in crate::app) fn move_path_node(&mut self, point: Point) {
        let scale = self.viewport.transform(self.bounds).scale;
        if self
            .path_selection
            .as_mut()
            .is_some_and(|path| path.move_node(point, scale))
        {
            // Keep publishing completed frames during a continuous drag.
            self.scene.invalidate();
        }
    }

    pub(in crate::app) fn finish_path_drag(&mut self) {
        let Some(selection) = &mut self.path_selection else {
            return;
        };
        let Some(NodeDrag {
            node,
            preview: Some(points),
            ..
        }) = selection.drag.take()
        else {
            return;
        };
        if self
            .editor
            .update_linear_stroke_points(selection.id, points.clone())
        {
            selection.anchor = points[selection.nodes[node]];
            selection.points = points;
            self.changed();
        } else {
            self.invalidate_preview();
        }
    }

    pub(in crate::app) fn preview_path(&self, document: &mut Document) {
        if let Some(selection) = &self.path_selection
            && let Some(points) = selection.preview()
            && let Some(layer) = document.layer_mut(selection.id.layer)
        {
            layer.strokes[selection.id.index].points = points.clone();
        }
    }

    pub(in crate::app) fn refresh_path_nodes(&mut self, density_step: i32) {
        let Some(selection) = &mut self.path_selection else {
            return;
        };
        if selection.drag.is_some() {
            return;
        }
        let scale = self.viewport.transform(self.bounds).scale;
        let density = (selection.density + density_step).clamp(-2, 2);
        if density == selection.density && (0.6..=1.65).contains(&(scale / selection.scale)) {
            return;
        }
        if let Some(stroke) = self.editor.linear_stroke(selection.id) {
            selection.density = density;
            selection.rebuild(
                &stroke.points,
                MAX_STROKE_POINTS - self.editor.point_count(),
                scale,
            );
            self.ui_dirty = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elmapa::document::{Brush, Stroke};

    #[test]
    fn editing_a_sparse_path_preserves_other_nodes_and_commits_once() {
        for brush in [Brush::River, Brush::Path] {
            let mut studio = Studio::default();
            let a = Point::new(0., 0.);
            let b = Point::new(272., 0.);
            let original = Stroke {
                creation_id: None,
                material: None,
                brush,
                points: vec![a, b].into(),
                size: 70.,
                strength: 0.8,
                seed: 27,
            };
            assert!(studio.editor.commit_stroke(original));
            let saved = studio.editor.document().clone();
            let id = LinearStrokeId {
                layer: 1,
                index: 0,
                seed: 27,
            };
            let original_points = &studio.editor.linear_stroke(id).unwrap().points;
            let center = Point::new(136., 0.);
            let mut path = PathSelection::new(id, center, 1., original_points, 1000);
            assert_eq!(path.nodes().len(), 5);
            path.begin_drag(2, center);
            assert!(path.move_node(Point::new(136., 40.), 1.));
            studio.path_selection = Some(path);
            assert!(studio.cancel_path_drag());
            assert_eq!(studio.editor.document(), &saved);

            let path = studio.path_selection.as_mut().unwrap();
            path.begin_drag(2, center);
            assert!(path.move_node(Point::new(136., 60.), 1.));
            assert!(path.move_node(Point::new(136., 40.), 1.));
            assert_eq!(
                path.nodes().collect::<Vec<_>>(),
                [
                    a,
                    Point::new(68., 0.),
                    Point::new(136., 40.),
                    Point::new(204., 0.),
                    b,
                ]
            );
            let mut preview = saved.clone();
            studio.preview_path(&mut preview);
            let bent = &preview.layers[0].strokes[0];
            assert!(bent.points.iter().any(|p| p.y > 0. && p.y < 40.));
            assert_eq!(studio.editor.document(), &saved);
            let revision = studio.editor.revision();
            studio.finish_path_drag();
            assert_eq!(studio.editor.revision(), revision + 1);
            assert_eq!(studio.editor.document(), &preview);
            assert_eq!(studio.editor.point_count(), bent.points.len());
            assert_eq!(studio.editor.stroke_count(), 1);
            assert_eq!(
                studio.editor.linear_stroke(id).unwrap().creation_id,
                saved.layers[0].strokes[0].creation_id
            );
            assert!(studio.editor.undo());
            assert_eq!(studio.editor.document(), &saved);
            assert!(studio.editor.redo());
            assert_eq!(studio.editor.document(), &preview);
        }
    }
}
