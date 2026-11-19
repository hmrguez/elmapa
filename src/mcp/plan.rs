//! Translate coarse map plans into ordinary editable Elmapa layers.
mod area;

use area::Drawing;
use elmapa::{
    brush_materials,
    document::{
        Brush, ContentBounds, Document, Label, Landmark, LandmarkKind, Layer, Point, Stroke,
        TerrainReplay,
    },
    scene,
};
use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

const MAX_FEATURES: usize = 64;
const MAX_POINTS: usize = 128;
const COORDINATE_LIMIT: f32 = 32_768.;
type Position = [f32; 2];

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Create {
    name: String,
    /// Initial canvas size in world units. Defaults to [1600, 1000].
    pub size: Option<Position>,
    /// Controls reproducible brush variation. Defaults to 0.
    pub seed: Option<u32>,
    /// One layer per feature, in bottom-to-top order. Use unique names.
    pub features: Vec<Feature>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Feature {
    name: String,
    art: Artwork,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "kind", deny_unknown_fields)]
enum Artwork {
    /// A filled land, water or forest outline. The outline closes automatically.
    Area {
        brush: AreaBrush,
        /// Vertices in world units, with x right and y down.
        outline: Vec<Position>,
        /// Optional material ID. Discover IDs with inspect_map.
        material: Option<String>,
        /// Order in which the area is painted. Omit for the top-to-bottom scanline fill.
        drawing: Option<Drawing>,
    },
    /// A smoothed mountain ridge, river, trail, or other native brush stroke.
    Stroke {
        /// land, water, forest, mountain, river, or path (trail).
        brush: LineBrush,
        /// Coordinate pairs or names of earlier features to use as waypoints.
        through: Vec<Waypoint>,
        /// Brush size in world units. The renderer derives line width from this size.
        /// Rivers and trails default to the native editor’s initial brush size.
        size: Option<f32>,
        /// Optional material ID. Rivers accept Water finish IDs for channel color.
        /// Discover IDs with inspect_map.
        material: Option<String>,
    },
    Landmark {
        /// Asset ID, e.g. elmapa.basic:town, or short ID from the basic pack.
        asset: String,
        at: Position,
        size: Option<f32>,
    },
    Label {
        text: String,
        at: Position,
        size: Option<f32>,
        /// Clockwise rotation in degrees, from −180 to 180. Defaults to 0.
        rotation: Option<f32>,
    },
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum AreaBrush {
    Land,
    Water,
    Forest,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum LineBrush {
    Land,
    Water,
    Forest,
    Mountain,
    River,
    Path,
}

impl From<LineBrush> for Brush {
    fn from(brush: LineBrush) -> Self {
        match brush {
            LineBrush::Land => Self::Land,
            LineBrush::Water => Self::Water,
            LineBrush::Forest => Self::Forest,
            LineBrush::Mountain => Self::Mountain,
            LineBrush::River => Self::River,
            LineBrush::Path => Self::Path,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
enum Waypoint {
    Position(Position),
    Feature(String),
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Edit {
    pub map_id: String,
    pub expected_revision: String,
    /// Applied in order and saved together. Earlier edits are visible to later edits.
    pub changes: Vec<Change>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "op", deny_unknown_fields)]
pub(super) enum Change {
    /// Add on top, or replace the artwork in the specified layer in place.
    Put {
        layer_id: Option<u64>,
        feature: Feature,
    },
    /// Move the feature's anchor to a new world coordinate.
    Move {
        layer_id: u64,
        to: Position,
    },
    /// Rename a layer. If it contains a single label, change its visible text as well.
    Rename {
        layer_id: u64,
        name: String,
    },
    Remove {
        layer_id: u64,
    },
}

pub(super) fn create(plan: Create) -> Result<Document, String> {
    check_count(plan.features.len(), MAX_FEATURES, "features")?;
    let mut document = Document::blank();
    document.name = plan.name;
    if let Some([width, height]) = plan.size {
        document.width = width;
        document.height = height;
    }
    document.layers.clear();
    document.next_id = 1;
    for (index, feature) in plan.features.into_iter().enumerate() {
        put(
            &mut document,
            None,
            feature,
            u64::from(plan.seed.unwrap_or(0)),
        )
        .map_err(|error| format!("features[{index}]: {error}"))?;
    }
    document.validate()?;
    Ok(document)
}

pub(super) fn apply(document: &mut Document, changes: Vec<Change>) -> Result<(), String> {
    check_count(changes.len(), MAX_FEATURES, "changes")?;
    for (index, change) in changes.into_iter().enumerate() {
        apply_change(document, change).map_err(|error| format!("changes[{index}]: {error}"))?;
    }
    document.validate()
}

fn apply_change(document: &mut Document, change: Change) -> Result<(), String> {
    match change {
        Change::Put { layer_id, feature } => put(document, layer_id, feature, 0),
        Change::Move { layer_id, to } => {
            let to = point(to)?;
            let layer = layer_mut(document, layer_id)?;
            let from = anchor(layer).ok_or("This layer has no artwork to move.")?;
            let offset = |point: &mut Point| {
                point.x += to.x - from.x;
                point.y += to.y - from.y;
            };
            layer.strokes = layer
                .strokes
                .iter()
                .cloned()
                .map(|mut stroke| {
                    stroke.points.iter_mut().for_each(offset);
                    stroke
                })
                .collect::<Vec<_>>()
                .into();
            if let Some(TerrainReplay::BrushPaths { paths, .. }) = &mut layer.terrain_replay {
                for path in std::sync::Arc::make_mut(paths) {
                    path.iter_mut().for_each(offset);
                }
            }
            for item in std::sync::Arc::make_mut(&mut layer.relief_moves) {
                offset(&mut item.position);
            }
            for label in &mut layer.labels {
                offset(&mut label.position);
            }
            for landmark in &mut layer.landmarks {
                offset(&mut landmark.position);
            }
            Ok(())
        }
        Change::Rename { layer_id, name } => {
            check_name(document, Some(layer_id), &name)?;
            let layer = layer_mut(document, layer_id)?;
            if let [label] = layer.labels.as_mut_slice() {
                label.text.clone_from(&name);
            }
            layer.name = name;
            Ok(())
        }
        Change::Remove { layer_id } => {
            let index = layer_index(document, layer_id)?;
            document.layers.remove(index);
            Ok(())
        }
    }
}

fn layer_index(document: &Document, id: u64) -> Result<usize, String> {
    document
        .layers
        .iter()
        .position(|layer| layer.id == id)
        .ok_or_else(|| format!("Unknown layer ID {id}. Use inspect_map to find layer IDs."))
}

fn layer_mut(document: &mut Document, id: u64) -> Result<&mut Layer, String> {
    let index = layer_index(document, id)?;
    Ok(&mut document.layers[index])
}

fn check_name(document: &Document, old_id: Option<u64>, name: &str) -> Result<(), String> {
    if name.trim().is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err("Feature names must be nonempty, single-line, and at most 128 bytes.".into());
    }
    if document
        .layers
        .iter()
        .any(|layer| Some(layer.id) != old_id && layer.name == name)
    {
        return Err(format!("Feature name {name:?} is already in use."));
    }
    Ok(())
}

fn put(
    document: &mut Document,
    layer_id: Option<u64>,
    feature: Feature,
    seed: u64,
) -> Result<(), String> {
    check_name(document, layer_id, &feature.name)?;
    let index = layer_id.map(|id| layer_index(document, id)).transpose()?;
    let previous = index.map(|index| &document.layers[index]);
    let previous_label = previous
        .and_then(|layer| layer.labels.first())
        .map(|label| label.id);
    let previous_landmark = previous
        .and_then(|layer| layer.landmarks.first())
        .map(|landmark| landmark.id);
    let seed = previous
        .and_then(|layer| layer.strokes.iter().next())
        .map_or(seed ^ layer_id.unwrap_or(document.next_id), |stroke| {
            stroke.seed
        });
    let (id, visible, opacity) = match previous {
        Some(layer) => (layer.id, layer.visible, layer.opacity),
        None => (document.allocate_id()?, true, 1.),
    };
    let mut layer = Layer {
        id,
        name: feature.name,
        visible,
        opacity,
        strokes: Default::default(),
        labels: Vec::new(),
        landmarks: Vec::new(),
        relief_moves: Default::default(),
        terrain_replay: None,
        territories: None,
    };
    match feature.art {
        Artwork::Area {
            brush,
            outline,
            material,
            drawing,
        } => {
            let brush = match brush {
                AreaBrush::Land => Brush::Land,
                AreaBrush::Water => Brush::Water,
                AreaBrush::Forest => Brush::Forest,
            };
            let material = material_for(brush, material)?;
            let outline = outline
                .into_iter()
                .map(point)
                .collect::<Result<Vec<_>, _>>()?;
            let (mut strokes, terrain_replay) =
                area::fill(outline, drawing.unwrap_or_default(), brush, material, seed)?;
            layer.terrain_replay = terrain_replay;
            for stroke in &mut strokes {
                stroke.creation_id = Some(document.allocate_id()?);
            }
            layer.strokes = strokes.into();
        }
        Artwork::Stroke {
            brush,
            through,
            size,
            material,
        } => {
            use Brush::*;
            let brush = Brush::from(brush);
            check_count(through.len(), MAX_POINTS, "waypoints")?;
            let points = through
                .into_iter()
                .map(|waypoint| match waypoint {
                    Waypoint::Position(position) => point(position),
                    Waypoint::Feature(name) => document
                        .layers
                        .iter()
                        .find(|layer| layer.name == name)
                        .and_then(anchor)
                        .ok_or_else(|| format!("Unknown feature anchor {name:?}.")),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let (default_size, strength) = match brush {
                Land | Water => (96., 1.),
                Forest => (82., 1.),
                Mountain => (78., 1.),
                River | Path => (Brush::DEFAULT_SIZE, Brush::DEFAULT_STRENGTH),
                Erase => unreachable!("not exposed as a plan brush"),
            };
            let stroke = Stroke {
                creation_id: Some(document.allocate_id()?),
                brush,
                points: smooth(points, false).into(),
                size: size.unwrap_or(default_size),
                strength,
                seed,
                material: material_for(brush, material)?,
            };
            stroke.validate()?;
            layer.strokes.push(stroke);
        }
        Artwork::Label {
            text,
            at,
            size,
            rotation,
        } => {
            let label = Label {
                id: existing_or_next(previous_label, document)?,
                text,
                position: point(at)?,
                size: size.unwrap_or(32.),
                rotation: rotation.unwrap_or_default(),
                ..Label::default()
            };
            label.validate()?;
            layer.labels.push(label);
        }
        Artwork::Landmark { asset, at, size } => {
            let kind = LandmarkKind::from(asset.as_str());
            if !scene::landmark_registry()
                .iter()
                .any(|definition| definition.kind == kind)
            {
                return Err(format!(
                    "Unknown landmark asset {asset:?}. See inspect_map for assets."
                ));
            }
            let landmark = Landmark {
                id: existing_or_next(previous_landmark, document)?,
                kind,
                position: point(at)?,
                size: size.unwrap_or(92.),
            };
            landmark.validate()?;
            layer.landmarks.push(landmark);
        }
    }
    if let Some(index) = index {
        document.layers[index] = layer;
    } else {
        document.layers.push(layer);
    }
    Ok(())
}

fn existing_or_next(id: Option<u64>, document: &mut Document) -> Result<u64, String> {
    match id {
        Some(id) => Ok(id),
        None => document.allocate_id(),
    }
}

fn check_count(count: usize, limit: usize, items: &str) -> Result<(), String> {
    if count == 0 || count > limit {
        return Err(format!("Supply between 1 and {limit} {items}."));
    }
    Ok(())
}

fn point([x, y]: Position) -> Result<Point, String> {
    if x.abs() <= COORDINATE_LIMIT && y.abs() <= COORDINATE_LIMIT {
        Ok(Point::new(x, y))
    } else {
        Err(format!(
            "Coordinates must be within ±{COORDINATE_LIMIT} world units."
        ))
    }
}

pub(super) fn anchor(layer: &Layer) -> Option<Point> {
    layer
        .landmarks
        .first()
        .map(|landmark| landmark.position)
        .or_else(|| layer.labels.first().map(|label| label.position))
        .or_else(|| {
            layer
                .strokes
                .iter()
                .filter_map(|stroke| stroke.points.point_bounds())
                .reduce(ContentBounds::union)
                .map(center)
        })
}

pub(super) fn center(bounds: ContentBounds) -> Point {
    Point::new(
        (bounds.min.x + bounds.max.x) / 2.,
        (bounds.min.y + bounds.max.y) / 2.,
    )
}

fn material_for(
    brush: Brush,
    material: Option<String>,
) -> Result<Option<std::sync::Arc<str>>, String> {
    let finish_brush = if brush == Brush::River {
        Brush::Water
    } else {
        brush
    };
    if let Some(id) = &material
        && !brush_materials::for_brush(finish_brush).any(|(_, material)| material.id == *id)
    {
        return Err(format!(
            "Material {id:?} does not belong to the {} brush.",
            brush.label()
        ));
    }
    Ok(material.map(Into::into))
}

fn mix(a: Point, b: Point, t: f32) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// Cut corners twice while keeping the endpoints of routes pinned to their anchors.
fn smooth(mut points: Vec<Point>, closed: bool) -> Vec<Point> {
    if points.len() < 3 {
        return points;
    }
    for _ in 0..2 {
        let first = points[0];
        let last = points[points.len() - 1];
        let mut rounded = Vec::with_capacity(points.len() * 2);
        if closed {
            points.push(first);
        } else {
            rounded.push(first);
        }
        for edge in points.windows(2) {
            rounded.push(mix(edge[0], edge[1], 0.2));
            rounded.push(mix(edge[0], edge[1], 0.8));
        }
        if !closed {
            rounded.push(last);
        }
        points = rounded;
    }
    points
}
