//! Apply one rigid translation across the selected artwork and its source layers.
use super::Editor;
use crate::document::{Document, Point, SelectionId, limits::MAX_STROKES};
use std::collections::BTreeMap;

impl Editor {
    pub fn preview_selection_move(
        &self,
        items: &[(SelectionId, Point)],
        delta: Point,
    ) -> Option<Document> {
        let mut document = self.document.clone();
        let translate = |point: &mut Point| {
            point.x += delta.x;
            point.y += delta.y;
            point.is_in_world()
        };
        let mut placements = BTreeMap::<_, Vec<_>>::new();
        for &(item, anchor) in items {
            if !item.is_visible(&self.document) {
                return None;
            }
            let layer = document.layer_mut(item.layer())?;
            let valid = match item {
                SelectionId::Label { id, .. } => translate(
                    &mut layer
                        .labels
                        .iter_mut()
                        .find(|label| label.id == id)?
                        .position,
                ),
                SelectionId::Landmark { id, .. } => translate(
                    &mut layer
                        .landmarks
                        .iter_mut()
                        .find(|landmark| landmark.id == id)?
                        .position,
                ),
                SelectionId::Course(id) => layer.strokes[id.index].points.iter_mut().all(translate),
                SelectionId::Relief { key, layer, .. } => {
                    let mut position = anchor;
                    let valid = translate(&mut position);
                    placements.entry(layer).or_default().push((key, position));
                    valid
                }
            };
            if !valid {
                return None;
            }
        }
        for (id, moves) in placements {
            let layer = document.layer_mut(id)?;
            layer.move_relief(moves);
            if layer.relief_moves.len() > MAX_STROKES {
                return None;
            }
        }
        Some(document)
    }

    pub fn move_selection(&mut self, items: &[(SelectionId, Point)], delta: Point) -> bool {
        if items.is_empty() || (delta.x == 0. && delta.y == 0.) {
            return false;
        }
        let Some(document) = self.preview_selection_move(items, delta) else {
            return false;
        };
        if document == self.document {
            return false;
        }
        self.checkpoint();
        self.document = document;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{Brush, Point, ReliefKey, Specimen, Stroke},
        scene::SceneCache,
    };

    fn stroke(brush: Brush, x: f32, y: f32) -> Stroke {
        Stroke {
            creation_id: None,
            material: None,
            brush,
            points: vec![Point::new(x, y)].into(),
            size: 110.,
            strength: 0.85,
            seed: 7,
        }
    }

    #[test]
    fn moving_individual_specimens_preserves_neighbors_and_respects_later_erasing() {
        let mut editor = Editor::blank();
        assert!(editor.commit_stroke(stroke(Brush::Forest, 170., 270.)));
        let mut ridge = stroke(Brush::Mountain, 430., 275.);
        ridge.points.push(Point::new(760., 275.));
        assert!(editor.commit_stroke(ridge));
        let mut cache = SceneCache::default();
        let before = editor.document().clone();
        let scene = cache.render(&before, None);
        let tree = scene
            .forest_trees()
            .min_by_key(|tree| tree.id)
            .unwrap()
            .clone();
        let peak = scene
            .mountain_parts()
            .find(|peak| peak.id.3 == 0)
            .unwrap()
            .clone();
        assert!(scene.mountain_parts().any(|other| other.id != peak.id));
        assert!(scene.forest_trees().any(|other| other.id != tree.id));

        let items = [
            (
                SelectionId::Relief {
                    layer: 1,
                    key: ReliefKey {
                        stroke: 0,
                        specimen: Specimen::Tree {
                            dab: tree.id.2,
                            candidate: tree.id.3,
                        },
                    },
                    seed: tree.seed,
                },
                tree.center,
            ),
            (
                SelectionId::Relief {
                    layer: 1,
                    key: ReliefKey {
                        stroke: 1,
                        specimen: Specimen::Peak(0),
                    },
                    seed: peak.id.2,
                },
                peak.base,
            ),
        ];
        let delta = Point::new(0., 400.);
        let revision = editor.revision();
        assert!(editor.move_selection(&items, delta));
        assert_eq!(editor.revision(), revision + 1);
        let saved = editor.document().clone();
        assert_eq!(saved.layers[0].strokes, before.layers[0].strokes);
        let reopened: Document =
            serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        reopened.validate().unwrap();
        assert_eq!(saved, reopened);
        let moved = cache.render(&reopened, None);
        let moved_tree = moved
            .forest_trees()
            .find(|item| item.id == tree.id)
            .unwrap();
        let moved_peak = moved
            .mountain_parts()
            .find(|item| item.id == peak.id)
            .unwrap();
        assert_eq!(
            moved_tree.center,
            Point::new(tree.center.x, tree.center.y + delta.y)
        );
        assert_eq!(
            moved_peak.base,
            Point::new(peak.base.x, peak.base.y + delta.y)
        );
        assert_eq!(moved_tree.variant, tree.variant);
        for other in scene.forest_trees().filter(|item| item.id != tree.id) {
            assert!(moved.forest_trees().any(|item| item == other));
        }
        for other in scene.mountain_parts().filter(|item| item.id != peak.id) {
            assert!(moved.mountain_parts().any(|item| item == other));
        }
        assert!(editor.undo());
        assert_eq!(editor.document(), &before);
        assert!(editor.redo());
        assert_eq!(editor.document(), &saved);

        let mut erase = stroke(Brush::Erase, tree.center.x, tree.center.y);
        erase.size = 16.;
        assert!(editor.commit_stroke(erase.clone()));
        let erased = cache.render(editor.document(), None);
        assert!(erased.forest_trees().any(|item| item.id == tree.id));
        erase.points = vec![moved_tree.center, moved_peak.base].into();
        let live = cache.render(editor.document(), Some((1, &erase)));
        assert!(!live.forest_trees().any(|item| item.id == tree.id));
        assert!(!live.mountain_parts().any(|item| item.id == peak.id));
        assert!(editor.commit_stroke(erase));
        let finished = cache.render(editor.document(), None);
        assert!(!finished.forest_trees().any(|item| item.id == tree.id));
        assert!(!finished.mountain_parts().any(|item| item.id == peak.id));
    }
}
