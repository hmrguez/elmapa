use super::Editor;
use crate::document::{Label, Point};

impl Editor {
    pub fn label(&self, id: u64) -> Option<&Label> {
        self.annotation(id)
    }
    pub fn label_at(&self, point: Point, tolerance: f32) -> Option<(u64, u64)> {
        self.annotation_at::<Label>(point, tolerance)
    }
    pub fn add_label(&mut self, label: Label) -> Option<u64> {
        self.add_annotation(label)
    }
    pub fn add_label_on_layer(&mut self, label: Label, layer: u64) -> Option<u64> {
        self.add_annotation_on_layer(label, layer)
    }
    pub fn update_label(&mut self, label: Label) -> bool {
        self.update_annotation(label)
    }
    pub fn delete_label(&mut self, id: u64) -> bool {
        self.delete_annotation::<Label>(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Brush, Stroke};
    #[test]
    fn labels_share_layer_order_history_and_validation_with_brushwork() {
        let mut e = Editor::blank();
        let id = e.add_label(Label::default()).unwrap();
        let original = e.document().clone();
        let mut label = e.label(id).unwrap().clone();
        label.text = "Éire & the Isles".into();
        label.size = 52.;
        label.rotation = -27.;
        assert!(e.update_label(label.clone()));
        assert!(e.undo());
        assert_eq!(e.document(), &original);
        assert!(e.redo());
        assert_eq!(e.label(id), Some(&label));
        let layer = e.add_layer();
        let upper = e.add_label(label.clone()).unwrap();
        assert_eq!(e.label_at(label.position, 1.), Some((layer, upper)));
        e.toggle_layer(layer);
        assert!(!e.update_label(Label {
            id: upper,
            text: "Hidden".into(),
            ..label.clone()
        }));
        assert!(!e.delete_label(upper));
        assert_eq!(e.label_at(label.position, 1.), Some((1, id)));
        e.select_layer(1);
        e.commit_stroke(Stroke {
            creation_id: None,
            material: None,
            brush: Brush::Erase,
            points: vec![Point::new(0., 0.)].into(),
            size: 200.,
            strength: 1.,
            seed: 5,
        });
        assert!(e.label(id).is_some()); // text deletion is explicit
        assert!(e.delete_label(id));
        assert!(e.undo());
        assert!(e.label(id).is_some());
        let revision = e.revision();
        assert!(
            e.add_label(Label {
                size: f32::NAN,
                ..Label::default()
            })
            .is_none()
        );
        assert_eq!(e.revision(), revision);
        assert!(e.document().validate().is_ok());
    }
}
