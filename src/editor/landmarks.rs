use super::Editor;
use crate::document::{Landmark, Point};

impl Editor {
    pub fn landmark(&self, id: u64) -> Option<&Landmark> {
        self.annotation(id)
    }
    pub fn landmark_at(&self, point: Point, tolerance: f32) -> Option<(u64, u64)> {
        self.annotation_at::<Landmark>(point, tolerance)
    }
    pub fn add_landmark(&mut self, landmark: Landmark) -> Option<u64> {
        self.add_annotation(landmark)
    }
    pub fn update_landmark(&mut self, landmark: Landmark) -> bool {
        self.update_annotation(landmark)
    }
    pub fn delete_landmark(&mut self, id: u64) -> bool {
        self.delete_annotation::<Landmark>(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::LandmarkKind;
    #[test]
    fn placement_replacement_movement_deletion_and_hidden_layers_obey_history() {
        let mut e = Editor::blank();
        let id = e.add_landmark(Landmark::default()).unwrap();
        let placed = e.document().clone();
        let mut p = e.landmark(id).unwrap().clone();
        p.kind = LandmarkKind::from("town");
        p.position = Point::new(-100., 400.);
        assert!(e.update_landmark(p.clone()));
        assert!(e.undo());
        assert_eq!(e.document(), &placed);
        assert!(e.redo());
        assert_eq!(e.landmark(id), Some(&p));
        let upper = e.add_layer();
        let id2 = e.add_landmark(p.clone()).unwrap();
        assert_eq!(
            e.landmark_at(Point::new(-100., 390.), 1.),
            Some((upper, id2))
        );
        e.toggle_layer(upper);
        assert!(!e.delete_landmark(id2));
        assert!(!e.update_landmark(Landmark {
            id: id2,
            size: 200.,
            ..p.clone()
        }));
        assert!(e.add_landmark(p.clone()).is_none());
        e.select_layer(1);
        assert_eq!(e.landmark_at(Point::new(-100., 390.), 1.), Some((1, id)));
        let revision = e.revision();
        assert!(
            e.add_landmark(Landmark {
                size: f32::INFINITY,
                ..p
            })
            .is_none()
        );
        assert_eq!(e.revision(), revision);
        assert!(e.delete_landmark(id));
        assert!(e.undo());
        assert!(e.landmark(id).is_some());
    }
}
