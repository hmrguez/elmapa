//! Ordered material and landmark selections for painting a biome.
use crate::{
    brush_materials::{self, Material},
    document::{Brush, LandmarkKind},
    scene::{LandmarkCatalog, landmark_catalog},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::OnceLock};

const MANIFEST: &str = include_str!("../assets/biome-packs/manifest.json");
const MATERIAL_BRUSHES: [Brush; 4] = [Brush::Land, Brush::Water, Brush::Forest, Brush::Mountain];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    packs: Vec<BiomePack>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BiomePack {
    pub id: String,
    pub name: String,
    pub description: String,
    pub materials: BiomeMaterials,
    pub landmarks: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BiomeMaterials {
    pub land: Vec<String>,
    pub water: Vec<String>,
    pub forest: Vec<String>,
    pub mountain: Vec<String>,
}

impl BiomeMaterials {
    pub fn for_brush(&self, brush: Brush) -> &[String] {
        match brush {
            Brush::Land => &self.land,
            Brush::Water => &self.water,
            Brush::Forest => &self.forest,
            Brush::Mountain => &self.mountain,
            _ => &[],
        }
    }
}

pub fn catalog() -> &'static [BiomePack] {
    static PACKS: OnceLock<Vec<BiomePack>> = OnceLock::new();
    PACKS.get_or_init(|| validate_manifest(MANIFEST).expect("valid bundled biome packs"))
}

pub fn find(id: &str) -> Option<usize> {
    catalog().iter().position(|pack| pack.id == id)
}

/// The first finish is the pack default. All indices refer to the global material catalog.
pub fn materials(
    pack: Option<usize>,
    brush: Brush,
) -> impl Iterator<Item = (usize, &'static Material)> {
    let selected = pack.and_then(|index| catalog().get(index));
    let brush = if selected.is_some() && brush == Brush::River {
        Brush::Water
    } else {
        brush
    };
    selected
        .into_iter()
        .flat_map(move |pack| pack.materials.for_brush(brush))
        .filter_map(move |id| {
            brush_materials::for_brush(brush).find(|(_, material)| material.id == *id)
        })
        .chain(brush_materials::for_brush(brush).filter(move |_| selected.is_none()))
}

/// Preserve the pack order while applying the library's landmark pack preferences.
pub fn landmark_items<'a>(
    pack: usize,
    landmarks: &'a LandmarkCatalog,
    disabled: &'a HashSet<String>,
) -> impl Iterator<Item = usize> + 'a {
    catalog()
        .get(pack)
        .into_iter()
        .flat_map(|pack| &pack.landmarks)
        .filter_map(move |id| landmarks.find(&LandmarkKind::from(id.as_str())))
        .filter(move |&index| !disabled.contains(&landmarks.packs[landmarks.items[index].pack].id))
}

fn validate_manifest(json: &str) -> Result<Vec<BiomePack>, String> {
    let manifest: Manifest = serde_json::from_str(json).map_err(|error| error.to_string())?;
    if manifest.schema_version != 1 || manifest.packs.is_empty() {
        return Err("Invalid biome pack manifest".into());
    }

    let mut pack_ids = HashSet::new();
    let landmarks = landmark_catalog();
    for pack in &manifest.packs {
        if !brush_materials::valid_id(&pack.id) || !pack_ids.insert(&pack.id) {
            return Err(format!("Invalid or duplicate biome pack ID: {}", pack.id));
        }
        if pack.name.trim().is_empty() || pack.description.trim().is_empty() {
            return Err(format!(
                "Missing biome pack name or description: {}",
                pack.id
            ));
        }

        for brush in MATERIAL_BRUSHES {
            let ids = pack.materials.for_brush(brush);
            if ids.is_empty() {
                return Err(format!("{} needs a {brush:?} default", pack.id));
            }
            let mut seen = HashSet::new();
            for id in ids {
                if !seen.insert(id)
                    || !brush_materials::for_brush(brush).any(|(_, material)| material.id == *id)
                {
                    return Err(format!(
                        "{} has an invalid or duplicate {brush:?} material: {id}",
                        pack.id
                    ));
                }
            }
        }

        let mut seen = HashSet::new();
        for id in &pack.landmarks {
            let kind = LandmarkKind::from(id.as_str());
            if kind.key() != id || !seen.insert(id) || landmarks.find(&kind).is_none() {
                return Err(format!(
                    "{} has an invalid or duplicate landmark: {id}",
                    pack.id
                ));
            }
        }
    }
    Ok(manifest.packs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_order_sets_defaults_without_changing_the_global_catalog() {
        let desert = find("desert").unwrap();
        let alpine = find("alpine").unwrap();
        let ids = |pack, brush| {
            materials(pack, brush)
                .map(|(_, material)| material.id.as_str())
                .collect::<Vec<_>>()
        };
        for brush in MATERIAL_BRUSHES {
            let all = ids(None, brush);
            assert_eq!(all, ids(Some(usize::MAX), brush));
            assert_ne!(ids(Some(desert), brush)[0], ids(Some(alpine), brush)[0]);
            assert!(ids(Some(desert), brush).iter().all(|id| all.contains(id)));
            assert!(ids(Some(alpine), brush).iter().all(|id| all.contains(id)));
        }
        assert_eq!(ids(Some(desert), Brush::Land), ["dune-sand", "ochre-sands"]);
        assert_eq!(ids(Some(alpine), Brush::Water), ["glacial-water", "lake"]);
        assert_eq!(
            ids(Some(alpine), Brush::River),
            ids(Some(alpine), Brush::Water)
        );
        assert!(ids(None, Brush::River).is_empty());
        assert!(ids(Some(alpine), Brush::Erase).is_empty());
    }

    #[test]
    fn landmark_choices_preserve_order_and_honor_disabled_packs() {
        let alpine = find("alpine").unwrap();
        let landmarks = landmark_catalog();
        let keys = |disabled: &HashSet<String>| {
            landmark_items(alpine, landmarks, disabled)
                .map(|index| landmarks.items[index].kind.key())
                .collect::<Vec<_>>()
        };
        assert_eq!(keys(&HashSet::new()), catalog()[alpine].landmarks);
        let disabled = HashSet::from(["elmapa.nature".to_owned()]);
        assert_eq!(keys(&disabled), ["brillanto.brilliant:observatory"]);
    }

    #[test]
    fn manifest_rejects_invalid_and_duplicate_references() {
        use serde_json::{Value, json};

        let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
        let bad_fields = [
            ("/packs/0/id", json!("Desert")),
            ("/packs/0/id", json!("alpine")),
            ("/packs/0/materials/land", json!([])),
            ("/packs/0/materials/land", json!(["missing-material"])),
            ("/packs/0/materials/land", json!(["oasis-water"])),
            ("/packs/0/materials/land", json!(["dune-sand", "dune-sand"])),
            (
                "/packs/0/landmarks",
                json!(["elmapa.nature:missing-landmark"]),
            ),
            (
                "/packs/0/landmarks",
                json!(["elmapa.nature:oasis", "elmapa.nature:oasis"]),
            ),
        ];
        for (path, value) in bad_fields {
            let mut invalid = manifest.clone();
            *invalid.pointer_mut(path).unwrap() = value;
            assert!(validate_manifest(&invalid.to_string()).is_err(), "{path}");
        }
    }
}
