//! Border hit testing, drawing cuts and moving shared borders.
use super::{BorderDrag, Gesture, Mode, Selection};
use crate::app::state::Studio;
use elmapa::{
    document::{MAX_BORDER_POINTS, Point, RegionAddress, RegionNode},
    scene::territories,
};

struct BorderHit {
    address: RegionAddress,
    distance: f64,
    point: Point,
}

impl Studio {
    pub fn region_border_at(&mut self, point: Point) -> bool {
        matches!(self.regions.mode, Mode::Select | Mode::Split) && self.border_hit(point).is_some()
    }

    fn border_hit(&mut self, point: Point) -> Option<BorderHit> {
        let tolerance = f64::from(8. / self.viewport.zoom).powi(2);
        let view = if self.regions.mode == Mode::Split {
            self.regions.split.as_ref()?.view()
        } else {
            self.region_view()?
        };
        view.borders
            .iter()
            .flat_map(|border| {
                border.visible.iter().filter_map(move |span| {
                    let (position, distance, _) = territories::nearest_path(point, span)?;
                    (distance <= tolerance).then_some(BorderHit {
                        address: border.address,
                        distance,
                        point: position,
                    })
                })
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }

    pub fn region_down(&mut self, point: Point) {
        self.regions.message = None;
        self.regions.style.close_controls();
        if self
            .regions_layer()
            .is_some_and(|layer| !layer.visible || layer.opacity == 0.)
        {
            self.regions.clear();
            self.regions.message = Some("Show this Regions layer to edit it.".into());
        } else {
            match self.regions.mode {
                Mode::Draw => self.regions.gesture = Some(Gesture::Cut(vec![point])),
                Mode::Assign(owner) => {
                    self.regions.gesture = Some(Gesture::Assign { point, owner });
                }
                Mode::Select | Mode::Split => {
                    if let Some(hit) = self.border_hit(point)
                        && let Some(drag) = self.begin_border_drag(hit, point)
                    {
                        self.regions.gesture = Some(Gesture::Move(drag, Vec::new()));
                    } else if self.regions.mode == Mode::Select {
                        self.regions.selection = self.select_land(point);
                        self.regions.style.anchor = self.regions.selection.as_ref().map(|_| point);
                    }
                }
            }
        }
        self.regions.refresh_overlay();
        self.ui_dirty = true;
    }

    fn begin_border_drag(&self, hit: BorderHit, start: Point) -> Option<BorderDrag> {
        let data = if self.regions.mode == Mode::Split {
            &self.regions.split.as_ref()?.data
        } else {
            self.regions_layer()?.territories.as_deref()?
        };
        let RegionNode::Cut { path, .. } = data
            .areas
            .get(hit.address.area)?
            .nodes
            .get(hit.address.node)?
        else {
            return None;
        };
        let (anchor, _, segment) = territories::nearest_path(hit.point, path)?;
        let mut border = path.clone();
        let zoom = self.viewport.zoom;
        let nearby = [segment, segment + 1].into_iter().find(|&index| {
            index > 0 && index + 1 < border.len() && distance(border[index], anchor) * zoom < 5.
        });
        let index = nearby.or_else(|| {
            if border.len() == MAX_BORDER_POINTS {
                return None;
            }
            border.insert(segment + 1, anchor);
            Some(segment + 1)
        })?;
        let anchor = border[index];
        let mut lengths = vec![0.; border.len()];
        for i in (0..index).rev() {
            lengths[i] = lengths[i + 1] + distance(border[i], border[i + 1]);
        }
        for i in index + 1..border.len() {
            lengths[i] = lengths[i - 1] + distance(border[i - 1], border[i]);
        }
        let mut weights: Vec<_> = lengths
            .into_iter()
            .map(|length| {
                let t = (1. - length * zoom / 50.).max(0.);
                t * t * (3. - 2. * t)
            })
            .collect();
        weights[0] = 0.;
        *weights.last_mut()? = 0.;
        Some(BorderDrag {
            address: hit.address,
            start,
            anchor,
            border,
            weights,
            moved: false,
        })
    }

    pub fn region_move(&mut self, point: Point) {
        if !point.is_in_world() {
            return;
        }
        match &mut self.regions.gesture {
            Some(Gesture::Cut(path)) => {
                if path.len() >= MAX_BORDER_POINTS - 3
                    || path.last().is_some_and(|&previous| {
                        distance(point, previous) * self.viewport.zoom < 5.
                    })
                {
                    return;
                }
                path.push(point);
            }
            Some(Gesture::Move(drag, path)) => {
                drag.moved = distance(point, drag.start) * self.viewport.zoom >= 2.;
                if drag.moved {
                    *path = drag
                        .border
                        .iter()
                        .zip(&drag.weights)
                        .map(|(&p, &weight)| translate(p, point, drag.start, weight))
                        .collect();
                } else {
                    path.clear();
                }
                if let (Some(draft), Some(selection)) =
                    (&mut self.regions.split, &self.regions.selection)
                {
                    draft.preview_move(drag.address, path, &selection.shore);
                }
            }
            Some(Gesture::Assign { .. }) | None => return,
        }
        self.regions.refresh_overlay();
    }

    pub fn region_up(&mut self, point: Point, blocked: bool) {
        if blocked {
            self.regions.cancel_gesture();
            return;
        }
        self.region_move(point);
        let Some(gesture) = self.regions.gesture.take() else {
            return;
        };
        match gesture {
            Gesture::Cut(mut path) => {
                if path.last().is_some_and(|&last| distance(last, point) > 0.1)
                    && point.is_in_world()
                {
                    path.push(point);
                }
                self.finish_region_cut(path);
            }
            Gesture::Move(drag, path) if drag.moved => {
                if let Some(draft) = &mut self.regions.split {
                    draft.finish_move(drag.address);
                } else {
                    self.finish_region_drag(drag.address, path);
                }
            }
            Gesture::Assign {
                point: start,
                owner,
            } if distance(point, start) * self.viewport.zoom <= 6. => {
                self.assign_island(start, owner);
            }
            Gesture::Move(..) | Gesture::Assign { .. } => {}
        }
        self.regions.refresh_overlay();
        self.ui_dirty = true;
    }

    fn assign_island(&mut self, point: Point, owner: usize) {
        match self.select_land(point) {
            Some(selection) if selection.owner.is_none() => {
                self.claim_region(selection, Some(owner))
            }
            Some(selection) if selection.owner == Some(owner) => {
                self.regions.message =
                    Some("This land already belongs to the selected region.".into());
            }
            Some(_) => self.regions.message = Some("Choose land without a region.".into()),
            None => {
                self.regions.message = Some(if self.active_region_parent().is_some() {
                    "Choose unassigned land inside the parent region.".into()
                } else {
                    "Click an island to add it to this region.".into()
                });
            }
        }
    }

    fn finish_region_cut(&mut self, path: Vec<Point>) {
        let Some(selection) = self.regions.selection.as_ref() else {
            return;
        };
        let Some(path) = extend_cut(path, 18. / self.viewport.zoom) else {
            self.regions.message = Some("Drag a line across the selected land.".into());
            return;
        };
        let Some((sides, _)) = territories::cut(&selection.polygon, &path) else {
            self.regions.message =
                Some("Draw one line between two edges of the selected land.".into());
            return;
        };
        let shore = selection.shore.clone();
        let (id, mut data) = self.region_data();
        let (address, first) = match (selection.address, selection.owner) {
            (Some(address), Some(owner)) => (address, owner),
            _ => {
                let owner = data.new_region();
                (data.claim(selection.polygon.clone(), owner), owner)
            }
        };
        let next = data.new_region();
        let first_node = data.areas[address.area].nodes.len();
        if !data.split(address, path, next) {
            return;
        }
        match self.editor.set_regions(id, data, &shore) {
            Ok(()) => {
                self.regions.mode = Mode::Select;
                let [polygon, _] = sides;
                let addr = RegionAddress {
                    area: address.area,
                    node: first_node,
                };
                if self
                    .regions
                    .style
                    .anchor
                    .is_some_and(|point| !territories::contains(&polygon, point))
                {
                    self.regions.style.anchor = None;
                }
                self.regions.selection = Some(Selection::new(polygon, Some((addr, first)), shore));
            }
            Err(error) => self.regions.message = Some(error),
        }
    }

    fn finish_region_drag(&mut self, address: RegionAddress, path: Vec<Point>) {
        let (id, mut data) = self.region_data();
        let Some(RegionNode::Cut { path: border, .. }) = data
            .areas
            .get_mut(address.area)
            .and_then(|area| area.nodes.get_mut(address.node))
        else {
            return;
        };
        *border = path;
        let Some(shore) = self.region_shore() else {
            return;
        };
        match self.editor.set_regions(id, data, &shore) {
            Ok(()) => self.regions.selection = None,
            Err(error) => self.regions.message = Some(error),
        }
    }
}
fn distance(a: Point, b: Point) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}

pub(super) fn translate(p: Point, target: Point, start: Point, weight: f32) -> Point {
    Point::new(
        p.x + (target.x - start.x) * weight,
        p.y + (target.y - start.y) * weight,
    )
}

fn extend_cut(mut path: Vec<Point>, reach: f32) -> Option<Vec<Point>> {
    let &first = path.first()?;
    let &last = path.last()?;
    let from = *path
        .iter()
        .find(|&&p| distance(p, first) >= reach.min(6.))?;
    let to = *path
        .iter()
        .rev()
        .find(|&&p| distance(p, last) >= reach.min(6.))?;
    let extension = |p, inward| translate(p, p, inward, reach / distance(p, inward));
    let start = extension(first, from);
    let end = extension(last, to);
    if start.is_in_world() {
        path.insert(0, start);
    }
    if end.is_in_world() {
        path.push(end);
    }
    Some(path)
}
