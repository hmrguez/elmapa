use super::{DOCUMENT_VERSION, SharedChunks, SharedVec, limits::MAX_WORLD_COORDINATE};
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn is_in_world(self) -> bool {
        self.x.abs() <= MAX_WORLD_COORDINATE && self.y.abs() <= MAX_WORLD_COORDINATE
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Brush {
    Land,
    Water,
    Forest,
    Mountain,
    River,
    Path,
    Erase,
}

impl Brush {
    pub const DEFAULT_SIZE: f32 = 110.;
    pub const DEFAULT_STRENGTH: f32 = 0.85;

    pub const ALL: [Self; 7] = [
        Self::Land,
        Self::Water,
        Self::Forest,
        Self::Mountain,
        Self::River,
        Self::Path,
        Self::Erase,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Land => "Land",
            Self::Water => "Water",
            Self::Forest => "Forest",
            Self::Mountain => "Mountains",
            Self::River => "River",
            Self::Path => "Trail",
            Self::Erase => "Eraser",
        }
    }

    pub const fn shortcut(self) -> &'static str {
        match self {
            Self::Land => "L",
            Self::Water => "W",
            Self::Forest => "F",
            Self::Mountain => "M",
            Self::River => "R",
            Self::Path => "P",
            Self::Erase => "E",
        }
    }
}

/// Layer, stroke position, and variation seed identify a rendered river or trail.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LinearStrokeId {
    pub layer: u64,
    pub index: usize,
    pub seed: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    /// Shares the document's creation order with labels and landmarks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creation_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<std::sync::Arc<str>>,
    pub brush: Brush,
    pub points: SharedVec<Point>,
    pub size: f32,
    pub strength: f32,
    /// Stored variation seed: reopening a map never reshuffles its terrain.
    pub seed: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainReplay {
    /// Spread a continuous wash from the first stroke through the painted area.
    BreadthFirst,
    /// Sweep one fixed-radius brush along the supplied gestures.
    BrushPaths {
        radius: f32,
        paths: std::sync::Arc<Vec<SharedVec<Point>>>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub strokes: SharedChunks<Stroke>,
    #[serde(default, skip_serializing_if = "relief_is_empty")]
    pub relief_moves: std::sync::Arc<Vec<super::ReliefMove>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terrain_replay: Option<TerrainReplay>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<super::Label>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub landmarks: Vec<super::Landmark>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub territories: Option<std::sync::Arc<super::RegionLayer>>,
}

fn relief_is_empty(moves: &[super::ReliefMove]) -> bool {
    moves.is_empty()
}

impl Layer {
    pub fn brushes(&self) -> Vec<Brush> {
        let mask = self.strokes.brush_mask();
        Brush::ALL
            .into_iter()
            .filter(|brush| mask & (1 << *brush as u8) != 0)
            .collect()
    }
}

/// Controls the map's artwork independently of the editor's UI appearance.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AtlasStyle {
    #[default]
    Classic,
    Woodland,
}

impl AtlasStyle {
    pub fn is_classic(&self) -> bool {
        *self == Self::Classic
    }

    pub fn paper(self) -> u32 {
        match self {
            Self::Classic => 0xe6e5d3,
            Self::Woodland => 0xe1e6da,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub version: u32,
    #[serde(default, skip_serializing_if = "AtlasStyle::is_classic")]
    pub style: AtlasStyle,
    pub name: String,
    pub width: f32,
    pub height: f32,
    /// Bottom to top paint order.
    pub layers: SharedVec<Layer>,
    pub next_id: u64,
}

impl Default for Document {
    fn default() -> Self {
        Self::blank()
    }
}

impl Document {
    pub fn blank() -> Self {
        Self {
            version: DOCUMENT_VERSION,
            style: AtlasStyle::Woodland,
            name: "Untitled atlas".into(),
            width: 1600.0,
            height: 1000.0,
            layers: vec![Layer {
                id: 1,
                name: "Terrain".into(),
                visible: true,
                opacity: 1.0,
                strokes: Vec::new().into(),
                labels: Vec::new(),
                landmarks: Vec::new(),
                relief_moves: Default::default(),
                terrain_replay: None,
                territories: None,
            }]
            .into(),
            next_id: 2,
        }
    }

    /// Allocate an ID in the namespace shared by layers, strokes, labels and landmarks.
    pub fn allocate_id(&mut self) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or("No free document IDs.")?;
        Ok(id)
    }

    pub fn layer(&self, id: u64) -> Option<&Layer> {
        self.layers.iter().find(|layer| layer.id == id)
    }

    pub fn layer_mut(&mut self, id: u64) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|layer| layer.id == id)
    }
}
