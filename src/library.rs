//! Local atlas shelf. Snapshots preserve work independently of exported document files.
use crate::{
    document::{Document, Point},
    storage::{DocumentStore, LocalDocumentStore},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub center: Point,
    pub width: f32,
    pub height: f32,
}

impl Camera {
    pub fn for_document(document: &Document) -> Self {
        let bounds = document.content_bounds();
        Self {
            center: Point::new(
                (bounds.min.x + bounds.max.x) / 2.,
                (bounds.min.y + bounds.max.y) / 2.,
            ),
            width: bounds.width() / 0.78,
            height: bounds.height() / 0.78,
        }
    }

    fn valid(self) -> bool {
        [self.center.x, self.center.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite())
            && self.width > 0.
            && self.height > 0.
            && self.width <= 100_000_000.
            && self.height <= 100_000_000.
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AtlasRecord {
    pub id: u64,
    pub name: String,
    pub source: Option<PathBuf>,
    pub camera: Camera,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeChannel {
    Master,
    Music,
    Brush,
    Tools,
}

impl VolumeChannel {
    pub const ALL: [Self; 4] = [Self::Master, Self::Music, Self::Brush, Self::Tools];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioLevels {
    master: u8,
    music: u8,
    brush: u8,
    tools: u8,
}

impl Default for AudioLevels {
    fn default() -> Self {
        Self {
            master: 100,
            music: 25,
            brush: 100,
            tools: 100,
        }
    }
}

impl AudioLevels {
    pub fn level(self, channel: VolumeChannel) -> u8 {
        match channel {
            VolumeChannel::Master => self.master,
            VolumeChannel::Music => self.music,
            VolumeChannel::Brush => self.brush,
            VolumeChannel::Tools => self.tools,
        }
        .min(100)
    }

    pub fn set(&mut self, channel: VolumeChannel, level: u8) -> bool {
        let slot = match channel {
            VolumeChannel::Master => &mut self.master,
            VolumeChannel::Music => &mut self.music,
            VolumeChannel::Brush => &mut self.brush,
            VolumeChannel::Tools => &mut self.tools,
        };
        let level = level.min(100);
        let changed = *slot != level;
        *slot = level;
        changed
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Shelf {
    pub atlases: Vec<AtlasRecord>,
    pub reduced_motion: bool,
    // Retain the preference key used by the original Paper toolbar preview.
    #[serde(default)]
    pub paper_ui: bool,
    #[serde(default)]
    pub shader_effects_disabled: bool,
    #[serde(default)]
    pub sound_effects_disabled: bool,
    #[serde(default)]
    pub audio_levels: AudioLevels,
    #[serde(default)]
    pub disabled_landmark_packs: std::collections::HashSet<String>,
}

#[derive(Clone)]
pub struct Library {
    root: PathBuf,
    pub shelf: Shelf,
}

impl Library {
    /// Local snapshot/preview directory, also used by isolated verification.
    pub fn root_path(&self) -> &std::path::Path {
        &self.root
    }
    pub fn default_path() -> PathBuf {
        if let Some(path) = std::env::var_os("ELMAPA_LIBRARY_DIR") {
            return PathBuf::from(path);
        }
        let root = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Elmapa");
        // Keep shelves created by earlier desktop builds discoverable.
        #[cfg(not(target_os = "macos"))]
        if !root.join("shelf.json").exists()
            && let Some(home) = std::env::var_os("HOME")
        {
            let legacy = PathBuf::from(home).join("Library/Application Support/Elmapa");
            if legacy.join("shelf.json").is_file() {
                return legacy;
            }
        }
        root
    }

    pub fn load(root: PathBuf) -> Result<Self, String> {
        let path = root.join("shelf.json");
        let shelf: Shelf = match fs::File::open(&path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| format!("Could not read atlas shelf: {e}"))?;
                if bytes.len() > 1024 * 1024 {
                    return Err("Atlas shelf is too large.".into());
                }
                serde_json::from_slice(&bytes)
                    .map_err(|e| format!("Could not read atlas shelf: {e}"))?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Shelf::default(),
            Err(e) => return Err(format!("Could not read atlas shelf: {e}")),
        };
        if shelf.atlases.len() > 128
            || shelf.atlases.iter().any(|a| a.id == 0 || !a.camera.valid())
            || shelf
                .atlases
                .iter()
                .enumerate()
                .any(|(i, a)| shelf.atlases[..i].iter().any(|b| a.id == b.id))
        {
            return Err("Atlas shelf contains invalid records.".into());
        }
        Ok(Self { root, shelf })
    }

    /// Preview payloads are rebuildable and keyed by camera and snapshot file stamp.
    /// Reads/generation/writes run on the bounded shelf worker, never in paint.
    pub fn preview(&self, record: &AtlasRecord) -> Result<Vec<u8>, String> {
        self.preview_impl(record, None)
    }

    /// `scene` must correspond to the snapshot already durably stored for `record`.
    pub fn preview_with_scene(
        &self,
        record: &AtlasRecord,
        scene: &crate::scene::Scene,
    ) -> Result<Vec<u8>, String> {
        self.preview_impl(record, Some(scene))
    }

    fn preview_impl(
        &self,
        record: &AtlasRecord,
        scene: Option<&crate::scene::Scene>,
    ) -> Result<Vec<u8>, String> {
        let document_path = self.root.join(format!("{}.elmapa", record.id));
        let metadata = fs::metadata(&document_path).map_err(|e| e.to_string())?;
        let stamp = metadata
            .modified()
            .map_err(|e| e.to_string())?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        // Rebuild cached shelf art when shared terrain rendering changes.
        let mut header = b"ELMAPA-THUMB-5".to_vec();
        header.extend_from_slice(&crate::scene::landmark_catalog().fingerprint.to_le_bytes());
        header.extend_from_slice(b"relief-1");
        header.extend_from_slice(&stamp.to_le_bytes());
        header.extend_from_slice(&metadata.len().to_le_bytes());
        for value in [
            record.camera.center.x,
            record.camera.center.y,
            record.camera.width,
            record.camera.height,
        ] {
            header.extend_from_slice(&value.to_le_bytes());
        }
        let path = self.root.join(format!("{}.preview", record.id));
        let length =
            (crate::scene::thumbnail::WIDTH * crate::scene::thumbnail::HEIGHT * 4) as usize;
        if let Ok(file) = fs::File::open(&path) {
            let mut bytes = Vec::new();
            file.take((header.len() + length + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() == header.len() + length && bytes.starts_with(&header) {
                return Ok(bytes[header.len()..].to_vec());
            }
        }
        let pixels = if let Some(scene) = scene {
            crate::performance::counter("thumbnail_scene_reused", 1);
            crate::scene::thumbnail::thumbnail_from_scene(scene, record.camera)?
        } else {
            let document = self.read(record.id)?;
            crate::scene::thumbnail::thumbnail(&document, record.camera)?
        };
        static PREVIEW_SEQUENCE: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let sequence = PREVIEW_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temporary = self.root.join(format!(
            ".preview-{}-{}-{sequence}.tmp",
            record.id,
            std::process::id()
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        let result = file
            .write_all(&header)
            .and_then(|_| file.write_all(&pixels));
        drop(file);
        if let Err(error) = result.and_then(|_| fs::rename(&temporary, path)) {
            let _ = fs::remove_file(&temporary);
            return Err(error.to_string());
        }
        Ok(pixels)
    }

    pub fn read(&self, id: u64) -> Result<Document, String> {
        LocalDocumentStore.load(&self.root.join(format!("{id}.elmapa")))
    }

    pub fn next_id(&self) -> u64 {
        self.shelf
            .atlases
            .iter()
            .map(|a| a.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }

    pub fn checkpoint(&mut self, record: AtlasRecord, document: &Document) -> Result<(), String> {
        if record.id == 0 || !record.camera.valid() {
            return Err("Could not save atlas camera.".into());
        }
        let existing = self.shelf.atlases.iter().position(|a| a.id == record.id);
        if existing.is_none() && self.shelf.atlases.len() >= 128 {
            return Err("The shelf holds up to 128 atlases.".into());
        }
        fs::create_dir_all(&self.root)
            .map_err(|e| format!("Could not create atlas library: {e}"))?;
        LocalDocumentStore.save(document, &self.root.join(format!("{}.elmapa", record.id)))?;
        self.update_record(record)
    }

    pub fn update_record(&mut self, record: AtlasRecord) -> Result<(), String> {
        if record.id == 0 || !record.camera.valid() {
            return Err("Could not save atlas camera.".into());
        }
        let existing = self.shelf.atlases.iter().position(|a| a.id == record.id);
        if existing.is_none() && self.shelf.atlases.len() >= 128 {
            return Err("The shelf holds up to 128 atlases.".into());
        }
        let previous = self.shelf.atlases.clone();
        if let Some(i) = existing {
            self.shelf.atlases[i] = record;
        } else {
            self.shelf.atlases.push(record);
        }
        if let Err(error) = self.save_preferences() {
            self.shelf.atlases = previous;
            return Err(error);
        }
        Ok(())
    }

    pub fn save_preferences(&self) -> Result<(), String> {
        fs::create_dir_all(&self.root)
            .map_err(|e| format!("Could not create atlas library: {e}"))?;
        let bytes = serde_json::to_vec(&self.shelf).map_err(|e| e.to_string())?;
        let temporary = self.root.join(format!(".shelf-{}.tmp", std::process::id()));
        let write = || -> std::io::Result<()> {
            let mut file = fs::File::create(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()
        };
        write()
            .and_then(|_| fs::rename(&temporary, self.root.join("shelf.json")))
            .map_err(|e| format!("Could not save atlas shelf: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shelf_restores_work_camera_sources_and_preferences() {
        let root = std::env::temp_dir().join(format!(
            "elmapa-shelf-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut library = Library::load(root.clone()).unwrap();
        let mut document = Document::demo();
        let record = AtlasRecord {
            id: library.next_id(),
            name: document.name.clone(),
            source: Some(root.join("external.elmapa")),
            camera: Camera {
                center: Point::new(-230., 470.),
                width: 1200.,
                height: 800.,
            },
        };
        library.checkpoint(record.clone(), &document).unwrap();
        document.name = "A changed world".into();
        library
            .checkpoint(
                AtlasRecord {
                    name: document.name.clone(),
                    ..record.clone()
                },
                &document,
            )
            .unwrap();
        library.shelf.reduced_motion = true;
        library.shelf.paper_ui = true;
        library.shelf.shader_effects_disabled = true;
        library.shelf.sound_effects_disabled = true;
        library.shelf.audio_levels.set(VolumeChannel::Master, 77);
        library.shelf.audio_levels.set(VolumeChannel::Music, 0);
        library.shelf.audio_levels.set(VolumeChannel::Brush, 0);
        library.shelf.audio_levels.set(VolumeChannel::Tools, 41);
        library
            .shelf
            .disabled_landmark_packs
            .insert("elmapa.basic".into());
        library.save_preferences().unwrap();
        // Older shelves can still contain a global biome preference.
        let path = root.join("shelf.json");
        let mut shelf: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        shelf["biome_pack"] = "alpine".into();
        fs::write(&path, serde_json::to_vec(&shelf).unwrap()).unwrap();
        let restored = Library::load(root.clone()).unwrap();
        assert_eq!(restored.read(record.id).unwrap(), document);
        assert_eq!(restored.shelf.atlases.len(), 1);
        assert_eq!(
            restored.shelf.atlases[0].camera.center,
            record.camera.center
        );
        assert_eq!(restored.shelf.atlases[0].source, record.source);
        assert!(restored.shelf.reduced_motion);
        assert!(restored.shelf.shader_effects_disabled);
        assert!(restored.shelf.sound_effects_disabled);
        assert_eq!(restored.shelf.audio_levels, library.shelf.audio_levels);
        assert!(restored.shelf.paper_ui);
        assert!(
            restored
                .shelf
                .disabled_landmark_packs
                .contains("elmapa.basic")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn older_shelves_use_default_appearance_and_effects_preferences() {
        let shelf: Shelf =
            serde_json::from_str(r#"{"atlases":[],"dark":true,"reduced_motion":false}"#).unwrap();
        assert!(!shelf.shader_effects_disabled);
        assert!(!shelf.sound_effects_disabled);
        assert_eq!(shelf.audio_levels, AudioLevels::default());
        assert!(!shelf.paper_ui);
        assert!(serde_json::to_value(shelf).unwrap().get("dark").is_none());
    }

    #[test]
    fn older_audio_profiles_keep_their_mix_and_get_default_music_volume() {
        let levels: AudioLevels =
            serde_json::from_str(r#"{"master":70,"brush":40,"tools":65}"#).unwrap();
        assert_eq!(levels.level(VolumeChannel::Master), 70);
        assert_eq!(levels.level(VolumeChannel::Music), 25);
        assert_eq!(levels.level(VolumeChannel::Brush), 40);
        assert_eq!(levels.level(VolumeChannel::Tools), 65);
    }

    #[test]
    fn corrupt_shelf_is_reported_without_overwriting_it() {
        let root = std::env::temp_dir().join(format!("elmapa-bad-shelf-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("shelf.json"), b"broken").unwrap();
        assert!(Library::load(root.clone()).is_err());
        assert_eq!(fs::read(root.join("shelf.json")).unwrap(), b"broken");
        fs::remove_dir_all(root).unwrap();
    }
}
