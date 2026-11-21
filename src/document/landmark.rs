use super::{
    ContentBounds, Label, Point,
    limits::{MAX_LANDMARK_SIZE, MIN_LANDMARK_SIZE},
};
use serde::{Deserialize, Serialize};

/// Stable pack-qualified asset ID. Legacy unqualified IDs belong to the basic pack.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct LandmarkKind(String);
impl LandmarkKind {
    pub fn key(&self) -> &str {
        &self.0
    }
    pub fn valid(&self) -> bool {
        self.0.len() <= 193
            && self.0.split_once(':').is_some_and(|(pack, item)| {
                [pack, item].iter().all(|s| {
                    !s.is_empty()
                        && s.bytes()
                            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
                })
            })
    }
}
impl From<&str> for LandmarkKind {
    fn from(value: &str) -> Self {
        Self(if value.contains(':') {
            value.into()
        } else {
            format!("elmapa.basic:{value}")
        })
    }
}
impl Default for LandmarkKind {
    fn default() -> Self {
        Self::from("pin")
    }
}
impl<'de> Deserialize<'de> for LandmarkKind {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        let id = Self::from(value.as_str());
        if !id.valid() {
            return Err(serde::de::Error::custom("Invalid landmark asset ID"));
        }
        Ok(id)
    }
}
/// A ground-anchored illustration. Width is in world units; motion is never saved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Landmark {
    pub id: u64,
    pub kind: LandmarkKind,
    pub position: Point,
    pub size: f32,
    /// Attached lettering with a position relative to the landmark anchor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<Label>,
}
impl Default for Landmark {
    fn default() -> Self {
        Self {
            id: 0,
            kind: LandmarkKind::default(),
            position: Point::new(0., 0.),
            size: 144.,
            name: None,
        }
    }
}
impl Landmark {
    pub fn validate(&self) -> Result<(), String> {
        if !self.kind.valid()
            || !(MIN_LANDMARK_SIZE..=MAX_LANDMARK_SIZE).contains(&self.size)
            || !self.position.is_in_world()
        {
            return Err("A landmark has an invalid position or size.".into());
        }
        if let Some(label) = self.name_label() {
            label.validate()?;
        }
        Ok(())
    }

    pub fn name_label(&self) -> Option<Label> {
        let mut label = self.name.clone()?;
        label.id = self.id;
        label.position.x += self.position.x;
        label.position.y += self.position.y;
        Some(label)
    }

    /// Set the name using its map position. Empty lettering removes the name.
    pub fn set_name_label(&mut self, mut label: Label) {
        label.id = 0;
        label.position.x -= self.position.x;
        label.position.y -= self.position.y;
        self.name = (!label.text.trim().is_empty()).then_some(label);
    }

    pub fn bounds(&self) -> ContentBounds {
        let icon = crate::scene::landmark_bounds(self);
        self.name_label()
            .filter(|label| label.opacity > 0.)
            .map_or(icon, |label| icon.union(label.bounds()))
    }
    pub fn contains(&self, point: Point, tolerance: f32) -> bool {
        let b = crate::scene::landmark_bounds(self).padded(tolerance);
        (point.x >= b.min.x && point.x <= b.max.x && point.y >= b.min.y && point.y <= b.max.y)
            || self
                .name_label()
                .is_some_and(|label| label.opacity > 0. && label.contains(point, tolerance))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn landmark_ids_migrate_old_maps_and_preserve_missing_pack_references() {
        let legacy = r#"{"id":2,"kind":"town","position":{"x":0,"y":0},"size":144}"#;
        let mut landmark: Landmark = serde_json::from_str(legacy).unwrap();
        assert_eq!(landmark.kind.key(), "elmapa.basic:town");
        assert!(
            serde_json::to_string(&landmark)
                .unwrap()
                .contains("elmapa.basic:town")
        );
        landmark.kind = LandmarkKind::from("uninstalled.pack:observatory");
        let json = serde_json::to_string(&landmark).unwrap();
        assert_eq!(serde_json::from_str::<Landmark>(&json).unwrap(), landmark);
        assert!(landmark.validate().is_ok());
        assert!(landmark.bounds().width() > 0.);
        assert!(serde_json::from_str::<LandmarkKind>(r#""../bad:thing""#).is_err());
    }
}

#[cfg(test)]
mod names {
    use super::*;
    use crate::{
        editor::Editor,
        scene::{SceneCache, to_svg},
    };

    #[test]
    fn names_round_trip_and_render_at_the_moved_landmark() {
        let mut editor = Editor::blank();
        let mut landmark = Landmark {
            position: Point::new(160., 80.),
            ..Landmark::default()
        };
        landmark.set_name_label(Label {
            text: "Harbour".into(),
            position: Point::new(168., 104.),
            ..Label::default()
        });
        let id = editor.add_landmark(landmark).unwrap();
        let mut landmark = editor.landmark(id).unwrap().clone();
        landmark.position = Point::new(200., 100.);
        assert!(editor.update_landmark(landmark));
        let json = serde_json::to_string(editor.document()).unwrap();
        let document = serde_json::from_str::<crate::document::Document>(&json).unwrap();
        assert!(document.validate().is_ok());
        let scene = SceneCache::default().render(&document, None);
        let named = scene
            .batches()
            .find_map(|batch| scene.label(batch).map(|label| (batch, label)))
            .unwrap();
        assert!(!named.0.is_empty());
        assert_eq!(named.1.text, "Harbour");
        assert_eq!(named.1.position, Point::new(208., 124.));
        assert!(document.content_bounds().intersects(named.1.bounds()));
        let mut unnamed = document.clone();
        unnamed.layers[0].landmarks[0].name = None;
        assert_ne!(to_svg(&document), to_svg(&unnamed));
    }
}
