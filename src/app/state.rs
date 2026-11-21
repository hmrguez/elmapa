//! Studio state, document installation and scene invalidation.
mod actions;
mod canvas;
#[cfg(test)]
mod tests;

use super::{
    scene_work::SceneWork,
    viewport::{Bounds, Viewport},
};
use bevy::prelude::*;
use elmapa::{
    biome_packs,
    document::{Brush, Document, Point, RegionParent, Stroke},
    editor::Editor,
    library::Library,
    scene::{ForestAnimation, MountainAnimation, PathAnimation, RiverAnimation},
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LayerBranch {
    Layer(u64),
    Region(RegionParent),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Brush(Brush),
    BrushMaterial(usize),
    AdvancedBrushMaterial(usize),
    BrushOrbitPage(usize),
    CloseBrushOrbit,
    TextTool,
    PlaceLabel,
    TextStyle,
    CloseTextStyle,
    TextStyleDetails,
    TextReuseStyle,
    Regions(super::regions::RegionAction),
    LandmarkTool,
    LandmarkKind(usize),
    LandmarkEnter(usize, usize),
    LandmarkBack,
    LandmarkName,
    LandmarkNameAbove(bool),
    LandmarkIcon,
    LandmarkPack(usize),
    LandmarkSize(f32),
    LandmarkDuplicate,
    LandmarkDelete,
    SelectTool,
    PathNodeDensity(i32),
    ClearSelection,
    TextField(super::lettering::Field),
    TextFont(elmapa::document::LabelFont),
    TextInk(elmapa::document::LabelInk),
    TextStep(super::lettering::Field, f32),
    RadialEnter(usize, usize),
    RadialBack,
    TextDuplicate,
    TextDelete,
    TextReset,
    Theme,
    Advanced,
    Menu,
    BiomePack(Option<usize>),
    ReplayView,
    SaveReplayVideo,
    LayerTools,
    SetLayerBranch(LayerBranch, bool),
    Radius(f32),
    Strength(f32),
    AddLayer,
    SelectLayer(u64),
    Visibility(u64),
    DeleteLayer,
    MoveLayer(isize),
    Opacity(f32),
    Undo,
    Redo,
    Save,
    Open,
    Export,
    AtlasStyle,
    Zoom(f32),
    Fit,
    Blank,
    Demo,
    Home,
    Enter(u64),
    Settings,
    Motion,
    ShaderEffects,
    Quit,
    CloseSettings,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Brush,
    Select,
    Text,
    Landmarks,
}

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ShaderMode {
    Off = 0,
    Still = 1,
    Animated = 2,
}

#[derive(Resource)]
pub struct Studio {
    pub editor: Editor,
    pub tool: Tool,
    pub text: super::lettering::TextTool,
    pub landmarks: super::landmarks::LandmarkTool,
    pub regions: super::regions::RegionTool,
    pub selection: super::selection::MapSelection,
    pub path_selection: Option<super::selection::PathSelection>,
    pub library: Option<Library>,
    pub path: Option<PathBuf>,
    pub atlas: Option<u64>,
    pub brush: Brush,
    pub brush_orbit: super::brush_orbit::BrushOrbit,
    pub biome_pack: Option<usize>,
    pub dark: bool,
    pub reduced: bool,
    pub shader_effects_disabled: bool,
    pub disabled_landmark_packs: HashSet<String>,
    pub home: bool,
    pub installing: bool,
    pub settings: bool,
    pub settings_on_home: bool,
    pub advanced: bool,
    pub menu: bool,
    pub replay_view: Option<ViewReplay>,
    pub replay_video_requested: bool,
    pub layer_tools: bool,
    pub layer_branches: HashMap<LayerBranch, bool>,
    pub radius: f32,
    pub strength: f32,
    pub bounds: Bounds,
    pub viewport: Viewport,
    pub stroke: Option<Stroke>,
    pub(super) wet_release: Option<(u64, u64)>,
    pub drag: Option<Vec2>,
    pub pointer: Option<Vec2>,
    pub scene: SceneWork,
    pub motion: ForestAnimation,
    pub mountain_motion: MountainAnimation,
    pub path_motion: PathAnimation,
    pub river_motion: RiverAnimation,
    pub ui_dirty: bool,
    pub dirty: bool,
    pub status: String,
    pub opening: Option<f32>,
    pub opening_ready: bool,
    pub opening_from: Rect,
    pub opening_image: Option<Handle<Image>>,
    pub animate_new: bool,
    pub quit_requested: bool,
}
impl Default for Studio {
    fn default() -> Self {
        let editor = Editor::blank();
        let viewport = Viewport::new(editor.document().content_bounds());
        Self {
            editor,
            tool: Tool::Brush,
            text: super::lettering::TextTool::default(),
            landmarks: super::landmarks::LandmarkTool::default(),
            regions: default(),
            selection: Default::default(),
            path_selection: None,
            viewport,
            library: None,
            path: None,
            atlas: None,
            brush: Brush::Land,
            brush_orbit: default(),
            biome_pack: None,
            dark: std::env::args().any(|a| a == "--dark"),
            reduced: false,
            shader_effects_disabled: false,
            disabled_landmark_packs: HashSet::new(),
            home: true,
            installing: false,
            settings: false,
            settings_on_home: false,
            advanced: false,
            menu: false,
            replay_view: None,
            replay_video_requested: false,
            layer_tools: false,
            layer_branches: HashMap::new(),
            radius: Brush::DEFAULT_SIZE,
            strength: Brush::DEFAULT_STRENGTH,
            bounds: Bounds {
                size: Vec2::new(1440., 940.),
            },
            stroke: None,
            wet_release: None,
            drag: None,
            pointer: None,
            scene: SceneWork::default(),
            motion: ForestAnimation::default(),
            mountain_motion: MountainAnimation::default(),
            path_motion: PathAnimation::default(),
            river_motion: RiverAnimation::default(),
            ui_dirty: true,
            dirty: false,
            status: "Opening your atlases…".into(),
            opening: None,
            opening_ready: false,
            opening_from: Rect::from_corners(Vec2::new(74., 152.), Vec2::new(484., 384.)),
            opening_image: None,
            animate_new: false,
            quit_requested: false,
        }
    }
}
impl Studio {
    pub fn shader_mode(&self) -> ShaderMode {
        if self.shader_effects_disabled {
            ShaderMode::Off
        } else if self.reduced {
            ShaderMode::Still
        } else {
            ShaderMode::Animated
        }
    }

    pub fn shader_motion(&self) -> bool {
        self.shader_mode() == ShaderMode::Animated
    }

    pub fn set_biome_pack(&mut self, pack: Option<usize>) -> bool {
        let pack = pack.filter(|&index| biome_packs::catalog().get(index).is_some());
        if self.biome_pack == pack {
            return false;
        }
        self.biome_pack = pack;
        self.brush_orbit.reset_picker();
        self.reset_landmark_picker();
        self.ui_dirty = true;
        true
    }

    pub fn map_point(&self, point: Vec2) -> Point {
        self.viewport.transform(self.bounds).map_point(point)
    }
    pub(super) fn invalidate(&mut self) {
        self.scene.invalidate();
    }
    pub(super) fn invalidate_preview(&mut self) {
        self.scene.invalidate_preview();
    }
    pub fn changed(&mut self) {
        self.stop_view_replay();
        self.wet_release = None;
        self.dirty = true;
        self.ui_dirty = true;
        self.invalidate_preview();
    }
    pub(super) fn cancel_canvas_interaction(&mut self) {
        self.cancel_text();
        self.cancel_landmark();
        self.clear_map_selection();
        self.regions.clear();
        self.stroke = None;
        self.wet_release = None;
        self.drag = None;
        self.motion.reset();
        self.mountain_motion.reset();
        self.river_motion.reset();
        self.path_motion.reset();
        self.animate_new = false;
        self.invalidate_preview();
        self.menu = false;
        self.advanced = false;
        self.layer_tools = false;
        self.settings = false;
        self.ui_dirty = true;
    }

    pub fn install_document(
        &mut self,
        document: Document,
        id: u64,
        path: Option<PathBuf>,
        camera: Option<elmapa::library::Camera>,
        now: f32,
    ) {
        self.stop_view_replay();
        self.close_brush_orbit();
        self.tool = Tool::Brush;
        self.text = super::lettering::TextTool::default();
        self.landmarks = super::landmarks::LandmarkTool::default();
        self.reset_landmark_picker();
        self.regions = default();
        self.path_selection = None;
        self.selection = Default::default();
        if self.atlas != Some(id) {
            self.layer_branches.clear();
        }
        self.editor = Editor::new(document).expect("validated document");
        self.path = path;
        self.atlas = Some(id);
        self.viewport = camera.map_or_else(
            || Viewport::new(self.editor.document().content_bounds()),
            |c| Viewport::restore(c, self.bounds),
        );
        self.home = false;
        self.settings = false;
        self.menu = false;
        self.stroke = None;
        self.wet_release = None;
        self.drag = None;
        self.invalidate_preview();
        self.installing = true;
        self.dirty = false;
        self.ui_dirty = true;
        self.motion.reset();
        self.mountain_motion.reset();
        self.path_motion.reset();
        self.river_motion.reset();
        self.opening = (!self.reduced).then_some(now);
        self.opening_ready = false;
        self.animate_new = false;
    }

    pub fn stop_view_replay(&mut self) -> bool {
        let was_open = self.replay_view.take().is_some();
        self.replay_video_requested = false;
        self.ui_dirty |= was_open;
        was_open
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ViewReplay {
    Playing,
    Finished,
}
