use super::plan;
use elmapa::{
    document::{ContentBounds, Document, Layer, Point},
    library::Camera,
    scene::thumbnail,
    storage::{DocumentStore, LocalDocumentStore, export_svg},
};
use image::{ImageEncoder, codecs::png::PngEncoder};
use serde_json::{Value, json};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

#[derive(Clone)]
pub(super) struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: &Path) -> Result<Self, String> {
        fs::create_dir_all(root)
            .map_err(|error| format!("Create atlas output directory: {error}"))?;
        let root = root.canonicalize().map_err(|error| error.to_string())?;
        root.to_str()
            .ok_or("The output directory needs a UTF-8 path.")?;
        Ok(Self { root })
    }

    pub fn create(&self, request: plan::Create) -> Result<Value, String> {
        let document = plan::create(request)?;
        let id = format!("{:032x}", rand::random::<u128>());
        let path = self.path(&id)?;
        let _lock = self.lock(&id)?;
        if path.try_exists().map_err(|error| error.to_string())? {
            return Err("A map with this generated ID already exists. Retry creation.".into());
        }
        let result = write_info(&id, &path, &document)?;
        LocalDocumentStore.save(&document, &path)?;
        Ok(result)
    }

    pub fn edit(&self, request: plan::Edit) -> Result<Value, String> {
        let id = request.map_id;
        let path = self.existing_path(&id)?;
        let _lock = self.lock(&id)?;
        let mut document = LocalDocumentStore.load(&path)?;
        let current_revision = revision(&document)?;
        if current_revision != request.expected_revision {
            return Err(format!(
                "Revision conflict. Current revision is {current_revision}. Inspect the map before retrying."
            ));
        }
        plan::apply(&mut document, request.changes)?;
        let result = write_info(&id, &path, &document)?;
        LocalDocumentStore.save(&document, &path)?;
        Ok(result)
    }

    pub fn inspect(&self, id: &str, layer: Option<u64>, offset: usize) -> Result<Value, String> {
        const PAGE_SIZE: usize = 20;
        let path = self.existing_path(id)?;
        let document = LocalDocumentStore.load(&path)?;
        let mut result = map_info(id, &path, &document)?;
        result["bounds"] = bounds_json(document.content_bounds());
        if let Some(layer) = layer {
            let layer = document
                .layer(layer)
                .ok_or("Unknown layer ID. Inspect the map to list layers.")?;
            result["layer"] = layer_info(layer);
            result["layer"]["bounds"] = json!(layer.content_bounds().map(bounds_json));
        } else {
            result["layers"] = json!(
                document
                    .layers
                    .iter()
                    .skip(offset)
                    .take(PAGE_SIZE)
                    .map(layer_info)
                    .collect::<Vec<_>>()
            );
            let next = offset.saturating_add(PAGE_SIZE);
            if next < document.layers.len() {
                result["next_offset"] = json!(next);
            }
        }
        Ok(result)
    }

    pub fn preview(&self, id: &str, layer: Option<u64>) -> Result<(Value, Vec<u8>), String> {
        let path = self.existing_path(id)?;
        let document = LocalDocumentStore.load(&path)?;
        let bounds = match layer {
            Some(layer_id) => document
                .layer(layer_id)
                .ok_or("Unknown layer ID. Inspect the map to list layers.")?
                .content_bounds()
                .ok_or("The requested layer has no visible artwork.")?,
            None => document.content_bounds(),
        };
        let ratio = thumbnail::WIDTH as f32 / thumbnail::HEIGHT as f32;
        let bounds = bounds.padded(28.);
        let width = bounds.width().max(bounds.height() * ratio);
        let height = width / ratio;
        let center = plan::center(bounds);
        let camera = Camera {
            center,
            width,
            height,
        };
        let mut pixels = thumbnail::thumbnail(&document, camera)?;
        // Shelf thumbnails are BGRA for Bevy. PNG expects RGBA.
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(
                &pixels,
                thumbnail::WIDTH,
                thumbnail::HEIGHT,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|error| error.to_string())?;
        let mut summary = map_info(id, &path, &document)?;
        summary["viewport"] = bounds_json(ContentBounds {
            min: Point::new(center.x - width / 2., center.y - height / 2.),
            max: Point::new(center.x + width / 2., center.y + height / 2.),
        });
        Ok((summary, png))
    }

    pub fn export(&self, id: &str) -> Result<Value, String> {
        let path = self.existing_path(id)?;
        let document = LocalDocumentStore.load(&path)?;
        let mut result = map_info(id, &path, &document)?;
        let revision = result["revision"].as_str().expect("revision is a string");
        let export = self.root.join(format!("{id}-{revision}.svg"));
        export_svg(&export, &document, None)?;
        result["svg_path"] = json!(export);
        Ok(result)
    }

    fn path(&self, id: &str) -> Result<PathBuf, String> {
        if id.len() == 32
            && id
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            Ok(self.root.join(format!("{id}.elmapa")))
        } else {
            Err("Invalid map ID. Use the ID returned by create_map.".into())
        }
    }

    fn existing_path(&self, id: &str) -> Result<PathBuf, String> {
        let path = self.path(id)?;
        match path.symlink_metadata() {
            Ok(metadata) if metadata.is_file() => Ok(path),
            Ok(_) => Err("The map path is not a regular file.".into()),
            Err(error) => Err(format!("Could not open map {id}: {error}")),
        }
    }

    fn lock(&self, id: &str) -> Result<File, String> {
        let lock = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.root.join(format!(".{id}.lock")))
            .map_err(|error| error.to_string())?;
        lock.lock().map_err(|error| error.to_string())?;
        Ok(lock)
    }
}

fn revision(document: &Document) -> Result<String, String> {
    let mut hash = blake3::Hasher::new();
    serde_json::to_writer(&mut hash, document).map_err(|error| error.to_string())?;
    Ok(hash.finalize().to_hex().to_string())
}

fn map_info(id: &str, path: &Path, document: &Document) -> Result<Value, String> {
    Ok(json!({
        "map_id": id,
        "name": document.name,
        "revision": revision(document)?,
        "atlas_path": path,
        "layer_count": document.layers.len()
    }))
}

fn bounds_json(bounds: ContentBounds) -> Value {
    json!([[bounds.min.x, bounds.min.y], [bounds.max.x, bounds.max.y]])
}

fn write_info(id: &str, path: &Path, document: &Document) -> Result<Value, String> {
    let mut result = map_info(id, path, document)?;
    result["layer_ids"] = json!(
        document
            .layers
            .iter()
            .map(|layer| (&layer.name, layer.id))
            .collect::<Vec<_>>()
    );
    Ok(result)
}

fn layer_info(layer: &Layer) -> Value {
    let anchor = plan::anchor(layer).map(|point| [point.x, point.y]);
    let mut value = json!({
        "id": layer.id,
        "name": layer.name,
        "anchor": anchor,
    });
    let brushes = layer.brushes();
    if !brushes.is_empty() {
        value["brushes"] = json!(brushes);
    }
    if let [label] = layer.labels.as_slice() {
        value["label"] = json!(label.text);
    }
    if let [landmark] = layer.landmarks.as_slice() {
        value["asset"] = json!(landmark.kind);
    }
    value
}
