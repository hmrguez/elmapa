//! Identical deterministic native workload inputs for GPUI/Bevy comparisons.
use super::{
    files::{Jobs, Request},
    render::Canvas,
    state::{Action, Appearance, Studio},
};
use bevy::{
    ecs::system::SystemParam,
    input::{ButtonState, mouse::MouseButtonInput},
    prelude::*,
    window::{CursorMoved, WindowEvent},
};
use serde::Deserialize;
use std::{collections::VecDeque, path::PathBuf};
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum Step {
    Ready,
    Frames {
        count: u32,
    },
    Idle {
        milliseconds: u64,
    },
    Enter {
        id: u64,
    },
    Home,
    Create,
    Brush {
        brush: elmapa::document::Brush,
    },
    Down {
        x: f32,
        y: f32,
    },
    Move {
        x: f32,
        y: f32,
    },
    Hover {
        x: f32,
        y: f32,
    },
    Up {
        x: f32,
        y: f32,
    },
    Click {
        x: f32,
        y: f32,
    },
    Landmarks,
    Landmark {
        kind: elmapa::document::LandmarkKind,
    },
    Select,
    Appearance {
        appearance: Appearance,
    },
    Pan,
    Zoom {
        factor: f32,
    },
    Fit,
    ReplayView,
    Undo,
    Redo,
    Layer,
    Visibility,
    Opacity {
        delta: f32,
    },
    Save,
    Open,
    Export,
    ReducedMotion {
        enabled: bool,
    },
    Mark {
        name: String,
    },
    Quit,
    Screenshot {
        path: PathBuf,
    },
}
#[derive(Resource)]
pub struct Replay {
    steps: VecDeque<Step>,
    frames: u32,
    paused: bool,
    idle_until: f32,
    loaded: bool,
    pointer_button: MouseButton,
}
impl Default for Replay {
    fn default() -> Self {
        let mut replay = Self {
            steps: VecDeque::new(),
            frames: 0,
            paused: false,
            idle_until: 0.,
            loaded: false,
            pointer_button: MouseButton::Left,
        };
        if let Some(path) = std::env::var_os("ELMAPA_REPLAY") {
            assert!(
                std::env::var_os("ELMAPA_LIBRARY_DIR").is_some(),
                "Replay requires an isolated library"
            );
            let bytes = std::fs::read(path).expect("replay file");
            assert!(bytes.len() <= 1024 * 1024);
            replay.steps = serde_json::from_slice(&bytes).expect("valid replay");
            assert!(replay.steps.len() <= 10000);
            replay.paused = std::env::var_os("ELMAPA_REPLAY_WAIT_FOR_START").is_some();
            replay.loaded = true;
        }
        replay
    }
}
impl Replay {
    pub fn start(&mut self) {
        self.paused = false;
    }
}
#[derive(SystemParam)]
pub struct ReplayInput<'w, 's> {
    commands: Commands<'w, 's>,
    window: Single<'w, 's, (Entity, &'static mut Window)>,
    events: MessageWriter<'w, WindowEvent>,
}
pub fn advance(
    mut replay: ResMut<Replay>,
    mut studio: ResMut<Studio>,
    mut jobs: ResMut<Jobs>,
    canvas: Res<Canvas>,
    time: Res<Time<Real>>,
    input: ReplayInput,
) {
    let ReplayInput {
        mut commands,
        mut window,
        mut events,
    } = input;
    if !replay.loaded
        || replay.paused
        || replay.steps.is_empty()
        || time.elapsed_secs() < replay.idle_until
    {
        return;
    }
    if replay.frames > 0 {
        replay.frames -= 1;
        return;
    }
    if studio.library.is_none()
        || matches!(replay.steps.front(), Some(Step::Ready))
            && (jobs.busy()
                || studio.opening.is_some()
                || !studio.home && (studio.scene.is_pending() || !canvas.ready()))
    {
        return;
    }
    let step = replay.steps.pop_front().expect("step");
    let action = match step {
        Step::Ready => None,
        Step::Frames { count } => {
            replay.frames = count;
            None
        }
        Step::Idle { milliseconds } => {
            replay.idle_until = time.elapsed_secs() + milliseconds.min(60000) as f32 / 1000.;
            None
        }
        Step::Enter { id } => Some(Action::Enter(id)),
        Step::Home => Some(Action::Home),
        Step::Create => Some(Action::Blank),
        Step::Brush { brush } => Some(Action::Brush(brush)),
        Step::Landmarks => Some(Action::LandmarkTool),
        Step::Landmark { kind } => elmapa::scene::landmark_catalog()
            .find(&kind)
            .map(Action::LandmarkKind),
        Step::Select => Some(Action::SelectTool),
        Step::Appearance { appearance } => Some(Action::SetAppearance(appearance)),
        Step::Pan => {
            // Legacy workload files select which button subsequent drags inject.
            // Panning itself still begins/ends with middle-button down/up events.
            replay.pointer_button = if replay.pointer_button == MouseButton::Left {
                MouseButton::Middle
            } else {
                MouseButton::Left
            };
            None
        }
        Step::Zoom { factor } => Some(Action::Zoom(factor)),
        Step::Fit => Some(Action::Fit),
        Step::ReplayView => Some(Action::ReplayView),
        Step::Undo => Some(Action::Undo),
        Step::Redo => Some(Action::Redo),
        Step::Layer => Some(Action::AddLayer),
        Step::Visibility => Some(Action::Visibility(studio.editor.active_layer())),
        Step::Opacity { delta } => Some(Action::Opacity(delta)),
        Step::Save => {
            jobs.request(Request::Save(Some(path("native-save.elmapa"))));
            None
        }
        Step::Open => {
            jobs.request(Request::Open(Some(path("native-save.elmapa"))));
            None
        }
        Step::Export => {
            jobs.request(Request::Export(Some(path("native-export.svg"))));
            None
        }
        Step::ReducedMotion { enabled } => {
            studio.reduced = enabled;
            jobs.preferences = true;
            None
        }
        Step::Mark { name } => {
            elmapa::performance::mark(&name);
            None
        }
        Step::Quit => {
            elmapa::performance::mark("replay_complete");
            Some(Action::Quit)
        }
        Step::Screenshot { path } => {
            commands
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(path));
            None
        }
        event => {
            let (x, y) = match event {
                Step::Down { x, y }
                | Step::Move { x, y }
                | Step::Hover { x, y }
                | Step::Up { x, y }
                | Step::Click { x, y } => (x, y),
                _ => unreachable!(),
            };
            assert!((0.0..=1.).contains(&x) && (0.0..=1.).contains(&y));
            let point = Vec2::new(x, y) * studio.bounds.size;
            let (window_entity, window) = &mut *window;
            let window_entity = *window_entity;
            window.set_cursor_position(Some(point));
            events.write(WindowEvent::CursorMoved(CursorMoved {
                window: window_entity,
                position: point,
                delta: None,
            }));
            let button = |state| {
                WindowEvent::MouseButtonInput(MouseButtonInput {
                    window: window_entity,
                    button: replay.pointer_button,
                    state,
                })
            };
            match event {
                Step::Down { .. } => {
                    events.write(button(ButtonState::Pressed));
                }
                Step::Up { .. } | Step::Hover { .. } => {
                    events.write(button(ButtonState::Released));
                }
                Step::Click { .. } => {
                    events.write(button(ButtonState::Pressed));
                    events.write(button(ButtonState::Released));
                }
                Step::Move { .. } => {}
                _ => unreachable!(),
            }

            None
        }
    };
    if let Some(action) = action {
        studio.act(action, &mut jobs);
    }
    if replay.steps.is_empty() {
        elmapa::performance::mark("replay_complete");
    }
}
fn path(file: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("ELMAPA_LIBRARY_DIR").expect("isolated library")).join(file)
}
