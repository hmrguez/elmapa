//! Validated JSON/SVG packs. Loaded once; picker preferences never affect rendering.
use super::{
    Shape,
    landmarks::{LandmarkDefinition, LandmarkStep, parse_artwork},
};
use crate::document::{ContentBounds, LandmarkKind, Point};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Component, Path},
    sync::{Arc, OnceLock},
};

// The build script discovers bundled packs and assets without a Rust asset list.
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/landmark-packs.rs"));
}

#[derive(Debug)]
pub struct LandmarkPack {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_key: String,
    pub initial: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    icon: Option<String>,
    items: Vec<Item>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    svg: String,
    icon: Option<String>,
    width: f32,
    anchor: [f32; 2],
    parts: Vec<LandmarkStep>,
}
pub(super) struct Artwork {
    pub parts: Vec<Arc<Vec<Shape>>>,
    pub bounds: ContentBounds,
}
#[derive(Default)]
pub struct LandmarkCatalog {
    pub packs: Vec<LandmarkPack>,
    pub items: Vec<LandmarkDefinition>,
    pub errors: Vec<String>,
    pub fingerprint: u64,
    indices: HashMap<LandmarkKind, usize>,
    pub(super) artwork: Vec<Artwork>,
}
fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 96
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}
fn read_svg(path: &str, read: &impl Fn(&str) -> Result<String, String>) -> Result<String, String> {
    if Path::new(path).extension().is_none_or(|e| e != "svg")
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(format!("Invalid relative SVG path: {path}"));
    }
    let svg = read(path)?;
    if svg.len() > 2 * 1024 * 1024 {
        return Err(format!("SVG is too large: {path}"));
    }
    // Packs are self-contained vector artwork; never resolve files, URLs or fonts.
    let lower = svg.to_ascii_lowercase();
    if [
        "href",
        "<image",
        "<text",
        "<filter",
        "<mask",
        "<clippath",
        "<lineargradient",
        "<radialgradient",
        "<!entity",
        "url(",
    ]
    .iter()
    .any(|s| lower.contains(s))
    {
        return Err(format!(
            "SVG must use self-contained solid vector paths: {path}"
        ));
    }
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default())
        .map_err(|e| format!("Invalid SVG {path}: {e}"))?;
    if tree.root().abs_bounding_box().width() <= 0. || tree.root().abs_bounding_box().height() <= 0.
    {
        return Err(format!("Empty SVG: {path}"));
    }
    Ok(svg)
}
impl LandmarkCatalog {
    /// A pack is accepted atomically: invalid IDs, assets or animation reject it entirely.
    pub fn add_pack(
        &mut self,
        json: &str,
        read: impl Fn(&str) -> Result<String, String>,
    ) -> Result<(), String> {
        if json.len() > 1024 * 1024 {
            return Err("Pack manifest is too large".into());
        }
        let manifest: Manifest =
            serde_json::from_str(json).map_err(|e| format!("Invalid pack manifest: {e}"))?;
        if manifest.schema_version != 1
            || !valid_id(&manifest.id)
            || manifest.name.trim().is_empty()
            || manifest.name.len() > 160
            || manifest.description.len() > 1024
            || manifest.items.is_empty()
            || manifest.items.len() > 256
            || self.packs.len() >= 64
            || self.packs.iter().any(|p| p.id == manifest.id)
        {
            return Err("Invalid or duplicate landmark pack".into());
        }
        let icon = manifest
            .icon
            .as_deref()
            .map(|p| read_svg(p, &read))
            .transpose()?;
        let mut items = Vec::new();
        let mut artwork = Vec::new();
        let mut ids = HashSet::new();
        for item in manifest.items {
            if !valid_id(&item.id)
                || !ids.insert(item.id.clone())
                || item.name.trim().is_empty()
                || item.name.len() > 160
                || item.description.len() > 1024
                || !item.width.is_finite()
                || !(1. ..=4096.).contains(&item.width)
                || item
                    .anchor
                    .iter()
                    .any(|x| !x.is_finite() || x.abs() > 4096.)
                || item.parts.is_empty()
                || item.parts.len() > 128
            {
                return Err(format!("Invalid landmark item: {}", item.id));
            }
            let mut parts = HashSet::new();
            if item.parts.iter().any(|p| {
                p.id.is_empty()
                    || !parts.insert(&p.id)
                    || !p.delay.is_finite()
                    || !p.duration.is_finite()
                    || p.delay < 0.
                    || p.duration <= 0.
                    || p.delay + p.duration >= 1.
                    || p.pivot.iter().any(|x| !x.is_finite() || x.abs() > 4096.)
            }) {
                return Err(format!("Invalid animation: {}", item.id));
            }
            let svg = read_svg(&item.svg, &read)?;
            let icon = item
                .icon
                .as_deref()
                .map(|p| read_svg(p, &read))
                .transpose()?
                .unwrap_or_else(|| svg.clone());
            let geometry = parse_artwork(&svg, &item.parts)?;
            let kind = LandmarkKind::from(format!("{}:{}", manifest.id, item.id).as_str());
            items.push(LandmarkDefinition {
                kind,
                name: item.name,
                description: item.description,
                parts: item.parts,
                svg,
                icon,
                pack: self.packs.len(),
                width: item.width,
                anchor: Point::new(item.anchor[0], item.anchor[1]),
            });
            artwork.push(geometry);
        }
        // Rebuild shelf previews when installed artwork or definitions change.
        for byte in json.bytes().chain(
            items
                .iter()
                .flat_map(|d| d.svg.bytes().chain(d.icon.bytes())),
        ) {
            self.fingerprint = (self.fingerprint ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
        self.packs.push(LandmarkPack {
            icon_key: format!("landmark-pack/{}", manifest.id),
            initial: manifest
                .name
                .trim()
                .chars()
                .next()
                .unwrap()
                .to_uppercase()
                .collect(),
            id: manifest.id,
            name: manifest.name,
            description: manifest.description,
            icon,
        });
        for (offset, item) in items.iter().enumerate() {
            self.indices
                .insert(item.kind.clone(), self.items.len() + offset);
        }
        self.items.extend(items);
        self.artwork.extend(artwork);
        Ok(())
    }
    pub fn find(&self, id: &LandmarkKind) -> Option<usize> {
        self.indices.get(id).copied()
    }
    pub(super) fn ordinal(&self, id: &LandmarkKind) -> usize {
        // Missing packs preserve their document IDs and display the bundled pin.
        self.find(id).unwrap_or_else(|| {
            self.find(&LandmarkKind::default())
                .expect("bundled fallback pin")
        })
    }
    pub fn enabled_items(&self, disabled: &HashSet<String>) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, d)| !disabled.contains(&self.packs[d.pack].id))
            .map(|(i, _)| i)
            .collect()
    }
    pub fn load_installed(&mut self, root: &Path) {
        let entries = match fs::read_dir(root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) => {
                self.errors
                    .push(format!("Could not read landmark packs: {e}"));
                return;
            }
        };
        let mut paths: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        paths.sort();
        for directory in paths.into_iter().take(64) {
            let result = (|| {
                let root = directory.canonicalize().map_err(|e| e.to_string())?;
                let read = |path: &str| {
                    let path = root.join(path).canonicalize().map_err(|e| e.to_string())?;
                    if !path.starts_with(&root) {
                        return Err("Asset escapes its pack folder".into());
                    }
                    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 2 * 1024 * 1024 {
                        return Err("Pack file is too large".into());
                    }
                    fs::read_to_string(path).map_err(|e| e.to_string())
                };
                self.add_pack(&read("manifest.json")?, read)
            })();
            if let Err(e) = result {
                self.errors.push(format!(
                    "{}: {e}",
                    directory.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
        }
    }
}
pub fn landmark_catalog() -> &'static LandmarkCatalog {
    static CATALOG: OnceLock<LandmarkCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut catalog = LandmarkCatalog::default();
        let files = embedded::EMBEDDED_LANDMARK_FILES;
        for (path, json) in files.iter().filter(|(p, _)| p.ends_with("/manifest.json")) {
            let directory = Path::new(path).parent().unwrap();
            catalog
                .add_pack(json, |p| {
                    let name = directory.join(p);
                    files
                        .iter()
                        .find(|(p, _)| Path::new(p) == name)
                        .map(|(_, s)| s.to_string())
                        .ok_or_else(|| format!("Missing bundled SVG: {p}"))
                })
                .expect("valid bundled landmark pack");
        }
        assert!(
            !catalog.items.is_empty(),
            "at least one bundled landmark is required"
        );
        catalog.load_installed(&crate::library::Library::default_path().join("landmark-packs"));
        catalog
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><g id="body"><path fill="#554430" d="M2 2h20v20H2z"/></g></svg>"##;
    pub(crate) fn fixture(id: &str, name: &str) -> String {
        serde_json::json!({"schema_version":1,"id":id,"name":name,"items":[{
            "id":"observatory","name":"Observatory","svg":"observatory.svg","width":24,"anchor":[12,22],
            "parts":[{"id":"body","motion":"rise","delay":0.05,"duration":0.3,"pivot":[12,22]}]
        }]}).to_string()
    }
    #[test]
    fn landmark_pack_accepts_new_ids_icons_and_geometry_without_a_code_list() {
        let mut catalog = LandmarkCatalog::default();
        catalog
            .add_pack(&fixture("astral", "Astral Landmarks"), |_| Ok(SVG.into()))
            .unwrap();
        let pack = &catalog.packs[0];
        assert_eq!(pack.initial, "A");
        assert!(pack.icon.is_none());
        assert_eq!(catalog.items[0].kind.key(), "astral:observatory");
        assert_eq!(catalog.items[0].icon, SVG);
        assert_eq!(catalog.items[0].anchor, Point::new(12., 22.));
        assert_eq!(catalog.artwork[0].parts.len(), 1);
        assert_eq!(catalog.artwork[0].bounds.min, Point::new(2., 2.));
        let mut explicit: serde_json::Value =
            serde_json::from_str(&fixture("explicit", "Custom")).unwrap();
        explicit["icon"] = "pack.svg".into();
        explicit["items"][0]["icon"] = "small.svg".into();
        catalog
            .add_pack(&explicit.to_string(), |_| Ok(SVG.into()))
            .unwrap();
        assert_eq!(catalog.packs[1].icon.as_deref(), Some(SVG));
        let disabled = HashSet::from(["astral".into()]);
        assert_eq!(catalog.enabled_items(&disabled), vec![1]);
        // Disabling is a picker preference; the asset stays available to document rendering.
        assert_eq!(
            catalog.find(&LandmarkKind::from("astral:observatory")),
            Some(0)
        );
    }
    #[test]
    fn landmark_pack_rejects_bad_assets_timing_ids_and_drawing_order_atomically() {
        let mut catalog = LandmarkCatalog::default();
        let original: serde_json::Value =
            serde_json::from_str(&fixture("astral", "Astral")).unwrap();
        for (field, value) in [("svg", "../escape.svg"), ("id", "bad:id")] {
            let mut bad = original.clone();
            bad["items"][0][field] = value.into();
            assert!(
                catalog
                    .add_pack(&bad.to_string(), |_| Ok(SVG.into()))
                    .is_err()
            );
        }
        for (field, value) in [("duration", 0.), ("duration", -0.1), ("delay", 1.)] {
            let mut bad = original.clone();
            bad["items"][0]["parts"][0][field] = value.into();
            assert!(
                catalog
                    .add_pack(&bad.to_string(), |_| Ok(SVG.into()))
                    .is_err()
            );
        }
        assert!(
            catalog
                .add_pack(
                    &original.to_string(),
                    |_| Ok(SVG.replace("body", "missing"))
                )
                .is_err()
        );
        assert!(
            catalog
                .add_pack(&original.to_string(), |_| Ok(
                    SVG.replace("#554430", "url(external.svg)")
                ))
                .is_err()
        );
        assert!(
            catalog
                .add_pack(&original.to_string(), |_| Err("Missing SVG".into()))
                .is_err()
        );
        assert!(catalog.packs.is_empty() && catalog.items.is_empty() && catalog.artwork.is_empty());
        catalog
            .add_pack(&original.to_string(), |_| Ok(SVG.into()))
            .unwrap();
        assert!(
            catalog
                .add_pack(&original.to_string(), |_| Ok(SVG.into()))
                .is_err()
        );
        assert_eq!(catalog.items.len(), 1);
    }
    #[test]
    fn landmark_pack_discovers_folders_and_reports_broken_packs_without_losing_good_ones() {
        let root = std::env::temp_dir().join(format!("elmapa-pack-test-{}", std::process::id()));
        let good = root.join("good");
        let bad = root.join("bad");
        fs::create_dir_all(&good).unwrap();
        fs::create_dir_all(&bad).unwrap();
        fs::write(good.join("manifest.json"), fixture("astral", "Astral")).unwrap();
        fs::write(good.join("observatory.svg"), SVG).unwrap();
        fs::write(bad.join("manifest.json"), "{}").unwrap();
        let mut catalog = LandmarkCatalog::default();
        catalog.load_installed(&root);
        assert_eq!(catalog.items.len(), 1);
        assert_eq!(catalog.errors.len(), 1);
        assert!(catalog.errors[0].starts_with("bad:"));
        fs::remove_dir_all(root).unwrap();
    }
}
