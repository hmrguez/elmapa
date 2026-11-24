//! Route shortcuts after text input has had a chance to consume the key.
use crate::app::{
    files::{ExportFormat, Jobs},
    lettering::Field,
    replay::Replay,
    state::{Action, Studio, Tool},
};
use bevy::prelude::*;

pub(super) fn key_down(
    key: KeyCode,
    command: bool,
    shift: bool,
    studio: &mut Studio,
    jobs: &mut Jobs,
    replay: &mut Replay,
) {
    if studio.replay_view.is_some() && !(command && key == KeyCode::KeyQ) {
        match key {
            KeyCode::Escape => {
                studio.stop_view_replay();
            }
            KeyCode::KeyS if command => studio.act(Action::SaveReplayVideo, jobs),
            KeyCode::KeyR if command && shift => studio.act(Action::ReplayView, jobs),
            _ => {}
        }
        return;
    }
    if key == KeyCode::KeyP && !matches!(studio.tool, Tool::Text | Tool::Select) {
        replay.start();
    }
    if command && key == KeyCode::KeyQ {
        studio.act(Action::Quit, jobs);
        return;
    }
    if command && key == KeyCode::Comma {
        studio.act(Action::Settings, jobs);
        return;
    }
    if command && shift && key == KeyCode::KeyH {
        studio.act(Action::Home, jobs);
        return;
    }
    if studio.home {
        if !command && super::audio::keyboard(key, shift, studio, jobs) {
            return;
        }
        match key {
            KeyCode::Escape => {
                studio.settings = false;
                studio.ui_dirty = true;
            }
            KeyCode::KeyO if command && !studio.settings => studio.act(Action::Open, jobs),
            KeyCode::KeyN if command && !studio.settings => studio.act(Action::Blank, jobs),
            KeyCode::Enter if !studio.settings => {
                if let Some(id) = studio
                    .library
                    .as_ref()
                    .and_then(|l| l.shelf.atlases.first().map(|a| a.id))
                {
                    studio.act(Action::Enter(id), jobs);
                }
            }
            _ => {}
        }
        return;
    }
    if key == KeyCode::Escape {
        if studio.brush_orbit.anchor.is_some() && !studio.brush_orbit.navigation.path.is_empty() {
            studio.act(Action::BrushOrbitBack, jobs);
            return;
        }
        if studio.close_brush_orbit() {
            return;
        }
        if studio.landmarks.selected.is_some() && !studio.landmarks.radial.path.is_empty() {
            studio.act(Action::LandmarkBack, jobs);
            return;
        }
        if studio.text.style_open {
            studio.act(Action::CloseTextStyle, jobs);
            return;
        }
        if matches!(studio.tool, Tool::Landmarks | Tool::Text) {
            studio.act(Action::SelectTool, jobs);
            return;
        }
        if studio.cancel_selection_gesture()
            || studio.cancel_path_drag()
            || studio.cancel_region_step()
        {
            return;
        }
        studio.cancel_canvas_interaction();
        jobs.cancel_export();
        return;
    }
    let action = if command {
        match key {
            KeyCode::KeyD if studio.landmarks.selected.is_some() => Some(Action::LandmarkDuplicate),
            KeyCode::KeyZ => Some(if shift { Action::Redo } else { Action::Undo }),
            KeyCode::KeyS => Some(Action::Save),
            KeyCode::KeyO => Some(Action::Open),
            KeyCode::KeyE => Some(Action::Export(ExportFormat::Svg)),
            KeyCode::KeyR if shift => Some(Action::ReplayView),
            KeyCode::Digit0 => Some(Action::Fit),
            _ => None,
        }
    } else {
        use elmapa::document::Brush;
        match key {
            KeyCode::Digit1 => Some(Action::Brush(Brush::Land)),
            KeyCode::Digit2 => Some(Action::Brush(Brush::Water)),
            KeyCode::Digit3 => Some(Action::Brush(Brush::Forest)),
            KeyCode::Digit4 => Some(Action::Brush(Brush::Mountain)),
            KeyCode::Digit5 => Some(Action::Brush(Brush::River)),
            KeyCode::Digit6 => Some(Action::Brush(Brush::Path)),
            KeyCode::Digit7 => Some(Action::Brush(Brush::Erase)),
            KeyCode::KeyL | KeyCode::Digit0 => Some(Action::LandmarkTool),
            KeyCode::Delete | KeyCode::Backspace if studio.landmarks.selected.is_some() => {
                Some(Action::LandmarkDelete)
            }
            KeyCode::BracketLeft
                if studio.tool == Tool::Landmarks || studio.landmarks.selected.is_some() =>
            {
                Some(Action::LandmarkSize(-16.))
            }
            KeyCode::BracketRight
                if studio.tool == Tool::Landmarks || studio.landmarks.selected.is_some() =>
            {
                Some(Action::LandmarkSize(16.))
            }
            KeyCode::KeyV | KeyCode::Digit9 => Some(Action::SelectTool),
            KeyCode::KeyT | KeyCode::Digit8 => Some(Action::TextTool),
            KeyCode::BracketLeft if studio.tool == Tool::Text => {
                Some(Action::TextStep(Field::Size, -4.))
            }
            KeyCode::BracketRight if studio.tool == Tool::Text => {
                Some(Action::TextStep(Field::Size, 4.))
            }
            KeyCode::BracketLeft => Some(Action::Radius(-15.)),
            KeyCode::BracketRight => Some(Action::Radius(15.)),
            _ => None,
        }
    };
    if let Some(action) = action {
        studio.act(action, jobs);
    }
}
