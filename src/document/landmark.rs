use super::{
    ContentBounds, Point,
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
}
impl Default for Landmark {
    fn default() -> Self {
        Self {
            id: 0,
            kind: LandmarkKind::default(),
            position: Point::new(0., 0.),
            size: 144.,
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
        Ok(())
    }
    pub fn bounds(&self) -> ContentBounds {
        crate::scene::landmark_bounds(self)
    }
    pub fn contains(&self, point: Point, tolerance: f32) -> bool {
        let b = self.bounds().padded(tolerance);
        point.x >= b.min.x && point.x <= b.max.x && point.y >= b.min.y && point.y <= b.max.y
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
