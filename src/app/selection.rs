//! Dispatch canvas presses using the same active tool shown in the toolbar.
mod paths;
use super::state::{Studio, Tool};
use elmapa::document::Point;
pub(super) use paths::PathSelection;

impl Studio {
    pub(super) fn annotation_press(&mut self, point: Point, shift: bool) -> bool {
        if std::mem::take(&mut self.text.style_open) {
            self.ui_dirty = true;
            return true;
        }
        match self.tool {
            Tool::Brush => return false,
            Tool::Landmarks => self.place_landmark(point, shift),
            Tool::Text => self.place_text(point),
            Tool::Select => {
                if self.finish_label_draft() {
                    self.commit_text_field();
                    self.dismiss_text_control();
                    self.select_annotation(point);
                }
            }
        }
        self.ui_dirty = true;
        true
    }

    fn select_annotation(&mut self, point: Point) {
        if self.begin_path_drag(point) || self.begin_text_transform(point) {
            return;
        }
        let landmark = self.editor.landmark_at(point, 4. / self.viewport.zoom);
        let label = if landmark.is_some() && self.landmarks.selected.is_some() {
            None
        } else {
            self.editor.label_at(point, 6. / self.viewport.zoom)
        };
        if label.is_some() || landmark.is_some() {
            self.clear_path_selection();
        }
        if let Some((layer, id)) = label {
            self.landmarks.selected = None;
            self.select_label(point, layer, id);
        } else if let Some((layer, id)) = landmark {
            self.text.selected = None;
            self.select_landmark(point, layer, id);
        } else {
            self.landmarks.selected = None;
            self.text.selected = None;
            self.text.radial.reset();
            self.text.control = None;
            self.text.error = None;
            self.select_path(point);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elmapa::document::{Label, Landmark};

    #[test]
    fn selecting_either_annotation_keeps_select_active() {
        let mut studio = Studio::default();
        let point = Point::new(0., 0.);
        let label = studio.editor.add_label(Label::default()).unwrap();
        let landmark = studio.editor.add_landmark(Landmark::default()).unwrap();

        studio.tool = Tool::Select;
        assert!(studio.annotation_press(point, false));
        assert_eq!(studio.text.selected, Some(label));
        assert_eq!(studio.tool, Tool::Select);
        studio.finish();

        studio.editor.delete_label(label);
        assert!(studio.annotation_press(point, false));
        assert_eq!(studio.landmarks.selected, Some(landmark));
        assert_eq!(studio.text.selected, None);
        assert_eq!(studio.tool, Tool::Select);
    }
}
