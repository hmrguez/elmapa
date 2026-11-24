//! Toolbar and menu actions dispatched by the studio.
use super::{Action, Studio, Tool, ViewReplay};
use crate::app::{files, sound::Effect};
use elmapa::library::VolumeChannel;

impl Studio {
    fn set_tool(&mut self, tool: Tool) {
        if self.regions.active || self.regions_layer().is_some() {
            self.leave_regions();
        }
        if self.tool == tool {
            return;
        }
        self.cancel_text();
        self.cancel_landmark();
        self.clear_map_selection();
        self.tool = tool;
        self.advanced = false;
    }

    pub fn act(&mut self, action: Action, jobs: &mut files::Jobs) {
        if self.quit_requested || jobs.switching() && !matches!(action, Action::Quit) {
            return;
        }
        if action == Action::SaveReplayVideo || action == Action::Save && self.replay_view.is_some()
        {
            self.replay_video_requested |= self.replay_view.is_some();
            return;
        }
        if self.replay_view.is_some() {
            match action {
                Action::ReplayView => {
                    self.stop_view_replay();
                    return;
                }
                Action::Quit => {
                    self.stop_view_replay();
                }
                _ => return,
            }
        }
        if let Action::SetLayerBranch(branch, expanded) = action {
            self.layer_branches.insert(branch, expanded);
            self.ui_dirty = true;
            return;
        }
        let orbit_open = self.brush_orbit.anchor.is_some();
        if !matches!(action, Action::BrushOrbitEnter(..) | Action::BrushOrbitBack) {
            self.close_brush_orbit();
        }
        self.finish();
        if self.text.draft.is_some() {
            match action {
                Action::Undo | Action::Redo => self.cancel_label_draft(),
                Action::Brush(_)
                | Action::TextTool
                | Action::LandmarkTool
                | Action::SelectTool
                | Action::Save
                | Action::Export(_)
                | Action::AtlasStyle
                | Action::Open
                | Action::Home
                | Action::Quit
                | Action::Settings
                | Action::AddLayer
                | Action::SelectLayer(_)
                | Action::DeleteLayer
                | Action::Blank
                | Action::Demo
                | Action::Enter(_)
                | Action::ReplayView
                | Action::Regions(_)
                    if !self.finish_label_draft() =>
                {
                    return;
                }
                _ => {}
            }
        } else {
            self.commit_text_field();
        }
        if !matches!(action, Action::TextField(field) | Action::TextStep(field, _)
            if Some(field) == self.text.control)
        {
            self.dismiss_text_control();
        }
        self.viewport.resolve(self.bounds);
        let revision = self.editor.revision();
        match action {
            Action::Regions(action) => self.region_action(action),
            Action::SetLayerBranch(..) => unreachable!(),
            Action::Brush(b) => {
                self.set_tool(Tool::Brush);
                self.brush = b;
                if b == elmapa::document::Brush::Erase {
                    self.wet_release = None;
                }
            }
            Action::OpenBrushOrbit if self.advanced && self.can_open_brush_orbit() => {
                if !orbit_open {
                    self.open_brush_orbit(self.bounds.size / 2.);
                }
            }
            Action::BrushMaterial(index) if orbit_open => {
                self.brush_orbit.select(self.brush, index);
            }
            Action::BrushOrbitEnter(depth, index) if orbit_open => {
                self.brush_orbit.enter(self.brush, depth, index);
            }
            Action::BrushOrbitBack if orbit_open => self.brush_orbit.navigation.back(),
            Action::OpenBrushOrbit
            | Action::BrushMaterial(_)
            | Action::BrushOrbitEnter(..)
            | Action::BrushOrbitBack
            | Action::CloseBrushOrbit => {}
            Action::SelectTool => self.set_tool(Tool::Select),
            Action::PathNodeDensity(step) => self.refresh_path_nodes(step),
            Action::ClearSelection => {
                self.cancel_text();
                self.cancel_landmark();
                self.clear_map_selection();
            }
            Action::TextTool => self.set_tool(Tool::Text),
            Action::LandmarkTool => self.set_tool(Tool::Landmarks),
            Action::LandmarkEnter(_, _)
            | Action::LandmarkBack
            | Action::LandmarkKind(_)
            | Action::LandmarkSize(_)
            | Action::LandmarkDuplicate
            | Action::LandmarkDelete => self.landmark_action(action),
            Action::PlaceLabel
            | Action::TextStyle
            | Action::CloseTextStyle
            | Action::TextStyleDetails
            | Action::TextReuseStyle
            | Action::LandmarkName
            | Action::LandmarkNameAbove(_)
            | Action::LandmarkIcon
            | Action::TextField(_)
            | Action::TextFont(_)
            | Action::TextInk(_)
            | Action::TextStep(_, _)
            | Action::RadialEnter(_, _)
            | Action::RadialBack
            | Action::TextDuplicate
            | Action::TextDelete
            | Action::TextReset => self.text_action(action),
            Action::LandmarkPack(index) => {
                if let Some(pack) = elmapa::scene::landmark_catalog().packs.get(index) {
                    if !self.disabled_landmark_packs.remove(&pack.id) {
                        self.disabled_landmark_packs.insert(pack.id.clone());
                    }
                    self.landmarks.radial.reset();
                    jobs.preferences = true;
                }
            }
            Action::SetAppearance(appearance) => {
                if self.appearance != appearance {
                    self.appearance = appearance;
                    jobs.preferences = true;
                }
            }
            Action::Motion => {
                self.reduced = !self.reduced;
                self.wet_release = None;
                jobs.preferences = true;
            }
            Action::ShaderEffects => {
                self.shader_effects_disabled = !self.shader_effects_disabled;
                self.wet_release = None;
                jobs.preferences = true;
            }
            Action::SoundEffects => {
                self.sound_effects_disabled = !self.sound_effects_disabled;
                jobs.preferences = true;
            }
            Action::AudioSettings(open) if self.home && self.settings => {
                self.audio_settings = open;
                self.audio_focus = None;
            }
            Action::PreviewAudio(channel) if self.audio_settings_open() => {
                self.sound.preview = match channel {
                    VolumeChannel::Brush => Some(Effect::Brush(
                        self.brush,
                        self.brush_orbit.current_index(self.brush),
                    )),
                    VolumeChannel::Tools => Some(Effect::Landmark),
                    VolumeChannel::Master => None,
                };
            }
            Action::AudioSettings(_) | Action::AudioSlider(_) | Action::PreviewAudio(_) => {}
            Action::Settings => {
                self.audio_settings = false;
                self.audio_focus = None;
                if self.home {
                    self.settings = !self.settings;
                } else {
                    self.settings_on_home = true;
                    jobs.request(files::Request::Home);
                }
            }
            Action::CloseSettings => self.settings = false,
            Action::Advanced => self.advanced = !self.advanced,
            Action::Menu => self.menu = !self.menu,
            Action::ReplayView => {
                self.menu = false;
                self.wet_release = None;
                if !self.home {
                    if self.reduced {
                        self.status = "Replay follows Reduced Motion.".into();
                    } else {
                        self.status.clear();
                        self.pointer = None;
                        self.replay_view = Some(ViewReplay::Playing);
                    }
                }
            }
            Action::SaveReplayVideo => unreachable!("handled before editing the canvas"),
            Action::LayerTools => self.layer_tools = !self.layer_tools,
            Action::Radius(v) => self.radius = (self.radius + v).clamp(12., 360.),
            Action::Strength(v) => self.strength = (self.strength + v).clamp(0.15, 1.),
            Action::AddLayer => {
                self.clear_map_selection();
                self.regions.active = false;
                self.regions.clear();
                self.editor.add_layer();
            }
            Action::SelectLayer(id) => {
                self.clear_map_selection();
                self.text.selected = None;
                self.landmarks.selected = None;
                self.editor.select_layer(id);
                if self.regions_layer().is_some() {
                    self.activate_regions();
                } else {
                    self.leave_regions();
                }
            }
            Action::Visibility(id) => {
                self.editor.toggle_layer(id);
                self.regions.clear();
            }
            Action::DeleteLayer => {
                self.regions.clear();
                let layer = self.editor.active_layer();
                if self.editor.layer_has_children(layer) {
                    self.status = "Remove the child Regions layers first.".into();
                } else if !self.editor.remove_layer(layer) {
                    self.status = "Keep at least one layer in your world.".into();
                }
            }
            Action::MoveLayer(delta) => {
                self.regions.clear();
                self.editor.move_layer(self.editor.active_layer(), delta);
            }
            Action::Opacity(delta) => {
                if let Some(layer) = self.editor.document().layer(self.editor.active_layer()) {
                    self.editor.set_layer_opacity(
                        self.editor.active_layer(),
                        (layer.opacity + delta).clamp(0.1, 1.),
                    );
                }
            }
            Action::Undo => {
                self.clear_map_selection();
                self.regions.clear();
                self.landmarks.arrivals.clear();
                self.text.animate = None;
                self.text.drop = None;
                self.editor.undo();
                self.motion.reset();
                self.mountain_motion.reset();
                self.path_motion.reset();
                self.river_motion.reset();
                self.animate_new = false;
            }
            Action::Redo => {
                self.clear_map_selection();
                self.regions.clear();
                self.landmarks.arrivals.clear();
                self.text.animate = None;
                self.text.drop = None;
                self.editor.redo();
                self.motion.reset();
                self.mountain_motion.reset();
                self.path_motion.reset();
                self.river_motion.reset();
                self.animate_new = false;
            }
            Action::Zoom(f) => {
                self.viewport.zoom_at(self.bounds.center(), f, self.bounds);
                return;
            }
            Action::Fit => {
                self.viewport.reset(self.editor.document().content_bounds());
                return;
            }
            Action::Quit => {
                self.quit_requested = true;
                jobs.request(files::Request::Quit);
            }
            Action::Home => jobs.request(files::Request::Home),
            Action::Enter(id) => {
                if self.home && self.atlas == Some(id) {
                    self.home = false;
                    self.settings = false;
                    self.opening = (!self.reduced).then_some(-1.);
                    self.opening_ready = false;
                    self.opening_image = jobs.preview(id).cloned();
                } else {
                    jobs.request(files::Request::Enter(id));
                }
            }
            Action::Blank => jobs.request(files::Request::Create(false)),
            Action::Demo => jobs.request(files::Request::Create(true)),
            Action::Save => {
                self.menu = false;
                jobs.request(files::Request::Save(None));
            }
            Action::Open => {
                self.menu = false;
                jobs.request(files::Request::Open(None));
            }
            Action::AtlasStyle => {
                use elmapa::document::AtlasStyle;
                let style = match self.editor.document().style {
                    AtlasStyle::Classic => AtlasStyle::Woodland,
                    AtlasStyle::Woodland => AtlasStyle::Classic,
                };
                self.editor.set_atlas_style(style);
                self.menu = false;
            }
            Action::Export(format) => {
                self.menu = false;
                jobs.request(files::Request::Export(format, None));
            }
        }
        if let Some(id) = self.text.selected
            && !self
                .editor
                .document()
                .layers
                .iter()
                .any(|l| l.visible && l.opacity > 0. && l.labels.iter().any(|label| label.id == id))
        {
            self.text.selected = None;
        }
        if let Some(id) = self.landmarks.selected
            && !self
                .editor
                .document()
                .layers
                .iter()
                .any(|l| l.visible && l.opacity > 0. && l.landmarks.iter().any(|p| p.id == id))
        {
            self.cancel_landmark();
        }
        if self
            .landmarks
            .last_placed
            .is_some_and(|id| self.editor.landmark(id).is_none())
        {
            self.landmarks.last_placed = None;
        }
        self.retain_visible_selection();
        if self.editor.revision() != revision {
            self.changed();
        }
        self.ui_dirty = true;
    }
}
